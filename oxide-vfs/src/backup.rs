//! VFS Backup and Restore Functions
//!
//! This module provides comprehensive backup and restore functionality for VFS namespaces
//! with support for full/incremental backups, compression, and verification.

use chrono::{DateTime, Utc};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use oxide_core::{VfsError, VfsNamespace, VfsResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tar::{Archive, Builder};
use tokio::fs;
use tracing::{debug, error, info, instrument};

/// Backup metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Unique backup identifier
    pub backup_id: String,
    /// Namespace that was backed up
    pub namespace: VfsNamespace,
    /// When the backup was created
    pub created_at: DateTime<Utc>,
    /// Backup type (full or incremental)
    pub backup_type: BackupType,
    /// If incremental, the parent backup ID
    pub parent_backup_id: Option<String>,
    /// Total size of the backup in bytes
    pub backup_size: u64,
    /// Number of files included
    pub file_count: usize,
    /// Checksum of the backup for verification
    pub checksum: String,
    /// Compression algorithm used
    pub compression: String,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

/// Type of backup
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum BackupType {
    Full,
    Incremental,
}

/// Backup service for VFS operations
pub struct BackupService {
    backup_directory: PathBuf,
}

impl BackupService {
    /// Create a new backup service
    pub fn new(backup_directory: PathBuf) -> Self {
        Self { backup_directory }
    }

    /// Initialize the backup service
    #[instrument(skip(self))]
    pub async fn initialize(&self) -> VfsResult<()> {
        if let Err(e) = fs::create_dir_all(&self.backup_directory).await {
            error!("Failed to create backup directory: {}", e);
            return Err(VfsError::IoError {
                message: format!("Failed to create backup directory: {}", e),
            });
        }

        info!("Backup service initialized at {:?}", self.backup_directory);
        Ok(())
    }

    /// Create a full backup of a namespace
    #[instrument(skip(self))]
    pub async fn create_full_backup(
        &self,
        namespace: &VfsNamespace,
        namespace_path: &Path,
    ) -> VfsResult<String> {
        let backup_id = format!("{}_full_{}", namespace, Utc::now().format("%Y%m%d_%H%M%S"));
        let backup_path = self.backup_directory.join(&backup_id);

        // Create backup directory
        fs::create_dir_all(&backup_path)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to create backup directory: {}", e),
            })?;

        // Create compressed tar archive
        let archive_path = backup_path.join("data.tar.gz");
        let metadata = self
            .create_compressed_archive(namespace_path, &archive_path)
            .await?;

        // Create backup metadata
        let backup_metadata = BackupMetadata {
            backup_id: backup_id.clone(),
            namespace: namespace.clone(),
            created_at: Utc::now(),
            backup_type: BackupType::Full,
            parent_backup_id: None,
            backup_size: metadata.0,
            file_count: metadata.1,
            checksum: metadata.2,
            compression: "gzip".to_string(),
            metadata: HashMap::new(),
        };

        // Save metadata
        self.save_backup_metadata(&backup_path, &backup_metadata)
            .await?;

        info!(
            "Created full backup: {} for namespace: {}",
            backup_id, namespace
        );
        Ok(backup_id)
    }

    /// Create an incremental backup
    #[instrument(skip(self))]
    pub async fn create_incremental_backup(
        &self,
        namespace: &VfsNamespace,
        namespace_path: &Path,
        parent_backup_id: &str,
    ) -> VfsResult<String> {
        let backup_id = format!("{}_incr_{}", namespace, Utc::now().format("%Y%m%d_%H%M%S"));
        let backup_path = self.backup_directory.join(&backup_id);

        // Load parent backup metadata to get timestamp
        let parent_metadata = self.load_backup_metadata(parent_backup_id).await?;
        let since_timestamp = parent_metadata.created_at;

        // Create backup directory
        fs::create_dir_all(&backup_path)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to create backup directory: {}", e),
            })?;

        // Create incremental archive (files modified since parent backup)
        let archive_path = backup_path.join("data.tar.gz");
        let metadata = self
            .create_incremental_archive(namespace_path, &archive_path, since_timestamp)
            .await?;

        // Create backup metadata
        let backup_metadata = BackupMetadata {
            backup_id: backup_id.clone(),
            namespace: namespace.clone(),
            created_at: Utc::now(),
            backup_type: BackupType::Incremental,
            parent_backup_id: Some(parent_backup_id.to_string()),
            backup_size: metadata.0,
            file_count: metadata.1,
            checksum: metadata.2,
            compression: "gzip".to_string(),
            metadata: HashMap::new(),
        };

        // Save metadata
        self.save_backup_metadata(&backup_path, &backup_metadata)
            .await?;

        info!(
            "Created incremental backup: {} for namespace: {}",
            backup_id, namespace
        );
        Ok(backup_id)
    }

    /// Restore a backup to a namespace
    #[instrument(skip(self))]
    pub async fn restore_backup(&self, backup_id: &str, restore_path: &Path) -> VfsResult<()> {
        let backup_metadata = self.load_backup_metadata(backup_id).await?;

        match backup_metadata.backup_type {
            BackupType::Full => {
                self.restore_full_backup(backup_id, restore_path).await?;
            }
            BackupType::Incremental => {
                // Implement incremental restore with proper chain handling
                self.restore_incremental_chain(backup_id, restore_path)
                    .await?;
            }
        }

        info!("Restored backup: {} to {:?}", backup_id, restore_path);
        Ok(())
    }

    /// Verify backup integrity
    #[instrument(skip(self))]
    pub async fn verify_backup(&self, backup_id: &str) -> VfsResult<bool> {
        let backup_metadata = self.load_backup_metadata(backup_id).await?;
        let backup_path = self.backup_directory.join(backup_id);
        let archive_path = backup_path.join("data.tar.gz");

        // Verify file exists
        if !archive_path.exists() {
            return Ok(false);
        }

        // Verify checksum
        let actual_checksum = self.calculate_file_checksum(&archive_path).await?;
        let checksum_valid = actual_checksum == backup_metadata.checksum;

        if checksum_valid {
            debug!("Backup {} verification successful", backup_id);
        } else {
            error!(
                "Backup {} checksum mismatch: expected {}, got {}",
                backup_id, backup_metadata.checksum, actual_checksum
            );
        }

        Ok(checksum_valid)
    }

    /// List all backups for a namespace
    pub async fn list_backups(&self, namespace: &VfsNamespace) -> VfsResult<Vec<BackupMetadata>> {
        let mut backups = Vec::new();
        let mut entries =
            fs::read_dir(&self.backup_directory)
                .await
                .map_err(|e| VfsError::IoError {
                    message: format!("Failed to read backup directory: {}", e),
                })?;

        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry
                .file_type()
                .await
                .map(|ft| ft.is_dir())
                .unwrap_or(false)
            {
                let backup_name = entry.file_name().to_string_lossy().to_string();
                if backup_name.starts_with(namespace) {
                    if let Ok(metadata) = self.load_backup_metadata(&backup_name).await {
                        backups.push(metadata);
                    }
                }
            }
        }

        // Sort by creation time, newest first
        backups.sort_by_key(|backup| std::cmp::Reverse(backup.created_at));
        Ok(backups)
    }

    /// Delete a backup
    #[instrument(skip(self))]
    pub async fn delete_backup(&self, backup_id: &str) -> VfsResult<()> {
        let backup_path = self.backup_directory.join(backup_id);

        if backup_path.exists() {
            fs::remove_dir_all(&backup_path)
                .await
                .map_err(|e| VfsError::IoError {
                    message: format!("Failed to delete backup: {}", e),
                })?;
        }

        info!("Deleted backup: {}", backup_id);
        Ok(())
    }

    /// Create compressed tar archive
    async fn create_compressed_archive(
        &self,
        source_path: &Path,
        archive_path: &Path,
    ) -> VfsResult<(u64, usize, String)> {
        let mut file_count = 0usize;
        let temp_tar_path = archive_path.with_extension("tar.tmp");

        // Create uncompressed tar first
        {
            let tar_file =
                std::fs::File::create(&temp_tar_path).map_err(|e| VfsError::IoError {
                    message: format!("Failed to create tar file: {}", e),
                })?;

            let mut tar_builder = Builder::new(tar_file);
            self.add_directory_to_archive(&mut tar_builder, source_path, "", &mut file_count)
                .await?;
            tar_builder.finish().map_err(|e| VfsError::IoError {
                message: format!("Failed to finalize tar: {}", e),
            })?;
        }

        // Compress the tar file
        let tar_data = fs::read(&temp_tar_path)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to read tar file: {}", e),
            })?;

        let compressed_data = {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&tar_data)
                .map_err(|e| VfsError::CompressionError {
                    message: format!("Failed to compress: {}", e),
                })?;
            encoder.finish().map_err(|e| VfsError::CompressionError {
                message: format!("Failed to finish compression: {}", e),
            })?
        };

        // Write compressed data
        fs::write(archive_path, &compressed_data)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to write compressed archive: {}", e),
            })?;

        // Clean up temporary file
        let _ = fs::remove_file(&temp_tar_path).await;

        // Calculate checksum
        let checksum = self.calculate_file_checksum(archive_path).await?;

        Ok((compressed_data.len() as u64, file_count, checksum))
    }

    /// Create incremental archive with files modified since timestamp
    async fn create_incremental_archive(
        &self,
        source_path: &Path,
        archive_path: &Path,
        since: DateTime<Utc>,
    ) -> VfsResult<(u64, usize, String)> {
        let mut file_count = 0usize;
        let temp_tar_path = archive_path.with_extension("tar.tmp");

        // Create uncompressed tar with modified files only
        {
            let tar_file =
                std::fs::File::create(&temp_tar_path).map_err(|e| VfsError::IoError {
                    message: format!("Failed to create tar file: {}", e),
                })?;

            let mut tar_builder = Builder::new(tar_file);
            self.add_modified_files_to_archive(
                &mut tar_builder,
                source_path,
                "",
                since,
                &mut file_count,
            )
            .await?;
            tar_builder.finish().map_err(|e| VfsError::IoError {
                message: format!("Failed to finalize tar: {}", e),
            })?;
        }

        // Compress and finalize same as full backup
        let tar_data = fs::read(&temp_tar_path)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to read tar file: {}", e),
            })?;

        let compressed_data = {
            let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
            encoder
                .write_all(&tar_data)
                .map_err(|e| VfsError::CompressionError {
                    message: format!("Failed to compress: {}", e),
                })?;
            encoder.finish().map_err(|e| VfsError::CompressionError {
                message: format!("Failed to finish compression: {}", e),
            })?
        };

        fs::write(archive_path, &compressed_data)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to write compressed archive: {}", e),
            })?;

        let _ = fs::remove_file(&temp_tar_path).await;
        let checksum = self.calculate_file_checksum(archive_path).await?;

        Ok((compressed_data.len() as u64, file_count, checksum))
    }

    /// Add directory contents to tar archive recursively
    async fn add_directory_to_archive<W: Write>(
        &self,
        builder: &mut Builder<W>,
        path: &Path,
        prefix: &str,
        file_count: &mut usize,
    ) -> VfsResult<()> {
        // Use a queue to avoid async recursion
        let mut queue = vec![(path.to_path_buf(), prefix.to_string())];

        while let Some((current_path, current_prefix)) = queue.pop() {
            let mut entries = fs::read_dir(&current_path)
                .await
                .map_err(|e| VfsError::IoError {
                    message: format!("Failed to read directory: {}", e),
                })?;

            while let Ok(Some(entry)) = entries.next_entry().await {
                let entry_path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                let archive_path = if current_prefix.is_empty() {
                    file_name.clone()
                } else {
                    format!("{}/{}", current_prefix, file_name)
                };

                if entry_path.is_dir() {
                    queue.push((entry_path, archive_path));
                } else {
                    builder
                        .append_path_with_name(&entry_path, &archive_path)
                        .map_err(|e| VfsError::IoError {
                            message: format!("Failed to add file to archive: {}", e),
                        })?;
                    *file_count += 1;
                }
            }
        }

        Ok(())
    }

    /// Add only modified files to archive
    async fn add_modified_files_to_archive<W: Write>(
        &self,
        builder: &mut Builder<W>,
        path: &Path,
        prefix: &str,
        since: DateTime<Utc>,
        file_count: &mut usize,
    ) -> VfsResult<()> {
        // Use a queue to avoid async recursion
        let mut queue = vec![(path.to_path_buf(), prefix.to_string())];

        while let Some((current_path, current_prefix)) = queue.pop() {
            let mut entries = fs::read_dir(&current_path)
                .await
                .map_err(|e| VfsError::IoError {
                    message: format!("Failed to read directory: {}", e),
                })?;

            while let Ok(Some(entry)) = entries.next_entry().await {
                let entry_path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();
                let archive_path = if current_prefix.is_empty() {
                    file_name.clone()
                } else {
                    format!("{}/{}", current_prefix, file_name)
                };

                if entry_path.is_dir() {
                    queue.push((entry_path, archive_path));
                } else {
                    // Check if file was modified since the given timestamp
                    if let Ok(metadata) = entry.metadata().await {
                        if let Ok(modified) = metadata.modified() {
                            let modified_dt = DateTime::<Utc>::from(modified);
                            if modified_dt > since {
                                builder
                                    .append_path_with_name(&entry_path, &archive_path)
                                    .map_err(|e| VfsError::IoError {
                                        message: format!("Failed to add file to archive: {}", e),
                                    })?;
                                *file_count += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Restore full backup
    async fn restore_full_backup(&self, backup_id: &str, restore_path: &Path) -> VfsResult<()> {
        let backup_path = self.backup_directory.join(backup_id);
        let archive_path = backup_path.join("data.tar.gz");

        self.extract_archive(&archive_path, restore_path).await
    }

    /// Restore incremental backup
    async fn restore_incremental_backup(
        &self,
        backup_id: &str,
        restore_path: &Path,
    ) -> VfsResult<()> {
        let backup_path = self.backup_directory.join(backup_id);
        let archive_path = backup_path.join("data.tar.gz");

        self.extract_archive(&archive_path, restore_path).await
    }

    /// Restore incremental backup chain with proper dependency resolution
    async fn restore_incremental_chain(
        &self,
        backup_id: &str,
        restore_path: &Path,
    ) -> VfsResult<()> {
        // Build the chain of backups from root to target
        let backup_chain = self.build_backup_chain(backup_id).await?;

        // Restore backups in order from oldest to newest
        for chain_backup_id in backup_chain {
            let backup_metadata = self.load_backup_metadata(&chain_backup_id).await?;

            match backup_metadata.backup_type {
                BackupType::Full => {
                    info!("Restoring full backup: {}", chain_backup_id);
                    self.restore_full_backup(&chain_backup_id, restore_path)
                        .await?;
                }
                BackupType::Incremental => {
                    info!("Restoring incremental backup: {}", chain_backup_id);
                    self.restore_incremental_backup(&chain_backup_id, restore_path)
                        .await?;
                }
            }
        }

        info!(
            "Successfully restored incremental backup chain ending with: {}",
            backup_id
        );
        Ok(())
    }

    /// Build the chain of backups from a full backup to the target incremental backup
    async fn build_backup_chain(&self, target_backup_id: &str) -> VfsResult<Vec<String>> {
        let mut chain = Vec::new();
        let mut current_backup_id = target_backup_id.to_string();

        // Walk backwards from target to root to build the chain
        loop {
            let metadata = self.load_backup_metadata(&current_backup_id).await?;
            chain.push(current_backup_id.clone());

            match metadata.backup_type {
                BackupType::Full => {
                    // Reached the root full backup
                    break;
                }
                BackupType::Incremental => {
                    if let Some(parent_id) = metadata.parent_backup_id {
                        current_backup_id = parent_id;
                    } else {
                        return Err(VfsError::IoError {
                            message: format!(
                                "Incremental backup {} has no parent backup",
                                current_backup_id
                            ),
                        });
                    }
                }
            }

            // Safety check to prevent infinite loops
            if chain.len() > 100 {
                return Err(VfsError::IoError {
                    message: "Backup chain is too deep (>100 levels)".to_string(),
                });
            }
        }

        // Reverse the chain so we restore from oldest to newest
        chain.reverse();

        debug!(
            "Built backup chain with {} backups: {:?}",
            chain.len(),
            chain
        );
        Ok(chain)
    }

    /// Extract compressed archive to destination
    async fn extract_archive(&self, archive_path: &Path, destination: &Path) -> VfsResult<()> {
        // Read compressed data
        let compressed_data = fs::read(archive_path)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to read archive: {}", e),
            })?;

        // Decompress
        let mut decoder = GzDecoder::new(&compressed_data[..]);
        let mut tar_data = Vec::new();
        decoder
            .read_to_end(&mut tar_data)
            .map_err(|e| VfsError::CompressionError {
                message: format!("Failed to decompress: {}", e),
            })?;

        // Extract tar
        let mut archive = Archive::new(&tar_data[..]);
        archive.unpack(destination).map_err(|e| VfsError::IoError {
            message: format!("Failed to extract archive: {}", e),
        })?;

        Ok(())
    }

    /// Save backup metadata
    async fn save_backup_metadata(
        &self,
        backup_path: &Path,
        metadata: &BackupMetadata,
    ) -> VfsResult<()> {
        let metadata_path = backup_path.join("metadata.json");
        let metadata_json =
            serde_json::to_vec_pretty(metadata).map_err(|e| VfsError::EncodingError {
                message: format!("Failed to serialize metadata: {}", e),
            })?;

        fs::write(&metadata_path, &metadata_json)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to write metadata: {}", e),
            })
    }

    /// Load backup metadata
    async fn load_backup_metadata(&self, backup_id: &str) -> VfsResult<BackupMetadata> {
        let backup_path = self.backup_directory.join(backup_id);
        let metadata_path = backup_path.join("metadata.json");

        let metadata_data = fs::read(&metadata_path)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to read metadata: {}", e),
            })?;

        serde_json::from_slice(&metadata_data).map_err(|e| VfsError::EncodingError {
            message: format!("Failed to deserialize metadata: {}", e),
        })
    }

    /// Calculate SHA256 checksum of a file
    async fn calculate_file_checksum(&self, file_path: &Path) -> VfsResult<String> {
        let content = fs::read(file_path).await.map_err(|e| VfsError::IoError {
            message: format!("Failed to read file for checksum: {}", e),
        })?;

        Ok(crate::utils::calculate_content_hash(&content))
    }
}

