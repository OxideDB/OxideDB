//! High-Performance Metadata Storage
//!
//! This module provides ultra-fast metadata storage and retrieval using LMDB
//! with an in-memory LRU cache layer for absolute maximum performance.
//!
//! Performance characteristics:
//! - Sub-microsecond cache hits
//! - Single-digit microsecond LMDB lookups
//! - Zero-copy binary serialization
//! - Automatic cache invalidation

use lmdb::{
    Cursor, Database, DatabaseFlags, Environment, EnvironmentFlags, Transaction, WriteFlags,
};
use lru::LruCache;
use oxide_core::{FileIdentifier, FileMetadata, VfsError, VfsNamespace, VfsResult};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;
use tracing::{debug, instrument, warn};

/// Cache entry with timestamp for TTL
#[derive(Debug, Clone)]
struct CacheEntry {
    metadata: FileMetadata,
    cached_at: SystemTime,
}

/// High-performance metadata storage with LMDB backend and LRU cache
pub struct MetadataStore {
    /// LMDB environment
    env: Environment,
    /// Main metadata database
    metadata_db: Database,
    /// Index databases for fast lookups
    path_index_db: Database,
    namespace_index_db: Database,
    /// In-memory LRU cache for hot data
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    /// Cache TTL in seconds (default: 5 minutes)
    cache_ttl_secs: u64,
}

