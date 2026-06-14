//! VFS Storage Layer
//!
//! This module provides high-performance file storage abstraction for the VFS system.
//! It implements content-addressed storage with deduplication, compression, and atomic operations.
//! Now featuring ultra-fast metadata lookups using LMDB with LRU cache.

use crate::metadata_store::MetadataStore;
use crate::utils::{
    calculate_content_hash, compress_content, decompress_content, validate_namespace,
};
use oxide_core::{
    FileIdentifier, FileMetadata, VfsError, VfsNamespace, VfsNamespaceConfig, VfsResult,
};
use std::collections::{BTreeSet, HashMap};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tracing::{debug, error, info, instrument};

/// High-performance file system storage implementation with LMDB metadata backend
pub struct FileSystemStorage {
    base_path: PathBuf,
    /// High-performance metadata store with LMDB + LRU cache
    metadata_store: MetadataStore,
    /// Cache of namespace configurations
    namespace_configs: tokio::sync::RwLock<HashMap<VfsNamespace, VfsNamespaceConfig>>,
}

impl FileSystemStorage {
    /// Create a new filesystem storage instance with high-performance metadata backend
    pub fn new(base_path: PathBuf) -> VfsResult<Self> {
        // Create metadata store with optimized cache size (50,000 entries)
        let metadata_store = MetadataStore::new(base_path.clone(), Some(50_000))?;

        Ok(Self {
            base_path,
            metadata_store,
            namespace_configs: tokio::sync::RwLock::new(HashMap::new()),
        })
    }

    /// Get the base path for this storage instance
    pub fn base_path(&self) -> &PathBuf {
        &self.base_path
    }

    /// Initialize storage directories and cache
    #[instrument(skip(self))]
    pub async fn initialize(&self) -> VfsResult<()> {
        // Create base directory structure
        let content_dir = self.base_path.join("content");
        let namespace_dir = self.base_path.join("namespaces");

        for dir in [&content_dir, &namespace_dir] {
            if let Err(e) = fs::create_dir_all(dir).await {
                error!("Failed to create directory {:?}: {}", dir, e);
                return Err(VfsError::IoError {
                    message: format!("Failed to create directory: {}", e),
                });
            }
        }

        // Load existing namespace configurations
        self.load_namespace_configs().await?;

        info!(
            "VFS storage initialized at {:?} with high-performance metadata backend",
            self.base_path
        );
        Ok(())
    }

