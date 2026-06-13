//! VFS Service Implementation
//!
//! This module provides the main VFS service that implements the VirtualFileSystem trait.
//! It follows the event-driven architecture principle by dispatching Before/After events
//! for all operations through the central EventBus.

use crate::backup;
use crate::storage::FileSystemStorage;
use crate::utils::{detect_mime_type, generate_file_id, normalize_path, validate_path};
use oxide_core::event::RequestContext;
use oxide_core::{
    AfterEventContext, AfterEventType, BeforeEventContext, BeforeEventType, EventBus,
    FileIdentifier, FileListRequest, FileListResponse, FileMetadata, FileMoveRequest,
    FileReadRequest, FileReadResponse, FileWriteRequest, VfsError, VfsNamespace,
    VfsNamespaceConfig, VfsResult, VfsUsageStats, VirtualFileSystem,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, error, info, instrument, warn};

/// VFS performance and operational metrics
#[derive(Debug, Clone, Default)]
pub struct VfsMetrics {
    pub total_reads: u64,
    pub total_writes: u64,
    pub total_moves: u64,
    pub total_deletes: u64,
    pub bytes_written: u64,
    pub bytes_read: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub deduplication_saves: u64,
}

/// Main VFS service implementing the VirtualFileSystem trait
pub struct VfsService {
    storage: Arc<FileSystemStorage>,
    namespaces: Arc<RwLock<HashMap<VfsNamespace, VfsNamespaceConfig>>>,
    event_bus: Option<Arc<dyn EventBus>>,
    /// Performance metrics cache
    metrics: Arc<RwLock<VfsMetrics>>,
}

impl VfsService {
    /// Create a new VFS service instance with high-performance metadata backend
    pub fn new(base_path: PathBuf, event_bus: Option<Arc<dyn EventBus>>) -> VfsResult<Self> {
        let storage = FileSystemStorage::new(base_path)?;
        Ok(Self {
            storage: Arc::new(storage),
            namespaces: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            metrics: Arc::new(RwLock::new(VfsMetrics::default())),
        })
    }

    /// Initialize the VFS service
    #[instrument(skip(self))]
    pub async fn initialize(&self) -> VfsResult<()> {
        // Initialize storage
        self.storage.initialize().await?;

        // Load existing namespace configurations
        self.load_namespaces().await?;

        info!("VFS service initialized successfully");
        Ok(())
    }