impl MetadataStore {
    /// Create a new metadata store
    pub fn new(base_path: PathBuf, cache_size: Option<usize>) -> VfsResult<Self> {
        let lmdb_path = base_path.join("metadata_db");
        std::fs::create_dir_all(&lmdb_path).map_err(|e| VfsError::IoError {
            message: format!("Failed to create LMDB directory: {}", e),
        })?;

        // Create LMDB environment. Durable defaults are used for production;
        // unsafe async flags require explicit opt-in for benchmark/dev setups.
        let mut env_builder = Environment::new();
        if unsafe_lmdb_async_enabled() {
            warn!("OXIDEDB_VFS_LMDB_UNSAFE_ASYNC is enabled; LMDB durability is reduced");
            env_builder.set_flags(
                EnvironmentFlags::NO_SYNC
                    | EnvironmentFlags::WRITE_MAP
                    | EnvironmentFlags::MAP_ASYNC,
            );
        }

        let env = env_builder
            .set_max_readers(1024)
            .set_max_dbs(4)
            .set_map_size(1024 * 1024 * 1024) // 1GB max
            .open(&lmdb_path)
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to open LMDB environment: {}", e),
            })?;

        // Create databases
        let metadata_db = env
            .create_db(Some("metadata"), DatabaseFlags::empty())
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to create metadata database: {}", e),
            })?;

        let path_index_db = env
            .create_db(Some("path_index"), DatabaseFlags::empty())
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to create path index database: {}", e),
            })?;

        let namespace_index_db = env
            .create_db(Some("namespace_index"), DatabaseFlags::empty())
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to create namespace index database: {}", e),
            })?;

        // Create LRU cache (default 10,000 entries)
        let cache_capacity =
            NonZeroUsize::new(cache_size.unwrap_or(10_000).max(1)).ok_or_else(|| {
                VfsError::IoError {
                    message: "Metadata cache size must be greater than zero".to_string(),
                }
            })?;
        let cache = Arc::new(Mutex::new(LruCache::new(cache_capacity)));

        Ok(Self {
            env,
            metadata_db,
            path_index_db,
            namespace_index_db,
            cache,
            cache_ttl_secs: 300, // 5 minutes
        })
    }

    /// Store file metadata with atomic operation
    #[instrument(skip(self, metadata))]
    pub async fn store_metadata(
        &self,
        namespace: &VfsNamespace,
        metadata: &FileMetadata,
    ) -> VfsResult<()> {
        let metadata_key = format!("{}:{}", namespace, metadata.id);
        let path_key = format!("{}:{}", namespace, metadata.path);
        let namespace_key = format!("{}:{}", namespace, metadata.id);
        let path_cache_key = format!("{}:path:{}", namespace, metadata.path);
        let mut cache_keys_to_invalidate = vec![metadata_key.clone(), path_cache_key.clone()];

        // Serialize metadata using fast binary encoding
        let metadata_data = bincode::serialize(metadata).map_err(|e| VfsError::EncodingError {
            message: format!("Failed to serialize metadata: {}", e),
        })?;

        // Write to LMDB with atomic transaction
        {
            let mut txn = self.env.begin_rw_txn().map_err(|e| VfsError::IoError {
                message: format!("Failed to begin LMDB transaction: {}", e),
            })?;

            let existing_by_id = match txn.get(self.metadata_db, &metadata_key) {
                Ok(existing_data) => Some(
                    bincode::deserialize::<FileMetadata>(existing_data).map_err(|e| {
                        VfsError::EncodingError {
                            message: format!("Failed to deserialize existing metadata: {}", e),
                        }
                    })?,
                ),
                Err(_) => None,
            };

            if let Some(existing) = &existing_by_id {
                if existing.path != metadata.path {
                    let old_path_key = format!("{}:{}", namespace, existing.path);
                    let old_path_cache_key = format!("{}:path:{}", namespace, existing.path);
                    let _ = txn.del(self.path_index_db, &old_path_key, None);
                    cache_keys_to_invalidate.push(old_path_cache_key);
                }
            }

            let existing_id_for_path = match txn.get(self.path_index_db, &path_key) {
                Ok(existing_id_bytes) => {
                    let existing_id = std::str::from_utf8(existing_id_bytes).map_err(|e| {
                        VfsError::EncodingError {
                            message: format!("Invalid path index file ID encoding: {}", e),
                        }
                    })?;
                    Some(existing_id.to_string())
                }
                Err(_) => None,
            };

            if let Some(existing_id) = existing_id_for_path {
                if existing_id != metadata.id {
                    let old_metadata_key = format!("{}:{}", namespace, existing_id);
                    let old_namespace_key = old_metadata_key.clone();
                    let _ = txn.del(self.metadata_db, &old_metadata_key, None);
                    let _ = txn.del(self.namespace_index_db, &old_namespace_key, None);
                    cache_keys_to_invalidate.push(old_metadata_key);
                }
            }

            // Store main metadata
            txn.put(
                self.metadata_db,
                &metadata_key,
                &metadata_data,
                WriteFlags::empty(),
            )
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to store metadata: {}", e),
            })?;

            // Store path index
            txn.put(
                self.path_index_db,
                &path_key,
                &metadata.id,
                WriteFlags::empty(),
            )
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to store path index: {}", e),
            })?;

            // Store namespace index
            txn.put(
                self.namespace_index_db,
                &namespace_key,
                &metadata.id,
                WriteFlags::empty(),
            )
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to store namespace index: {}", e),
            })?;

            txn.commit().map_err(|e| VfsError::IoError {
                message: format!("Failed to commit LMDB transaction: {}", e),
            })?;
        }

        // Update cache and clear stale ID/path entries from replaced metadata.
        self.invalidate_cache_keys(cache_keys_to_invalidate).await;
        self.cache_metadata(&metadata_key, metadata).await;
        self.cache_metadata(&path_cache_key, metadata).await;

        debug!(
            "Stored metadata for file: {} in namespace: {}",
            metadata.id, namespace
        );
        Ok(())
    }

    /// Retrieve file metadata by ID or path with cache-first lookup
    #[instrument(skip(self))]
    pub async fn get_metadata(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
    ) -> VfsResult<FileMetadata> {
        let (cache_key, lookup_key) = match identifier {
            FileIdentifier::Id(id) => {
                let key = format!("{}:{}", namespace, id);
                (key.clone(), key)
            }
            FileIdentifier::Path(path) => {
                let cache_key = format!("{}:path:{}", namespace, path);
                let lookup_key = format!("{}:{}", namespace, path);
                (cache_key, lookup_key)
            }
        };

        // Check cache first
        if let Some(cached) = self.get_from_cache(&cache_key).await {
            debug!("Cache hit for metadata lookup: {}", cache_key);
            return Ok(cached);
        }

        // Cache miss - lookup in LMDB
        debug!("Cache miss for metadata lookup: {}", cache_key);
        let metadata = self
            .lookup_in_lmdb(namespace, identifier, &lookup_key)
            .await?;

        // Cache the result
        self.cache_metadata(&cache_key, &metadata).await;

        Ok(metadata)
    }

    /// Delete metadata atomically
    #[instrument(skip(self))]
    pub async fn delete_metadata(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
    ) -> VfsResult<()> {
        // Get metadata first to build all keys
        let metadata = self.get_metadata(namespace, identifier).await?;

        let metadata_key = format!("{}:{}", namespace, metadata.id);
        let path_key = format!("{}:{}", namespace, metadata.path);
        let namespace_key = format!("{}:{}", namespace, metadata.id);

        // Atomic delete from LMDB
        {
            let mut txn = self.env.begin_rw_txn().map_err(|e| VfsError::IoError {
                message: format!("Failed to begin LMDB transaction: {}", e),
            })?;

            // Delete from all databases
            let _ = txn.del(self.metadata_db, &metadata_key, None);
            let _ = txn.del(self.path_index_db, &path_key, None);
            let _ = txn.del(self.namespace_index_db, &namespace_key, None);

            txn.commit().map_err(|e| VfsError::IoError {
                message: format!("Failed to commit LMDB transaction: {}", e),
            })?;
        }

        // Remove from cache
        self.invalidate_cache(&metadata_key).await;
        self.invalidate_cache(&format!("{}:path:{}", namespace, metadata.path))
            .await;

        debug!(
            "Deleted metadata for file: {} in namespace: {}",
            metadata.id, namespace
        );
        Ok(())
    }

    /// List metadata for files in a namespace with optional filtering
    #[instrument(skip(self))]
    pub async fn list_metadata(
        &self,
        namespace: &VfsNamespace,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> VfsResult<Vec<FileMetadata>> {
        let mut results = Vec::new();
        let namespace_prefix = format!("{}:", namespace);

        let txn = self.env.begin_ro_txn().map_err(|e| VfsError::IoError {
            message: format!("Failed to begin LMDB read transaction: {}", e),
        })?;

        let mut cursor = txn
            .open_ro_cursor(self.metadata_db)
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to open LMDB cursor: {}", e),
            })?;

        let mut count = 0;
        let start_offset = offset.unwrap_or(0);
        let max_limit = limit.unwrap_or(usize::MAX);

        // Use a safer approach: iterate through all entries and filter by namespace
        // This avoids the LMDB NotFound panic when namespace doesn't exist
        for (key, value) in cursor.iter() {
            let key_str = std::str::from_utf8(key).unwrap_or("");

            // Only process keys that start with our namespace prefix
            if !key_str.starts_with(&namespace_prefix) {
                // Skip keys that don't match our namespace
                // Continue to find keys that might match (LMDB keys are sorted)
                if key_str > namespace_prefix.as_str() {
                    // If we've passed our namespace alphabetically, stop
                    break;
                }
                continue;
            }

            // This key belongs to our namespace
            if count >= start_offset {
                if results.len() >= max_limit {
                    break;
                }

                match bincode::deserialize::<FileMetadata>(value) {
                    Ok(metadata) => results.push(metadata),
                    Err(e) => {
                        warn!("Failed to deserialize metadata for key {}: {}", key_str, e);
                        continue;
                    }
                }
            }

            count += 1;
        }

        debug!(
            "Listed {} metadata entries for namespace: {}",
            results.len(),
            namespace
        );
        Ok(results)
    }

    /// Find one metadata record that references the given content hash.
    #[instrument(skip(self))]
    pub async fn find_by_content_hash(
        &self,
        content_hash: &str,
    ) -> VfsResult<Option<FileMetadata>> {
        let txn = self.env.begin_ro_txn().map_err(|e| VfsError::IoError {
            message: format!("Failed to begin LMDB read transaction: {}", e),
        })?;

        let mut cursor = txn
            .open_ro_cursor(self.metadata_db)
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to open LMDB cursor: {}", e),
            })?;

        for (key, value) in cursor.iter() {
            match bincode::deserialize::<FileMetadata>(value) {
                Ok(metadata) if metadata.content_hash == content_hash => {
                    return Ok(Some(metadata));
                }
                Ok(_) => {}
                Err(e) => {
                    let key_str = std::str::from_utf8(key).unwrap_or("<invalid key>");
                    warn!("Failed to deserialize metadata for key {}: {}", key_str, e);
                }
            }
        }

        Ok(None)
    }

    /// Return true when any metadata record still references a content hash.
    #[instrument(skip(self))]
    pub async fn has_content_references(&self, content_hash: &str) -> VfsResult<bool> {
        Ok(self.find_by_content_hash(content_hash).await?.is_some())
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.lock().await;
        (cache.len(), cache.cap().get())
    }

    /// Clear cache for a namespace
    pub async fn clear_namespace_cache(&self, namespace: &VfsNamespace) {
        let namespace_prefix = format!("{}:", namespace);
        let mut cache = self.cache.lock().await;

        let keys_to_remove: Vec<String> = cache
            .iter()
            .filter_map(|(key, _)| {
                if key.starts_with(&namespace_prefix) {
                    Some(key.clone())
                } else {
                    None
                }
            })
            .collect();

        for key in keys_to_remove {
            cache.pop(&key);
        }

        debug!("Cleared cache for namespace: {}", namespace);
    }

    /// Sync LMDB to disk (force persistence)
    pub fn sync(&self) -> VfsResult<()> {
        self.env.sync(true).map_err(|e| VfsError::IoError {
            message: format!("Failed to sync LMDB to disk: {}", e),
        })
    }

    // Private helper methods
    async fn get_from_cache(&self, key: &str) -> Option<FileMetadata> {
        let cache = self.cache.lock().await;
        if let Some(entry) = cache.peek(key) {
            // Check TTL
            if entry.cached_at.elapsed().unwrap_or_default().as_secs() < self.cache_ttl_secs {
                return Some(entry.metadata.clone());
            }
        }
        None
    }

    async fn cache_metadata(&self, key: &str, metadata: &FileMetadata) {
        let mut cache = self.cache.lock().await;
        let entry = CacheEntry {
            metadata: metadata.clone(),
            cached_at: SystemTime::now(),
        };
        cache.put(key.to_string(), entry);
    }

    async fn invalidate_cache(&self, key: &str) {
        let mut cache = self.cache.lock().await;
        cache.pop(key);
    }

    async fn invalidate_cache_keys(&self, keys: Vec<String>) {
        let mut cache = self.cache.lock().await;
        for key in keys {
            cache.pop(&key);
        }
    }

    async fn lookup_in_lmdb(
        &self,
        namespace: &VfsNamespace,
        identifier: &FileIdentifier,
        lookup_key: &str,
    ) -> VfsResult<FileMetadata> {
        let txn = self.env.begin_ro_txn().map_err(|e| VfsError::IoError {
            message: format!("Failed to begin LMDB read transaction: {}", e),
        })?;

        let (actual_key, db) = match identifier {
            FileIdentifier::Id(_) => (lookup_key.to_string(), self.metadata_db),
            FileIdentifier::Path(_) => {
                // First lookup file ID from path index
                match txn.get(self.path_index_db, &lookup_key) {
                    Ok(file_id_bytes) => {
                        let file_id = std::str::from_utf8(file_id_bytes).map_err(|e| {
                            VfsError::EncodingError {
                                message: format!("Invalid file ID encoding: {}", e),
                            }
                        })?;
                        (format!("{}:{}", namespace, file_id), self.metadata_db)
                    }
                    Err(_) => {
                        return Err(VfsError::FileNotFound {
                            path: lookup_key.to_string(),
                        });
                    }
                }
            }
        };

        // Get metadata from main database
        match txn.get(db, &actual_key) {
            Ok(metadata_bytes) => {
                bincode::deserialize::<FileMetadata>(metadata_bytes).map_err(|e| {
                    VfsError::EncodingError {
                        message: format!("Failed to deserialize metadata: {}", e),
                    }
                })
            }
            Err(_) => Err(VfsError::FileNotFound {
                path: match identifier {
                    FileIdentifier::Path(p) => p.clone(),
                    FileIdentifier::Id(id) => id.clone(),
                },
            }),
        }
    }
}

