//! VFS Storage Layer
//!
//! This module provides high-performance file storage abstraction for the VFS system.
//! It implements content-addressed storage with deduplication, compression, and atomic operations.

use oxide_core::{VfsResult, VfsError, FileMetadata, VfsNamespace, VfsNamespaceConfig, FileIdentifier};
use crate::utils::{calculate_content_hash, compress_content, decompress_content};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use tokio::fs;
use tracing::{instrument, error, debug, info};
use std::time::{SystemTime, UNIX_EPOCH};

/// File system storage implementation with content-addressed storage
pub struct FileSystemStorage {
    base_path: PathBuf,
    /// Cache of namespace configurations
    namespace_configs: tokio::sync::RwLock<HashMap<VfsNamespace, VfsNamespaceConfig>>,
    /// In-memory cache for recently accessed files metadata
    metadata_cache: tokio::sync::RwLock<HashMap<String, (FileMetadata, SystemTime)>>,
    /// Content deduplication cache (hash -> file_id)
    content_cache: tokio::sync::RwLock<HashMap<String, String>>,
}

impl FileSystemStorage {
    /// Create a new filesystem storage instance
    pub fn new(base_path: PathBuf) -> Self {
        Self { 
            base_path,
            namespace_configs: tokio::sync::RwLock::new(HashMap::new()),
            metadata_cache: tokio::sync::RwLock::new(HashMap::new()),
            content_cache: tokio::sync::RwLock::new(HashMap::new()),
        }
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
        let metadata_dir = self.base_path.join("metadata");
        let namespace_dir = self.base_path.join("namespaces");

        for dir in [&content_dir, &metadata_dir, &namespace_dir] {
            if let Err(e) = fs::create_dir_all(dir).await {
                error!("Failed to create directory {:?}: {}", dir, e);
                return Err(VfsError::IoError {
                    message: format!("Failed to create directory: {}", e),
                });
            }
        }

        // Load existing namespace configurations
        self.load_namespace_configs().await?;

        info!("VFS storage initialized at {:?}", self.base_path);
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
            if !allowed_types.iter().any(|t| metadata.mime_type.starts_with(t)) {
                return Err(VfsError::AccessDenied {
                    path: format!("MIME type {} not allowed", metadata.mime_type),
                });
            }
        }

        // Check for content deduplication
        let content_hash = calculate_content_hash(content);
        if config.enable_deduplication {
            if let Some(existing_id) = self.get_duplicate_content(&content_hash).await {
                debug!("Found duplicate content, reusing file ID: {}", existing_id);
                metadata.id = existing_id;
                metadata.content_hash = content_hash;
                return Ok(metadata);
            }
        }

        // Handle compression if enabled
        let (final_content, compressed) = if config.enable_compression && content.len() > 1024 {
            match compress_content(content) {
                Ok(compressed) if compressed.len() < content.len() => {
                    debug!("Compressed file from {} to {} bytes", content.len(), compressed.len());
                    (compressed, true)
                }
                _ => (content.to_vec(), false),
            }
        } else {
            (content.to_vec(), false)
        };

        // Store content in content-addressed storage
        let content_path = self.get_content_path(&content_hash);
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

        // Update metadata
        metadata.content_hash = content_hash.clone();
        metadata.size = content.len() as u64;
        metadata.compressed = compressed;
        metadata.compression_type = if compressed { Some("gzip".to_string()) } else { None };
        metadata.modified_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Store metadata
        self.store_metadata(namespace, &metadata).await?;

        // Update caches
        self.cache_content_mapping(&content_hash, &metadata.id).await;
        self.cache_metadata(&metadata).await;