// Global backup service instance
static BACKUP_SERVICE: OnceLock<BackupService> = OnceLock::new();

/// Initialize the global backup service
pub fn initialize_backup_service(backup_directory: PathBuf) {
    let _ = BACKUP_SERVICE.set(BackupService::new(backup_directory));
}

/// Get the global backup service
fn get_backup_service() -> &'static BackupService {
    BACKUP_SERVICE
        .get()
        .expect("Backup service not initialized")
}

/// Create a backup of a VFS namespace
pub async fn create_backup(namespace: &VfsNamespace) -> VfsResult<String> {
    let service = get_backup_service();

    // Determine the namespace path based on VFS storage structure
    let namespace_path = service
        .backup_directory
        .parent()
        .ok_or_else(|| VfsError::IoError {
            message: "Invalid backup directory structure".to_string(),
        })?
        .join("vfs")
        .join("namespaces")
        .join(namespace);

    if !namespace_path.exists() {
        return Err(VfsError::AccessDenied {
            path: format!("Namespace '{}' does not exist", namespace),
        });
    }

    // Check if there are existing backups to determine if this should be incremental
    let existing_backups = service.list_backups(namespace).await?;

    if existing_backups.is_empty() {
        // Create full backup for first backup
        info!("Creating first full backup for namespace: {}", namespace);
        service.create_full_backup(namespace, &namespace_path).await
    } else {
        // Find the most recent backup
        let latest_backup = existing_backups.first().unwrap(); // Already sorted by creation time

        // Check if we should create incremental or full backup
        // Create incremental if the latest backup is less than 7 days old
        let backup_age = Utc::now().signed_duration_since(latest_backup.created_at);

        if backup_age.num_days() < 7 && latest_backup.backup_type == BackupType::Full {
            info!("Creating incremental backup for namespace: {}", namespace);
            service
                .create_incremental_backup(namespace, &namespace_path, &latest_backup.backup_id)
                .await
        } else {
            info!("Creating new full backup for namespace: {}", namespace);
            service.create_full_backup(namespace, &namespace_path).await
        }
    }
}