    /// Store file content and return metadata
    #[instrument(skip(self, content), fields(content_size = content.len()))]
    pub async fn store_file(
        &self,
        namespace: &VfsNamespace,
        content: &[u8],
        mut metadata: FileMetadata,
    ) -> VfsResult<FileMetadata> {
        // Validate namespace exists and get config
        let config = self.get_namespace_config(namespace).await?;

        // Validate file size against namespace limits
        if let Some(max_size) = config.max_file_size {
            if content.len() as u64 > max_size {
                return Err(VfsError::QuotaExceeded {
                    namespace: namespace.clone(),
                });
            }
        }

        // Validate MIME type if restrictions exist
        if let Some(allowed_types) = &config.allowed_mime_types {
            if !allowed_types
                .iter()
                .any(|t| metadata.mime_type.starts_with(t))
            {
                return Err(VfsError::AccessDenied {
                    path: format!("MIME type {} not allowed", metadata.mime_type),
                });
            }
        }

        let replaced_by_path = self
            .metadata_store
            .get_metadata(namespace, &FileIdentifier::Path(metadata.path.clone()))
            .await
            .ok();
        let replaced_by_id = self
            .metadata_store
            .get_metadata(namespace, &FileIdentifier::Id(metadata.id.clone()))
            .await
            .ok();
        let mut replaced_metadata = Vec::new();
        if let Some(replaced) = replaced_by_path {
            replaced_metadata.push(replaced);
        }
        if let Some(replaced) = replaced_by_id {
            if replaced_metadata
                .iter()
                .all(|existing: &FileMetadata| existing.id != replaced.id)
            {
                replaced_metadata.push(replaced);
            }
        }

        let content_hash = calculate_content_hash(content);
        let content_path = self.get_content_path(&content_hash);
        let existing_content_metadata = self
            .metadata_store
            .find_by_content_hash(&content_hash)
            .await?;
        let content_exists = self.content_file_exists(&content_path).await?;

        // Reuse an existing content representation when another logical file has
        // the same hash. The content path is global, so every metadata record
        // that points at it must agree on compression settings.
        let (final_content, compressed, compression_type, should_write_content) =
            if let Some(existing_metadata) = existing_content_metadata {
                let encoded_content = if content_exists {
                    Vec::new()
                } else if existing_metadata.compressed {
                    compress_content(content).map_err(|e| VfsError::CompressionError {
                        message: format!("Failed to recompress missing content: {}", e),
                    })?
                } else {
                    content.to_vec()
                };

                (
                    encoded_content,
                    existing_metadata.compressed,
                    existing_metadata.compression_type,
                    !content_exists,
                )
            } else if config.enable_compression && content.len() > 1024 {
                match compress_content(content) {
                    Ok(compressed_content) if compressed_content.len() < content.len() => {
                        debug!(
                            "Compressed file from {} to {} bytes",
                            content.len(),
                            compressed_content.len()
                        );
                        (compressed_content, true, Some("gzip".to_string()), true)
                    }
                    Ok(_) => (content.to_vec(), false, None, true),
                    Err(e) => {
                        return Err(VfsError::CompressionError {
                            message: format!("Failed to compress file: {}", e),
                        });
                    }
                }
            } else {
                (content.to_vec(), false, None, true)
            };

        if !should_write_content {
            debug!(
                "Reusing existing content blob for hash {} in namespace {}",
                content_hash, namespace
            );
        }

        if should_write_content {
            // Store content in content-addressed storage
            self.ensure_parent_dir(&content_path).await?;

            // Atomic write using temporary file
            let temp_path = content_path.with_extension("tmp");
            if let Err(e) = fs::write(&temp_path, &final_content).await {
                error!("Failed to write file content: {}", e);
                return Err(VfsError::IoError {
                    message: format!("Failed to write file: {}", e),
                });
            }

            if let Err(e) = fs::rename(&temp_path, &content_path).await {
                error!("Failed to rename temporary file: {}", e);
                let _ = fs::remove_file(&temp_path).await; // Cleanup
                return Err(VfsError::IoError {
                    message: format!("Failed to finalize file write: {}", e),
                });
            }
        } else {
            debug!("Skipped content write for duplicate hash {}", content_hash);
        }

        // Update metadata
        metadata.content_hash = content_hash.clone();
        metadata.size = content.len() as u64;
        metadata.compressed = compressed;
        metadata.compression_type = compression_type;
        if metadata.modified_at == 0 {
            metadata.modified_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
        }

        // Store metadata in high-performance LMDB backend
        self.metadata_store
            .store_metadata(namespace, &metadata)
            .await?;

        for replaced in replaced_metadata {
            if replaced.content_hash != content_hash {
                self.remove_content_if_unreferenced(&replaced.content_hash)
                    .await?;
            }
        }

        debug!(
            "Successfully stored file: {} with LMDB metadata backend",
            metadata.id
        );
        Ok(metadata)
    }

    /// Retrieve file content and metadata
    #[instrument(skip(self))]
    pub async fn retrieve_file(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
    ) -> VfsResult<(FileMetadata, Vec<u8>)> {
        // Get file metadata from high-performance store
        let metadata = self
            .metadata_store
            .get_metadata(namespace, identifier)
            .await?;

        // Read content from content-addressed storage
        let content_path = self.get_content_path(&metadata.content_hash);
        let raw_content = match fs::read(&content_path).await {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to read file content from {:?}: {}", content_path, e);
                return Err(VfsError::FileNotFound {
                    path: metadata.path.clone(),
                });
            }
        };

        // Decompress if necessary
        let content = if metadata.compressed {
            match decompress_content(&raw_content) {
                Ok(decompressed) => decompressed,
                Err(e) => {
                    error!("Failed to decompress file {}: {}", metadata.id, e);
                    return Err(VfsError::CompressionError {
                        message: format!("Failed to decompress file: {}", e),
                    });
                }
            }
        } else {
            raw_content
        };

        // Verify content integrity
        let expected_hash = &metadata.content_hash;
        let actual_hash = calculate_content_hash(&content);
        if actual_hash != *expected_hash {
            error!(
                "Content hash mismatch for file {}: expected {}, got {}",
                metadata.id, expected_hash, actual_hash
            );
            return Err(VfsError::IoError {
                message: "Content integrity check failed".to_string(),
            });
        }