    /// Get current VFS metrics
    pub async fn get_metrics(&self) -> VfsMetrics {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Load existing namespace configurations from storage
    async fn load_namespaces(&self) -> VfsResult<()> {
        // Scan storage for existing namespace configurations
        let base_path = self.storage.base_path().clone();
        let namespace_dir = base_path.join("namespaces");

        if !namespace_dir.exists() {
            debug!("No existing namespaces directory found");
            return Ok(());
        }

        let mut entries =
            tokio::fs::read_dir(&namespace_dir)
                .await
                .map_err(|e| VfsError::IoError {
                    message: format!("Failed to read namespaces directory: {}", e),
                })?;

        let mut loaded_count = 0;
        let mut namespaces = self.namespaces.write().await;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry
                .file_type()
                .await
                .map(|ft| ft.is_dir())
                .unwrap_or(false)
            {
                let namespace = entry.file_name().to_string_lossy().to_string();
                let config_path = entry.path().join("config.json");

                if config_path.exists() {
                    match tokio::fs::read(&config_path).await {
                        Ok(config_data) => {
                            match serde_json::from_slice::<VfsNamespaceConfig>(&config_data) {
                                Ok(config) => {
                                    namespaces.insert(namespace.clone(), config);
                                    loaded_count += 1;
                                    debug!("Loaded namespace configuration: {}", namespace);
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to deserialize config for namespace {}: {}",
                                        namespace, e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to read config for namespace {}: {}", namespace, e);
                        }
                    }
                }
            }
        }

        info!("Loaded {} existing namespace configurations", loaded_count);
        Ok(())
    }

    /// Update performance metrics
    async fn update_metrics<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut VfsMetrics),
    {
        let mut metrics = self.metrics.write().await;
        update_fn(&mut metrics);
    }

    /// Validate path for security
    fn normalize_file_path(&self, path: &str) -> VfsResult<String> {
        let normalized = normalize_path(path).ok_or_else(|| VfsError::InvalidPath {
            path: path.to_string(),
        })?;

        if normalized.is_empty() {
            return Err(VfsError::InvalidPath {
                path: path.to_string(),
            });
        }

        Ok(normalized)
    }

    /// Normalize a directory path while allowing the virtual root.
    fn normalize_directory_path(&self, path: &str) -> VfsResult<String> {
        normalize_path(path).ok_or_else(|| VfsError::InvalidPath {
            path: path.to_string(),
        })
    }

    /// Normalize path identifiers before storage lookup.
    fn normalize_identifier(&self, identifier: FileIdentifier) -> VfsResult<FileIdentifier> {
        match identifier {
            FileIdentifier::Path(path) => {
                Ok(FileIdentifier::Path(self.normalize_file_path(&path)?))
            }
            FileIdentifier::Id(id) => Ok(FileIdentifier::Id(id)),
        }
    }

    /// Check namespace quota before writing
    async fn check_quota(&self, namespace: &VfsNamespace, additional_bytes: u64) -> VfsResult<()> {
        let namespaces = self.namespaces.read().await;
        if let Some(config) = namespaces.get(namespace) {
            if let Some(quota) = config.quota_bytes {
                let stats = self.get_usage_stats(namespace).await?;
                if stats.storage_used.saturating_add(additional_bytes) > quota {
                    return Err(VfsError::QuotaExceeded {
                        namespace: namespace.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl VirtualFileSystem for VfsService {
    #[instrument(skip(self))]
    async fn create_namespace(&self, config: VfsNamespaceConfig) -> VfsResult<()> {
        // Log namespace creation (namespace events are not yet in core EventBus)
        debug!(
            "VFS before namespace create: namespace={}, quota={:?}",
            config.namespace, config.quota_bytes
        );

        // Validate namespace name
        if config.namespace.is_empty() || !validate_path(&config.namespace) {
            return Err(VfsError::InvalidPath {
                path: config.namespace,
            });
        }

        // Check if namespace already exists
        {
            let namespaces = self.namespaces.read().await;
            if namespaces.contains_key(&config.namespace) {
                return Err(VfsError::FileAlreadyExists {
                    path: config.namespace,
                });
            }
        }

        // Store namespace configuration
        self.storage.store_namespace_config(&config).await?;

        // Add to in-memory cache
        {
            let mut namespaces = self.namespaces.write().await;
            namespaces.insert(config.namespace.clone(), config.clone());
        }

        // Log successful namespace creation
        info!("VFS after namespace create: namespace={}", config.namespace);

        info!("Created VFS namespace: {}", config.namespace);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn delete_namespace(&self, namespace: &VfsNamespace) -> VfsResult<()> {
        // Log namespace deletion (namespace events are not yet in core EventBus)
        debug!("VFS before namespace delete: namespace={}", namespace);

        // Check if namespace exists
        {
            let namespaces = self.namespaces.read().await;
            if !namespaces.contains_key(namespace) {
                return Err(VfsError::AccessDenied {
                    path: namespace.clone(),
                });
            }
        }

        // Remove from storage
        self.storage.remove_namespace(namespace).await?;

        // Remove from in-memory cache
        {
            let mut namespaces = self.namespaces.write().await;
            namespaces.remove(namespace);
        }

        // Log successful namespace deletion
        info!("VFS after namespace delete: namespace={}", namespace);

        info!("Deleted VFS namespace: {}", namespace);
        Ok(())
    }

    #[instrument(skip(self, request), fields(path = %request.path, size = request.content.len()))]
    async fn write_file(
        &self,
        namespace: &VfsNamespace,
        mut request: FileWriteRequest,
    ) -> VfsResult<FileMetadata> {
        // Validate inputs
        request.path = self.normalize_file_path(&request.path)?;

        let existing_metadata = match self
            .storage
            .get_file_metadata(namespace, &FileIdentifier::Path(request.path.clone()))
            .await
        {
            Ok(metadata) => Some(metadata),
            Err(VfsError::FileNotFound { .. }) => None,
            Err(e) => return Err(e),
        };

        if existing_metadata.is_some() && !request.overwrite {
            return Err(VfsError::FileAlreadyExists {
                path: request.path.clone(),
            });
        }

        // Check quota before writing
        let new_size = request.content.len() as u64;
        let additional_bytes = existing_metadata
            .as_ref()
            .map(|existing| new_size.saturating_sub(existing.size))
            .unwrap_or(new_size);
        self.check_quota(namespace, additional_bytes).await?;

        // Emit before event with real content
        let mut before_context = BeforeEventContext::new_vfs_write(
            namespace.clone(),
            request.path.clone(),
            request.content.clone(),
            request.mime_type.clone(),
        );

        if let Some(event_bus) = &self.event_bus {
            match event_bus
                .dispatch_before(BeforeEventType::FileWrite, &mut before_context)
                .await
            {
                Ok(results) => {
                    // Check if any handler failed critically
                    for result in results {
                        if !result.success && !result.skipped {
                            error!(
                                "VFS before file write handler failed: {}",
                                result.error.unwrap_or_else(|| "Unknown error".to_string())
                            );
                            return Err(VfsError::AccessDenied {
                                path: "Event handler rejected write operation".to_string(),
                            });
                        }
                    }
                    debug!("VFS before file write event dispatched successfully");
                }
                Err(e) => {
                    error!("Failed to dispatch VFS before file write event: {}", e);
                    return Err(VfsError::AccessDenied {
                        path: format!("Event dispatch failed: {}", e),
                    });
                }
            }
        }

        // Prepare metadata
        let file_id = existing_metadata
            .as_ref()
            .map(|existing| existing.id.clone())
            .unwrap_or_else(generate_file_id);
        let mime_type = request
            .mime_type
            .unwrap_or_else(|| detect_mime_type(&request.path));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let metadata = FileMetadata {
            id: file_id.clone(),
            name: std::path::Path::new(&request.path)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            path: request.path.clone(),
            mime_type: mime_type.clone(),
            size: request.content.len() as u64,
            content_hash: String::new(), // Will be set by storage layer
            created_at: existing_metadata
                .as_ref()
                .map(|existing| existing.created_at)
                .unwrap_or(now),
            modified_at: now,
            custom_metadata: request.custom_metadata.unwrap_or_else(|| {
                existing_metadata
                    .as_ref()
                    .map(|existing| existing.custom_metadata.clone())
                    .unwrap_or_default()
            }),
            compressed: false, // Will be set by storage layer
            compression_type: None,
            tags: request.tags.unwrap_or_else(|| {
                existing_metadata
                    .as_ref()
                    .map(|existing| existing.tags.clone())
                    .unwrap_or_default()
            }),
        };

        // Store file in storage layer
        let stored_metadata = self
            .storage
            .store_file(namespace, &request.content, metadata)
            .await?;

        // Update metrics
        self.update_metrics(|metrics| {
            metrics.total_writes += 1;
            metrics.bytes_written += request.content.len() as u64;
        })
        .await;

        // Emit after event with real metadata
        let after_context = AfterEventContext::file_written(
            namespace.clone(),
            stored_metadata.id.clone(),
            stored_metadata.path.clone(),
            stored_metadata.size,
            stored_metadata.mime_type.clone(),
            stored_metadata.content_hash.clone(),
            RequestContext::anonymous(),
        );

        if let Some(event_bus) = &self.event_bus {
            if let Err(e) = event_bus
                .dispatch_after(AfterEventType::FileWritten, &after_context)
                .await
            {
                warn!(
                    "Failed to dispatch VFS after file write event (non-critical): {}",
                    e
                );
            }
        }

        debug!(
            "Successfully wrote file: {} ({})",
            stored_metadata.id, stored_metadata.path
        );
        Ok(stored_metadata)
    }

    #[instrument(skip(self), fields(identifier = ?request.identifier, new_path = %request.new_path))]
    async fn move_file(
        &self,
        namespace: &VfsNamespace,
        request: FileMoveRequest,
    ) -> VfsResult<FileMetadata> {
        let identifier = self.normalize_identifier(request.identifier)?;
        let new_path = self.normalize_file_path(&request.new_path)?;
        let overwrite = request.overwrite;

        let source_metadata = self
            .storage
            .get_file_metadata(namespace, &identifier)
            .await?;

        if source_metadata.path == new_path {
            return Ok(source_metadata);
        }

        let destination_metadata = match self
            .storage
            .get_file_metadata(namespace, &FileIdentifier::Path(new_path.clone()))
            .await
        {
            Ok(existing) if existing.id == source_metadata.id => return Ok(source_metadata),
            Ok(existing) if overwrite => Some(existing),
            Ok(_) => {
                return Err(VfsError::FileAlreadyExists { path: new_path });
            }
            Err(VfsError::FileNotFound { .. }) => None,
            Err(e) => return Err(e),
        };

        let mut before_context = BeforeEventContext::new_vfs_move(
            namespace.clone(),
            source_metadata.id.clone(),
            source_metadata.path.clone(),
            new_path.clone(),
            overwrite,
        );

        if let Some(event_bus) = &self.event_bus {
            match event_bus
                .dispatch_before(BeforeEventType::FileMove, &mut before_context)
                .await
            {
                Ok(results) => {
                    for result in results {
                        if !result.success && !result.skipped {
                            error!(
                                "VFS before file move handler failed: {}",
                                result.error.unwrap_or_else(|| "Unknown error".to_string())
                            );
                            return Err(VfsError::AccessDenied {
                                path: "Event handler rejected move operation".to_string(),
                            });
                        }
                    }
                    debug!("VFS before file move event dispatched successfully");
                }
                Err(e) => {
                    error!("Failed to dispatch VFS before file move event: {}", e);
                    return Err(VfsError::AccessDenied {
                        path: format!("Event dispatch failed: {}", e),
                    });
                }
            }
        }

        let old_path = source_metadata.path.clone();
        let (moved_metadata, overwritten_metadata) = self
            .storage
            .move_file(namespace, &identifier, new_path, overwrite)
            .await?;

        self.update_metrics(|metrics| {
            metrics.total_moves += 1;
        })
        .await;

        let overwritten_file_id = overwritten_metadata
            .or(destination_metadata)
            .map(|metadata| metadata.id);
        let after_context = AfterEventContext::file_moved(
            namespace.clone(),
            moved_metadata.id.clone(),
            old_path,
            moved_metadata.path.clone(),
            overwritten_file_id,
            RequestContext::anonymous(),
        );

        if let Some(event_bus) = &self.event_bus {
            if let Err(e) = event_bus
                .dispatch_after(AfterEventType::FileMoved, &after_context)
                .await
            {
                warn!(
                    "Failed to dispatch VFS after file move event (non-critical): {}",
                    e
                );
            }
        }

        debug!(
            "Successfully moved file: {} ({})",
            moved_metadata.id, moved_metadata.path
        );
        Ok(moved_metadata)
    }

    #[instrument(skip(self), fields(identifier = ?request.identifier))]
    async fn read_file(
        &self,
        namespace: &VfsNamespace,
        request: FileReadRequest,
    ) -> VfsResult<FileReadResponse> {
        let identifier = self.normalize_identifier(request.identifier)?;
        let include_content = request.include_content;

        // Emit before event
        // Create proper context for read event
        let (file_id, path) = match &identifier {
            FileIdentifier::Path(p) => ("unknown".to_string(), p.clone()),
            FileIdentifier::Id(id) => (id.clone(), "unknown".to_string()),
        };
        let mut before_context = BeforeEventContext::new_vfs_read(namespace.clone(), file_id, path);

        if let Some(event_bus) = &self.event_bus {
            match event_bus
                .dispatch_before(BeforeEventType::FileRead, &mut before_context)
                .await
            {
                Ok(results) => {
                    // Check if any handler failed critically
                    for result in results {
                        if !result.success && !result.skipped {
                            error!(
                                "VFS before file read handler failed: {}",
                                result.error.unwrap_or_else(|| "Unknown error".to_string())
                            );
                            return Err(VfsError::AccessDenied {
                                path: "Event handler rejected read operation".to_string(),
                            });
                        }
                    }
                    debug!("VFS before file read event dispatched successfully");
                }
                Err(e) => {
                    error!("Failed to dispatch VFS before file read event: {}", e);
                    return Err(VfsError::AccessDenied {
                        path: format!("Event dispatch failed: {}", e),
                    });
                }
            }
        }

        let response = if include_content {
            // Read file with content
            let (metadata, content) = self.storage.retrieve_file(namespace, &identifier).await?;

            // Update metrics
            self.update_metrics(|metrics| {
                metrics.total_reads += 1;
                metrics.bytes_read += content.len() as u64;
            })
            .await;

            FileReadResponse {
                metadata: metadata.clone(),
                content: Some(content),
            }
        } else {
            // Read metadata only
            let metadata = self
                .storage
                .get_file_metadata(namespace, &identifier)
                .await?;

            self.update_metrics(|metrics| {
                metrics.total_reads += 1;
            })
            .await;

            FileReadResponse {
                metadata: metadata.clone(),
                content: None,
            }
        };

        // Emit after event with real metadata
        let after_context = AfterEventContext::file_read(
            namespace.clone(),
            response.metadata.id.clone(),
            response.metadata.path.clone(),
            response.metadata.size,
            include_content,
            RequestContext::anonymous(),
        );

        if let Some(event_bus) = &self.event_bus {
            if let Err(e) = event_bus
                .dispatch_after(AfterEventType::FileRead, &after_context)
                .await
            {
                warn!(
                    "Failed to dispatch VFS after file read event (non-critical): {}",
                    e
                );
            }
        }

        debug!("Successfully read file: {}", response.metadata.id);
        Ok(response)
    }

    #[instrument(skip(self), fields(identifier = ?identifier))]
    async fn delete_file(
        &self,
        namespace: &VfsNamespace,
        identifier: FileIdentifier,
    ) -> VfsResult<()> {
        let identifier = self.normalize_identifier(identifier)?;

        // Get file metadata first for events and validation
        let metadata = self
            .storage
            .get_file_metadata(namespace, &identifier)
            .await?;

        // Emit before event
        // Create proper context for delete event
        let mut before_context = BeforeEventContext::new_vfs_delete(
            namespace.clone(),
            metadata.id.clone(),
            metadata.path.clone(),
        );

        if let Some(event_bus) = &self.event_bus {
            match event_bus
                .dispatch_before(BeforeEventType::FileDelete, &mut before_context)
                .await
            {
                Ok(results) => {
                    // Check if any handler failed critically
                    for result in results {
                        if !result.success && !result.skipped {
                            error!(
                                "VFS before file delete handler failed: {}",
                                result.error.unwrap_or_else(|| "Unknown error".to_string())
                            );
                            return Err(VfsError::AccessDenied {
                                path: "Event handler rejected delete operation".to_string(),
                            });
                        }
                    }
                    debug!("VFS before file delete event dispatched successfully");
                }
                Err(e) => {
                    error!("Failed to dispatch VFS before file delete event: {}", e);
                    return Err(VfsError::AccessDenied {
                        path: format!("Event dispatch failed: {}", e),
                    });
                }
            }
        }

        // Delete from storage
        self.storage.delete_file(namespace, &identifier).await?;

        // Update metrics
        self.update_metrics(|metrics| {
            metrics.total_deletes += 1;
        })
        .await;

        // Emit after event with real metadata
        let after_context = AfterEventContext::file_deleted(
            namespace.clone(),
            metadata.id.clone(),
            metadata.path.clone(),
            RequestContext::anonymous(),
        );

        if let Some(event_bus) = &self.event_bus {
            if let Err(e) = event_bus
                .dispatch_after(AfterEventType::FileDeleted, &after_context)
                .await
            {
                warn!(
                    "Failed to dispatch VFS after file delete event (non-critical): {}",
                    e
                );
            }
        }

        debug!("Successfully deleted file: {}", metadata.id);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn list_files(
        &self,
        namespace: &VfsNamespace,
        mut request: FileListRequest,
    ) -> VfsResult<FileListResponse> {
        request.directory = self.normalize_directory_path(&request.directory)?;

        // Validate namespace exists
        {
            let namespaces = self.namespaces.read().await;
            if !namespaces.contains_key(namespace) {
                return Err(VfsError::AccessDenied {
                    path: namespace.clone(),
                });
            }
        }

        // Get files from storage
        let (files, total_count) = self
            .storage
            .list_files(
                namespace,
                &request.directory,
                request.recursive,
                request.mime_filter.as_deref(),
                request.tag_filter.as_deref(),
                request.offset,
                request.limit,
            )
            .await?;

        let has_more = if let (Some(offset), Some(limit)) = (request.offset, request.limit) {
            offset + limit < total_count
        } else {
            false
        };

        Ok(FileListResponse {
            files,
            total_count,
            has_more,
        })
    }

    #[instrument(skip(self))]
    async fn get_usage_stats(&self, namespace: &VfsNamespace) -> VfsResult<VfsUsageStats> {
        // Validate namespace exists
        let config = {
            let namespaces = self.namespaces.read().await;
            namespaces
                .get(namespace)
                .cloned()
                .ok_or_else(|| VfsError::AccessDenied {
                    path: namespace.clone(),
                })?
        };

        // Get stats from storage
        let (file_count, storage_used, directory_count) =
            self.storage.get_usage_stats(namespace).await?;

        Ok(VfsUsageStats {
            namespace: namespace.clone(),
            file_count,
            storage_used,
            storage_quota: config.quota_bytes,
            directory_count,
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }

    #[instrument(skip(self))]
    async fn create_backup(&self, namespace: &VfsNamespace) -> VfsResult<String> {
        // Validate namespace exists
        {
            let namespaces = self.namespaces.read().await;
            if !namespaces.contains_key(namespace) {
                return Err(VfsError::AccessDenied {
                    path: namespace.clone(),
                });
            }
        }

        // Create backup using backup module
        let backup_id = backup::create_backup(namespace).await?;

        info!("Created backup {} for namespace {}", backup_id, namespace);
        Ok(backup_id)
    }

    #[instrument(skip(self))]
    async fn restore_backup(&self, namespace: &VfsNamespace, backup_id: &str) -> VfsResult<()> {
        // Validate namespace exists
        {
            let namespaces = self.namespaces.read().await;
            if !namespaces.contains_key(namespace) {
                return Err(VfsError::AccessDenied {
                    path: namespace.clone(),
                });
            }
        }

        // Restore backup using backup module
        backup::restore_backup(namespace, backup_id).await?;

        info!("Restored backup {} for namespace {}", backup_id, namespace);
        Ok(())
    }

    #[instrument(skip(self))]
    async fn get_namespace_config(
        &self,
        namespace: &VfsNamespace,
    ) -> VfsResult<VfsNamespaceConfig> {
        let namespaces = self.namespaces.read().await;
        namespaces
            .get(namespace)
            .cloned()
            .ok_or_else(|| VfsError::AccessDenied {
                path: namespace.clone(),
            })
    }

    #[instrument(skip(self))]
    async fn update_namespace_config(&self, config: VfsNamespaceConfig) -> VfsResult<()> {
        // Validate configuration
        if config.namespace.is_empty() {
            return Err(VfsError::InvalidPath {
                path: config.namespace,
            });
        }

        // Store updated configuration
        self.storage.store_namespace_config(&config).await?;

        // Update in-memory cache
        {
            let mut namespaces = self.namespaces.write().await;
            namespaces.insert(config.namespace.clone(), config.clone());
        }

        info!("Updated namespace configuration: {}", config.namespace);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_service() -> (VfsService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let service = VfsService::new(temp_dir.path().to_path_buf(), None).unwrap();
        service.initialize().await.unwrap();
        (service, temp_dir)
    }

    #[tokio::test]
    async fn test_namespace_lifecycle() {
        let (service, _temp_dir) = create_test_service().await;

        let config = VfsNamespaceConfig {
            namespace: "test".to_string(),
            quota_bytes: Some(1024 * 1024), // 1MB
            ..Default::default()
        };

        // Create namespace
        service.create_namespace(config.clone()).await.unwrap();

        // Get namespace config
        let retrieved_config = service
            .get_namespace_config(&config.namespace)
            .await
            .unwrap();
        assert_eq!(retrieved_config.namespace, config.namespace);

        // Delete namespace
        service.delete_namespace(&config.namespace).await.unwrap();

        // Should fail to get deleted namespace
        assert!(service
            .get_namespace_config(&config.namespace)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_file_operations() {
        let (service, _temp_dir) = create_test_service().await;

        // Create namespace first
        let config = VfsNamespaceConfig {
            namespace: "test".to_string(),
            ..Default::default()
        };
        service.create_namespace(config).await.unwrap();

        let content = b"Hello, World!";
        let write_request = FileWriteRequest {
            path: "test.txt".to_string(),
            content: content.to_vec(),
            mime_type: Some("text/plain".to_string()),
            custom_metadata: None,
            tags: Some(vec!["test".to_string()]),
            overwrite: false,
        };

        // Write file
        let metadata = service
            .write_file(&"test".to_string(), write_request)
            .await
            .unwrap();
        assert_eq!(metadata.size, content.len() as u64);
        assert_eq!(metadata.mime_type, "text/plain");

        // Read file
        let read_request = FileReadRequest {
            identifier: FileIdentifier::Id(metadata.id.clone()),
            include_content: true,
        };
        let response = service
            .read_file(&"test".to_string(), read_request)
            .await
            .unwrap();
        assert_eq!(response.content.unwrap(), content);

        // List files
        let list_request = FileListRequest {
            directory: "".to_string(),
            recursive: false,
            mime_filter: None,
            tag_filter: Some(vec!["test".to_string()]),
            offset: None,
            limit: None,
        };
        let list_response = service
            .list_files(&"test".to_string(), list_request)
            .await
            .unwrap();
        assert_eq!(list_response.files.len(), 1);

        // Delete file
        service
            .delete_file(&"test".to_string(), FileIdentifier::Id(metadata.id))
            .await
            .unwrap();

        // File should no longer exist
        let read_request = FileReadRequest {
            identifier: FileIdentifier::Path("test.txt".to_string()),
            include_content: false,
        };
        assert!(service
            .read_file(&"test".to_string(), read_request)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_quota_enforcement() {
        let (service, _temp_dir) = create_test_service().await;

        // Create namespace with small quota
        let config = VfsNamespaceConfig {
            namespace: "test".to_string(),
            quota_bytes: Some(10), // Very small quota
            ..Default::default()
        };
        service.create_namespace(config).await.unwrap();

        let large_content = vec![0u8; 100]; // Larger than quota
        let write_request = FileWriteRequest {
            path: "large.bin".to_string(),
            content: large_content,
            mime_type: None,
            custom_metadata: None,
            tags: None,
            overwrite: false,
        };

        // Should fail due to quota
        let result = service.write_file(&"test".to_string(), write_request).await;
        assert!(matches!(result, Err(VfsError::QuotaExceeded { .. })));
    }

    #[tokio::test]
    async fn test_duplicate_content_keeps_distinct_logical_files() {
        let (service, _temp_dir) = create_test_service().await;
        let namespace = "test".to_string();

        service
            .create_namespace(VfsNamespaceConfig {
                namespace: namespace.clone(),
                ..Default::default()
            })
            .await
            .unwrap();

        let content = b"same bytes".to_vec();
        let first = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "first.txt".to_string(),
                    content: content.clone(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        let second = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "second.txt".to_string(),
                    content: content.clone(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        assert_ne!(first.id, second.id);
        assert_eq!(first.content_hash, second.content_hash);

        service
            .delete_file(&namespace, FileIdentifier::Id(first.id))
            .await
            .unwrap();

        let remaining = service
            .read_file(
                &namespace,
                FileReadRequest {
                    identifier: FileIdentifier::Id(second.id),
                    include_content: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(remaining.content.unwrap(), content);
    }

    #[tokio::test]
    async fn test_overwrite_preserves_file_id_and_replaces_metadata() {
        let (service, _temp_dir) = create_test_service().await;
        let namespace = "test".to_string();

        service
            .create_namespace(VfsNamespaceConfig {
                namespace: namespace.clone(),
                ..Default::default()
            })
            .await
            .unwrap();

        let original = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "/uploads/file.txt".to_string(),
                    content: b"old".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: Some(vec!["old".to_string()]),
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        let replacement = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "uploads/file.txt".to_string(),
                    content: b"replacement".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: Some(vec!["new".to_string()]),
                    overwrite: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(replacement.id, original.id);
        assert_eq!(replacement.created_at, original.created_at);
        assert_eq!(replacement.tags, vec!["new".to_string()]);

        let by_rooted_path = service
            .read_file(
                &namespace,
                FileReadRequest {
                    identifier: FileIdentifier::Path("/uploads/file.txt".to_string()),
                    include_content: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(by_rooted_path.metadata.id, original.id);
        assert_eq!(by_rooted_path.content.unwrap(), b"replacement".to_vec());

        let list = service
            .list_files(
                &namespace,
                FileListRequest {
                    directory: "uploads".to_string(),
                    recursive: false,
                    mime_filter: None,
                    tag_filter: None,
                    offset: None,
                    limit: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(list.files.len(), 1);
    }

    #[tokio::test]
    async fn test_overwrite_quota_uses_size_delta() {
        let (service, _temp_dir) = create_test_service().await;
        let namespace = "test".to_string();

        service
            .create_namespace(VfsNamespaceConfig {
                namespace: namespace.clone(),
                quota_bytes: Some(10),
                ..Default::default()
            })
            .await
            .unwrap();

        service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "file.txt".to_string(),
                    content: vec![b'a'; 8],
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        let replacement = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "file.txt".to_string(),
                    content: vec![b'b'; 9],
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: true,
                },
            )
            .await;

        assert!(replacement.is_ok());
    }

    #[tokio::test]
    async fn test_recursive_directory_listing_respects_boundaries() {
        let (service, _temp_dir) = create_test_service().await;
        let namespace = "test".to_string();

        service
            .create_namespace(VfsNamespaceConfig {
                namespace: namespace.clone(),
                ..Default::default()
            })
            .await
            .unwrap();

        for path in ["uploads/a.txt", "uploads/images/b.txt", "uploads2/c.txt"] {
            service
                .write_file(
                    &namespace,
                    FileWriteRequest {
                        path: path.to_string(),
                        content: path.as_bytes().to_vec(),
                        mime_type: Some("text/plain".to_string()),
                        custom_metadata: None,
                        tags: None,
                        overwrite: false,
                    },
                )
                .await
                .unwrap();
        }

        let list = service
            .list_files(
                &namespace,
                FileListRequest {
                    directory: "uploads".to_string(),
                    recursive: true,
                    mime_filter: None,
                    tag_filter: None,
                    offset: None,
                    limit: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(list.files.len(), 2);
        assert!(list
            .files
            .iter()
            .all(|metadata| metadata.path.starts_with("uploads/")));

        let stats = service.get_usage_stats(&namespace).await.unwrap();
        assert_eq!(stats.directory_count, 4);
    }

    #[tokio::test]
    async fn test_move_file_preserves_identity_and_content() {
        let (service, _temp_dir) = create_test_service().await;
        let namespace = "test".to_string();

        service
            .create_namespace(VfsNamespaceConfig {
                namespace: namespace.clone(),
                ..Default::default()
            })
            .await
            .unwrap();

        let original = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "uploads/report.txt".to_string(),
                    content: b"quarterly results".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: Some(vec!["finance".to_string()]),
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        let moved = service
            .move_file(
                &namespace,
                FileMoveRequest {
                    identifier: FileIdentifier::Id(original.id.clone()),
                    new_path: "archive/2026/report.txt".to_string(),
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        assert_eq!(moved.id, original.id);
        assert_eq!(moved.content_hash, original.content_hash);
        assert_eq!(moved.name, "report.txt");
        assert_eq!(moved.path, "archive/2026/report.txt");
        assert_eq!(moved.tags, vec!["finance".to_string()]);

        let read_moved = service
            .read_file(
                &namespace,
                FileReadRequest {
                    identifier: FileIdentifier::Path("archive/2026/report.txt".to_string()),
                    include_content: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(read_moved.content.unwrap(), b"quarterly results".to_vec());

        let old_path_lookup = service
            .read_file(
                &namespace,
                FileReadRequest {
                    identifier: FileIdentifier::Path("uploads/report.txt".to_string()),
                    include_content: false,
                },
            )
            .await;
        assert!(matches!(
            old_path_lookup,
            Err(VfsError::FileNotFound { .. })
        ));
    }

    #[tokio::test]
    async fn test_move_file_rejects_conflict_unless_overwrite_is_set() {
        let (service, _temp_dir) = create_test_service().await;
        let namespace = "test".to_string();

        service
            .create_namespace(VfsNamespaceConfig {
                namespace: namespace.clone(),
                ..Default::default()
            })
            .await
            .unwrap();

        let source = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "drafts/source.txt".to_string(),
                    content: b"source".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        let destination = service
            .write_file(
                &namespace,
                FileWriteRequest {
                    path: "published/source.txt".to_string(),
                    content: b"destination".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: false,
                },
            )
            .await
            .unwrap();

        let conflict = service
            .move_file(
                &namespace,
                FileMoveRequest {
                    identifier: FileIdentifier::Id(source.id.clone()),
                    new_path: "published/source.txt".to_string(),
                    overwrite: false,
                },
            )
            .await;
        assert!(matches!(conflict, Err(VfsError::FileAlreadyExists { .. })));

        let moved = service
            .move_file(
                &namespace,
                FileMoveRequest {
                    identifier: FileIdentifier::Id(source.id.clone()),
                    new_path: "published/source.txt".to_string(),
                    overwrite: true,
                },
            )
            .await
            .unwrap();

        assert_eq!(moved.id, source.id);
        assert_eq!(moved.path, "published/source.txt");

        let replaced_lookup = service
            .read_file(
                &namespace,
                FileReadRequest {
                    identifier: FileIdentifier::Id(destination.id),
                    include_content: false,
                },
            )
            .await;
        assert!(matches!(
            replaced_lookup,
            Err(VfsError::FileNotFound { .. })
        ));

        let stats = service.get_usage_stats(&namespace).await.unwrap();
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.storage_used, source.size);
    }
}