fn unsafe_lmdb_async_enabled() -> bool {
    std::env::var("OXIDEDB_VFS_LMDB_UNSAFE_ASYNC")
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

impl Drop for MetadataStore {
    fn drop(&mut self) {
        // Ensure data is synced before dropping
        let _ = self.sync();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    async fn create_test_store() -> (MetadataStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let store = MetadataStore::new(temp_dir.path().to_path_buf(), Some(100)).unwrap();
        (store, temp_dir)
    }

    fn create_test_metadata(id: &str, path: &str) -> FileMetadata {
        FileMetadata {
            id: id.to_string(),
            name: std::path::Path::new(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            path: path.to_string(),
            mime_type: "text/plain".to_string(),
            size: 1024,
            content_hash: "testhash".to_string(),
            created_at: 1234567890,
            modified_at: 1234567890,
            custom_metadata: HashMap::new(),
            compressed: false,
            compression_type: None,
            tags: vec!["test".to_string()],
        }
    }

    #[tokio::test]
    async fn test_store_and_retrieve_by_id() {
        let (store, _temp_dir) = create_test_store().await;
        let metadata = create_test_metadata("test123", "/test/file.txt");
        let namespace = "testns".to_string();

        // Store metadata
        store.store_metadata(&namespace, &metadata).await.unwrap();

        // Retrieve by ID
        let retrieved = store
            .get_metadata(&namespace, &FileIdentifier::Id("test123".to_string()))
            .await
            .unwrap();
        assert_eq!(retrieved.id, metadata.id);
        assert_eq!(retrieved.path, metadata.path);
    }

    #[tokio::test]
    async fn test_store_and_retrieve_by_path() {
        let (store, _temp_dir) = create_test_store().await;
        let metadata = create_test_metadata("test456", "/test/file2.txt");
        let namespace = "testns".to_string();

        // Store metadata
        store.store_metadata(&namespace, &metadata).await.unwrap();

        // Retrieve by path
        let retrieved = store
            .get_metadata(
                &namespace,
                &FileIdentifier::Path("/test/file2.txt".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(retrieved.id, metadata.id);
        assert_eq!(retrieved.path, metadata.path);
    }

    #[tokio::test]
    async fn test_cache_functionality() {
        let (store, _temp_dir) = create_test_store().await;
        let metadata = create_test_metadata("cached123", "/test/cached.txt");
        let namespace = "testns".to_string();

        // Store metadata
        store.store_metadata(&namespace, &metadata).await.unwrap();

        // First lookup (should populate cache)
        let _ = store
            .get_metadata(&namespace, &FileIdentifier::Id("cached123".to_string()))
            .await
            .unwrap();

        // Check cache stats
        let (used, _capacity) = store.get_cache_stats().await;
        assert!(used > 0);

        // Second lookup should be cache hit
        let retrieved = store
            .get_metadata(&namespace, &FileIdentifier::Id("cached123".to_string()))
            .await
            .unwrap();
        assert_eq!(retrieved.id, metadata.id);
    }

    #[tokio::test]
    async fn test_delete_metadata() {
        let (store, _temp_dir) = create_test_store().await;
        let metadata = create_test_metadata("delete123", "/test/delete.txt");
        let namespace = "testns".to_string();

        // Store and verify
        store.store_metadata(&namespace, &metadata).await.unwrap();
        let _ = store
            .get_metadata(&namespace, &FileIdentifier::Id("delete123".to_string()))
            .await
            .unwrap();

        // Delete
        store
            .delete_metadata(&namespace, &FileIdentifier::Id("delete123".to_string()))
            .await
            .unwrap();

        // Should not be found
        let result = store
            .get_metadata(&namespace, &FileIdentifier::Id("delete123".to_string()))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_metadata() {
        let (store, _temp_dir) = create_test_store().await;
        let namespace = "listns".to_string();

        // Store multiple files
        for i in 0..5 {
            let metadata =
                create_test_metadata(&format!("list{}", i), &format!("/test/file{}.txt", i));
            store.store_metadata(&namespace, &metadata).await.unwrap();
        }

        // List all
        let all_files = store.list_metadata(&namespace, None, None).await.unwrap();
        assert_eq!(all_files.len(), 5);

        // List with limit
        let limited = store
            .list_metadata(&namespace, None, Some(3))
            .await
            .unwrap();
        assert_eq!(limited.len(), 3);

        // List with offset
        let offset = store
            .list_metadata(&namespace, Some(2), Some(2))
            .await
            .unwrap();
        assert_eq!(offset.len(), 2);
    }
}