        debug!("Successfully stored file: {}", metadata.id);
        Ok(metadata)
    }

    /// Retrieve file content and metadata
    #[instrument(skip(self))]
    pub async fn retrieve_file(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
    ) -> VfsResult<(FileMetadata, Vec<u8>)> {
        // Get file metadata
        let metadata = self.get_file_metadata(namespace, identifier).await?;
        
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
            error!("Content hash mismatch for file {}: expected {}, got {}", 
                   metadata.id, expected_hash, actual_hash);
            return Err(VfsError::IoError {
                message: "Content integrity check failed".to_string(),
            });
        }

        debug!("Successfully retrieved file: {}", metadata.id);
        Ok((metadata, content))
    }

    /// Get file metadata only (without content)
    #[instrument(skip(self))]
    pub async fn get_file_metadata(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
    ) -> VfsResult<FileMetadata> {
        // Check cache first
        let cache_key = format!("{}:{}", namespace, match identifier {
            FileIdentifier::Path(p) => p.clone(),
            FileIdentifier::Id(id) => id.clone(),
        });

        {
            let cache = self.metadata_cache.read().await;
            if let Some((metadata, cached_at)) = cache.get(&cache_key) {
                // Cache is valid for 5 minutes
                if cached_at.elapsed().unwrap_or_default().as_secs() < 300 {
                    return Ok(metadata.clone());
                }
            }
        }

        // Load from disk
        let metadata_path = self.get_metadata_path(namespace, identifier);
        let metadata_content = match fs::read(&metadata_path).await {
            Ok(content) => content,
            Err(_) => {
                return Err(VfsError::FileNotFound {
                    path: match identifier {
                        FileIdentifier::Path(p) => p.clone(),
                        FileIdentifier::Id(id) => id.clone(),
                    },
                });
            }
        };

        let metadata: FileMetadata = match serde_json::from_slice(&metadata_content) {
            Ok(meta) => meta,
            Err(e) => {
                error!("Failed to deserialize metadata: {}", e);
                return Err(VfsError::EncodingError {
                    message: format!("Invalid metadata format: {}", e),
                });
            }
        };

        // Update cache
        self.cache_metadata(&metadata).await;

        Ok(metadata)
    }

    /// Delete file from storage
    #[instrument(skip(self))]
    pub async fn delete_file(
        &self, 
        namespace: &VfsNamespace, 
        identifier: &FileIdentifier
    ) -> VfsResult<()> {
        // Get metadata first to check if file exists and get content hash
        let metadata = self.get_file_metadata(namespace, identifier).await?;
        
        // Remove metadata file
        let metadata_path = self.get_metadata_path(namespace, identifier);
        if let Err(e) = fs::remove_file(&metadata_path).await {
            error!("Failed to remove metadata file: {}", e);
            return Err(VfsError::IoError {
                message: format!("Failed to remove metadata: {}", e),
            });
        }

        // Check if content is still referenced by other files before deleting
        let config = self.get_namespace_config(namespace).await?;
        if config.enable_deduplication {
            // Only remove content if no other files reference it
            if !self.has_content_references(&metadata.content_hash).await? {
                let content_path = self.get_content_path(&metadata.content_hash);
                if let Err(e) = fs::remove_file(&content_path).await {
                    // Log but don't fail - orphaned content will be cleaned up later
                    error!("Failed to remove content file: {}", e);
                }
            }
        } else {
            // Remove content directly
            let content_path = self.get_content_path(&metadata.content_hash);
            let _ = fs::remove_file(&content_path).await; // Don't fail on content removal
        }

        // Remove from caches
        self.remove_from_cache(&metadata).await;

        debug!("Successfully deleted file: {}", metadata.id);
        Ok(())
    }

    /// List files in a namespace with optional filtering
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
        let namespace_path = self.base_path.join("namespaces").join(namespace);
        
        if !namespace_path.exists() {
            return Ok((vec![], 0));
        }

        let mut files = Vec::new();
        let search_path = if directory.is_empty() {
            namespace_path.clone()
        } else {
            namespace_path.join(directory)
        };

        if recursive {
            self.collect_files_recursive(&search_path, &mut files).await?;
        } else {
            self.collect_files_direct(&search_path, &mut files).await?;
        }

        // Apply filters
        let mut filtered_files: Vec<FileMetadata> = files.into_iter()
            .filter(|meta| {
                // MIME type filter
                if let Some(mime_prefix) = mime_filter {
                    if !meta.mime_type.starts_with(mime_prefix) {
                        return false;
                    }
                }
                
                // Tag filter
                if let Some(required_tags) = tag_filter {
                    if !required_tags.iter().all(|tag| meta.tags.contains(tag)) {
                        return false;
                    }
                }
                
                true
            })
            .collect();

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

        Ok((filtered_files, total_count))
    }

    /// Get namespace configuration
    async fn get_namespace_config(&self, namespace: &VfsNamespace) -> VfsResult<VfsNamespaceConfig> {
        let configs = self.namespace_configs.read().await;
        configs.get(namespace).cloned().ok_or_else(|| VfsError::AccessDenied {
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
            if entry.file_type().await.map(|ft| ft.is_dir()).unwrap_or(false) {
                let namespace = entry.file_name().to_string_lossy().to_string();
                let config_path = entry.path().join("config.json");
                
                if let Ok(config_data) = fs::read(&config_path).await {
                    if let Ok(config) = serde_json::from_slice::<VfsNamespaceConfig>(&config_data) {
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

    fn get_metadata_path(&self, namespace: &VfsNamespace, identifier: &FileIdentifier) -> PathBuf {
        let filename = match identifier {
            FileIdentifier::Path(path) => {
                // Convert path to safe filename
                path.replace('/', "_").replace('\\', "_")
            }
            FileIdentifier::Id(id) => id.clone(),
        };
        self.base_path.join("namespaces").join(namespace).join(format!("{}.json", filename))
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

    async fn store_metadata(&self, namespace: &VfsNamespace, metadata: &FileMetadata) -> VfsResult<()> {
        let metadata_path = self.get_metadata_path(namespace, &FileIdentifier::Id(metadata.id.clone()));
        self.ensure_parent_dir(&metadata_path).await?;

        let metadata_json = match serde_json::to_vec_pretty(metadata) {
            Ok(json) => json,
            Err(e) => {
                return Err(VfsError::EncodingError {
                    message: format!("Failed to serialize metadata: {}", e),
                });
            }
        };

        if let Err(e) = fs::write(&metadata_path, &metadata_json).await {
            return Err(VfsError::IoError {
                message: format!("Failed to write metadata: {}", e),
            });
        }

        Ok(())
    }

    async fn get_duplicate_content(&self, content_hash: &str) -> Option<String> {
        let cache = self.content_cache.read().await;
        cache.get(content_hash).cloned()
    }

    async fn cache_content_mapping(&self, content_hash: &str, file_id: &str) {
        let mut cache = self.content_cache.write().await;
        cache.insert(content_hash.to_string(), file_id.to_string());
    }

    async fn cache_metadata(&self, metadata: &FileMetadata) {
        let mut cache = self.metadata_cache.write().await;
        // Use actual namespace from metadata path or extract from metadata
        let namespace = self.extract_namespace_from_metadata(metadata);
        let cache_key = format!("{}:{}", namespace, metadata.id);
        cache.insert(cache_key, (metadata.clone(), SystemTime::now()));
    }

    /// Extract namespace from metadata - using path prefix or custom metadata
    fn extract_namespace_from_metadata(&self, metadata: &FileMetadata) -> String {
        // Try to extract namespace from custom metadata first
        if let Some(namespace) = metadata.custom_metadata.get("namespace") {
            return namespace.clone();
        }
        
        // Fall back to extracting from path (assumes path format: namespace/...)
        let path_parts: Vec<&str> = metadata.path.split('/').collect();
        if path_parts.len() > 1 && !path_parts[0].is_empty() {
            path_parts[0].to_string()
        } else {
            "default".to_string()
        }
    }

    async fn remove_from_cache(&self, metadata: &FileMetadata) {
        {
            let mut meta_cache = self.metadata_cache.write().await;
            let cache_key = format!("{}:{}", "default", metadata.id);
            meta_cache.remove(&cache_key);
        }
        
        {
            let mut content_cache = self.content_cache.write().await;
            content_cache.remove(&metadata.content_hash);
        }
    }

    async fn has_content_references(&self, content_hash: &str) -> VfsResult<bool> {
        // This is a simplified implementation - in production you'd want a proper reference counting system
        let content_cache = self.content_cache.read().await;
        Ok(content_cache.contains_key(content_hash))
    }

    async fn collect_files_recursive(&self, dir_path: &Path, files: &mut Vec<FileMetadata>) -> VfsResult<()> {
        // Use a work queue to avoid async recursion
        let mut queue = vec![dir_path.to_path_buf()];
        
        while let Some(current_dir) = queue.pop() {
            let mut entries = match fs::read_dir(&current_dir).await {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    queue.push(path);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    if let Ok(metadata) = self.load_metadata_from_path(&path).await {
                        files.push(metadata);
                    }
                }
            }
        }

        Ok(())
    }

    async fn collect_files_direct(&self, dir_path: &Path, files: &mut Vec<FileMetadata>) -> VfsResult<()> {
        let mut entries = match fs::read_dir(dir_path).await {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if !path.is_dir() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Ok(metadata) = self.load_metadata_from_path(&path).await {
                    files.push(metadata);
                }
            }
        }

        Ok(())
    }

    async fn load_metadata_from_path(&self, path: &Path) -> VfsResult<FileMetadata> {
        let content = fs::read(path).await.map_err(|e| VfsError::IoError {
            message: format!("Failed to read metadata file: {}", e),
        })?;

        serde_json::from_slice(&content).map_err(|e| VfsError::EncodingError {
            message: format!("Invalid metadata format: {}", e),
        })
    }

    /// Store namespace configuration
    pub async fn store_namespace_config(&self, config: &VfsNamespaceConfig) -> VfsResult<()> {
        let namespace_dir = self.base_path.join("namespaces").join(&config.namespace);
        let config_path = namespace_dir.join("config.json");
        
        self.ensure_parent_dir(&config_path).await?;
        
        let config_json = serde_json::to_vec_pretty(config).map_err(|e| VfsError::EncodingError {
            message: format!("Failed to serialize config: {}", e),
        })?;
        
        fs::write(&config_path, &config_json).await.map_err(|e| VfsError::IoError {
            message: format!("Failed to write config: {}", e),
        })?;

        // Update in-memory cache
        let mut configs = self.namespace_configs.write().await;
        configs.insert(config.namespace.clone(), config.clone());

        Ok(())
    }

    /// Remove namespace and all its files
    pub async fn remove_namespace(&self, namespace: &VfsNamespace) -> VfsResult<()> {
        let namespace_dir = self.base_path.join("namespaces").join(namespace);
        
        if namespace_dir.exists() {
            fs::remove_dir_all(&namespace_dir).await.map_err(|e| VfsError::IoError {
                message: format!("Failed to remove namespace directory: {}", e),
            })?;
        }

        // Remove from cache
        let mut configs = self.namespace_configs.write().await;
        configs.remove(namespace);

        Ok(())
    }

    /// Get usage statistics for a namespace
    pub async fn get_usage_stats(&self, namespace: &VfsNamespace) -> VfsResult<(usize, u64, usize)> {
        let namespace_path = self.base_path.join("namespaces").join(namespace);
        
        if !namespace_path.exists() {
            return Ok((0, 0, 0));
        }

        let mut file_count = 0usize;
        let mut storage_used = 0u64;
        let mut directory_count = 0usize;

        // Walk through namespace directory
        let walker = walkdir::WalkDir::new(&namespace_path).into_iter();
        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                directory_count += 1;
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                file_count += 1;
                // Load metadata to get actual file size
                if let Ok(metadata) = self.load_metadata_from_path(path).await {
                    storage_used += metadata.size;
                }
            }
        }

        Ok((file_count, storage_used, directory_count))
    }
} 