        debug!(
            "Successfully retrieved file: {} with LMDB metadata lookup",
            metadata.id
        );
        Ok((metadata, content))
    }

    /// Get file metadata only (ultra-fast with LMDB + LRU cache)
    #[instrument(skip(self))]
    pub async fn get_file_metadata(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
    ) -> VfsResult<FileMetadata> {
        // Direct lookup in high-performance metadata store
        // This should be sub-microsecond for cache hits, single-digit microseconds for LMDB hits
        self.metadata_store
            .get_metadata(namespace, identifier)
            .await
    }

    /// Delete file from storage
    #[instrument(skip(self))]
    pub async fn delete_file(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
    ) -> VfsResult<()> {
        // Get metadata first to check if file exists and get content hash
        let metadata = self
            .metadata_store
            .get_metadata(namespace, identifier)
            .await?;

        // Delete metadata from high-performance store
        self.metadata_store
            .delete_metadata(namespace, identifier)
            .await?;

        self.remove_content_if_unreferenced(&metadata.content_hash)
            .await?;

        debug!(
            "Successfully deleted file: {} from LMDB metadata store",
            metadata.id
        );
        Ok(())
    }

    /// Move or rename a file by updating metadata indexes without rewriting content.
    pub async fn move_file(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
        new_path: String,
        overwrite: bool,
    ) -> VfsResult<(FileMetadata, Option<FileMetadata>)> {
        let mut metadata = self
            .metadata_store
            .get_metadata(namespace, identifier)
            .await?;

        if metadata.path == new_path {
            return Ok((metadata, None));
        }

        let destination = match self
            .metadata_store
            .get_metadata(namespace, &FileIdentifier::Path(new_path.clone()))
            .await
        {
            Ok(existing) if existing.id == metadata.id => return Ok((metadata, None)),
            Ok(existing) if overwrite => Some(existing),
            Ok(_) => {
                return Err(VfsError::FileAlreadyExists { path: new_path });
            }
            Err(VfsError::FileNotFound { .. }) => None,
            Err(e) => return Err(e),
        };

        if let Some(existing) = &destination {
            self.metadata_store
                .delete_metadata(namespace, &FileIdentifier::Id(existing.id.clone()))
                .await?;
            self.remove_content_if_unreferenced(&existing.content_hash)
                .await?;
        }

        metadata.name = Path::new(&new_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        metadata.path = new_path;
        metadata.modified_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.metadata_store
            .store_metadata(namespace, &metadata)
            .await?;

        debug!(
            "Moved file {} to {} in namespace {}",
            metadata.id, metadata.path, namespace
        );
        Ok((metadata, destination))
    }

    /// List files in a namespace with optional filtering (now powered by LMDB)
    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self))]
    pub async fn list_files(
        &self,
        namespace: &VfsNamespace,
        directory: &str,
        recursive: bool,
        mime_filter: Option<&str>,
        tag_filter: Option<&[String]>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> VfsResult<(Vec<FileMetadata>, usize)> {
        // Get all files from high-performance metadata store
        let all_files = self
            .metadata_store
            .list_metadata(namespace, None, None)
            .await?;

        // Apply directory filtering if specified
        let mut filtered_files: Vec<FileMetadata> = if directory.is_empty() {
            all_files
        } else {
            all_files
                .into_iter()
                .filter(|meta| {
                    if recursive {
                        path_is_in_directory(&meta.path, directory)
                    } else {
                        // Non-recursive: check if file is directly in the directory
                        let file_dir = std::path::Path::new(&meta.path)
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new(""))
                            .to_string_lossy();
                        file_dir == directory
                    }
                })
                .collect()
        };

        // Apply MIME type filter
        if let Some(mime_prefix) = mime_filter {
            filtered_files.retain(|meta| meta.mime_type.starts_with(mime_prefix));
        }

        // Apply tag filter
        if let Some(required_tags) = tag_filter {
            filtered_files.retain(|meta| required_tags.iter().all(|tag| meta.tags.contains(tag)));
        }

        let total_count = filtered_files.len();

        // Apply pagination
        if let Some(offset) = offset {
            if offset < filtered_files.len() {
                filtered_files = filtered_files.into_iter().skip(offset).collect();
            } else {
                filtered_files.clear();
            }
        }

        if let Some(limit) = limit {
            filtered_files.truncate(limit);
        }

        debug!(
            "Listed {} filtered files from {} total in namespace {} using LMDB",
            filtered_files.len(),
            total_count,
            namespace
        );
        Ok((filtered_files, total_count))
    }

    /// Get namespace configuration
    async fn get_namespace_config(
        &self,
        namespace: &VfsNamespace,
    ) -> VfsResult<VfsNamespaceConfig> {
        let configs = self.namespace_configs.read().await;
        configs
            .get(namespace)
            .cloned()
            .ok_or_else(|| VfsError::AccessDenied {
                path: namespace.clone(),
            })
    }

    /// Load namespace configurations from disk
    async fn load_namespace_configs(&self) -> VfsResult<()> {
        let namespace_dir = self.base_path.join("namespaces");
        if !namespace_dir.exists() {
            return Ok(());
        }

        let mut entries = match fs::read_dir(&namespace_dir).await {
            Ok(entries) => entries,
            Err(_) => return Ok(()), // Directory doesn't exist yet
        };

        let mut configs = self.namespace_configs.write().await;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry
                .file_type()
                .await
                .map(|ft| ft.is_dir())
                .unwrap_or(false)
            {
                let namespace = entry.file_name().to_string_lossy().to_string();
                let config_path = entry.path().join("config.json");

                if !validate_namespace(&namespace) {
                    continue;
                }

                if let Ok(config_data) = fs::read(&config_path).await {
                    if let Ok(config) = serde_json::from_slice::<VfsNamespaceConfig>(&config_data) {
                        if config.namespace != namespace || !validate_namespace(&config.namespace) {
                            continue;
                        }
                        configs.insert(namespace, config);
                    }
                }
            }
        }

        Ok(())
    }

    /// Helper methods for path construction and utilities
    fn get_content_path(&self, content_hash: &str) -> PathBuf {
        // Use first 2 chars for directory sharding to avoid too many files in one directory
        let (prefix, rest) = content_hash.split_at(2);
        self.base_path.join("content").join(prefix).join(rest)
    }

    async fn ensure_parent_dir(&self, file_path: &Path) -> VfsResult<()> {
        if let Some(parent) = file_path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                return Err(VfsError::IoError {
                    message: format!("Failed to create directory: {}", e),
                });
            }
        }
        Ok(())
    }

    async fn content_file_exists(&self, content_path: &Path) -> VfsResult<bool> {
        match fs::metadata(content_path).await {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(VfsError::IoError {
                message: format!("Failed to inspect content file: {}", e),
            }),
        }
    }

    async fn remove_content_if_unreferenced(&self, content_hash: &str) -> VfsResult<()> {
        if self
            .metadata_store
            .has_content_references(content_hash)
            .await?
        {
            return Ok(());
        }

        let content_path = self.get_content_path(content_hash);
        match fs::remove_file(&content_path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => {
                error!("Failed to remove unreferenced content file: {}", e);
                Err(VfsError::IoError {
                    message: format!("Failed to remove content file: {}", e),
                })
            }
        }
    }

    /// Store namespace configuration
    pub async fn store_namespace_config(&self, config: &VfsNamespaceConfig) -> VfsResult<()> {
        if !validate_namespace(&config.namespace) {
            return Err(VfsError::InvalidPath {
                path: config.namespace.clone(),
            });
        }

        let namespace_dir = self.base_path.join("namespaces").join(&config.namespace);
        let config_path = namespace_dir.join("config.json");

        self.ensure_parent_dir(&config_path).await?;

        let config_json =
            serde_json::to_vec_pretty(config).map_err(|e| VfsError::EncodingError {
                message: format!("Failed to serialize config: {}", e),
            })?;

        fs::write(&config_path, &config_json)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to write config: {}", e),
            })?;

        // Update in-memory cache
        let mut configs = self.namespace_configs.write().await;
        configs.insert(config.namespace.clone(), config.clone());

        Ok(())
    }

    /// Remove namespace and all its files
    pub async fn remove_namespace(&self, namespace: &VfsNamespace) -> VfsResult<()> {
        if !validate_namespace(namespace) {
            return Err(VfsError::InvalidPath {
                path: namespace.clone(),
            });
        }

        let namespace_dir = self.base_path.join("namespaces").join(namespace);

        if namespace_dir.exists() {
            fs::remove_dir_all(&namespace_dir)
                .await
                .map_err(|e| VfsError::IoError {
                    message: format!("Failed to remove namespace directory: {}", e),
                })?;
        }

        // Clear metadata cache for this namespace
        self.metadata_store.clear_namespace_cache(namespace).await;

        // Remove from config cache
        let mut configs = self.namespace_configs.write().await;
        configs.remove(namespace);

        Ok(())
    }

    /// Get usage statistics for a namespace
    pub async fn get_usage_stats(
        &self,
        namespace: &VfsNamespace,
    ) -> VfsResult<(usize, u64, usize)> {
        // Use LMDB to get file count and calculate storage
        let files = self
            .metadata_store
            .list_metadata(namespace, None, None)
            .await?;

        let file_count = files.len();
        let storage_used: u64 = files.iter().map(|f| f.size).sum();
        let directory_count = count_virtual_directories(&files);

        Ok((file_count, storage_used, directory_count))
    }

    /// Get metadata store statistics
    pub async fn get_metadata_stats(&self) -> (usize, usize) {
        self.metadata_store.get_cache_stats().await
    }

    /// Force sync metadata store to disk
    pub async fn sync_metadata(&self) -> VfsResult<()> {
        self.metadata_store.sync()
    }
}

fn path_is_in_directory(path: &str, directory: &str) -> bool {
    path == directory
        || path
            .strip_prefix(directory)
            .is_some_and(|remainder| remainder.starts_with('/'))
}

fn count_virtual_directories(files: &[FileMetadata]) -> usize {
    let mut directories = BTreeSet::new();
    directories.insert(String::new());

    for file in files {
        let mut parts: Vec<&str> = file
            .path
            .split('/')
            .filter(|part| !part.is_empty())
            .collect();
        parts.pop();

        let mut current = String::new();
        for part in parts {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(part);
            directories.insert(current.clone());
        }
    }

    directories.len()
}