/// Restore a VFS namespace from backup
pub async fn restore_backup(namespace: &VfsNamespace, backup_id: &str) -> VfsResult<()> {
    let service = get_backup_service();

    // Determine the restore path based on VFS storage structure
    let restore_path = service
        .backup_directory
        .parent()
        .ok_or_else(|| VfsError::IoError {
            message: "Invalid backup directory structure".to_string(),
        })?
        .join("vfs")
        .join("namespaces")
        .join(namespace);

    // Verify backup exists
    let backup_metadata = service.load_backup_metadata(backup_id).await?;
    if backup_metadata.namespace != *namespace {
        return Err(VfsError::AccessDenied {
            path: format!(
                "Backup {} does not belong to namespace {}",
                backup_id, namespace
            ),
        });
    }

    // Create parent directories if they don't exist
    if let Some(parent) = restore_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to create restore directory: {}", e),
            })?;
    }

    // Clear existing namespace data before restore
    if restore_path.exists() {
        tokio::fs::remove_dir_all(&restore_path)
            .await
            .map_err(|e| VfsError::IoError {
                message: format!("Failed to clear existing data: {}", e),
            })?;
    }

    // Perform the restore
    service.restore_backup(backup_id, &restore_path).await?;

    info!(
        "Successfully restored backup {} for namespace {}",
        backup_id, namespace
    );
    Ok(())
}
