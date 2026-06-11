//! Virtual File System Abstractions
//!
//! This module provides a virtual file system interface that allows
//! collections to have isolated file storage while keeping plugins
//! sandboxed from the real filesystem.
//!
//! ## Architecture
//!
//! - Each collection can have its own isolated filesystem namespace
//! - Plugins access files through VFS API calls, not direct filesystem access
//! - All file operations go through the event system (hook-first principle)
//! - Files are stored with metadata and can be organized in virtual directories
//! - Support for backup, compression, and content-addressed storage

use crate::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Result type for VFS operations
pub type VfsResult<T> = Result<T, VfsError>;

/// Errors that can occur in the virtual file system
#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum VfsError {
    #[error("File not found: {path}")]
    FileNotFound { path: String },

    #[error("Directory not found: {path}")]
    DirectoryNotFound { path: String },

    #[error("Access denied: {path}")]
    AccessDenied { path: String },

    #[error("File already exists: {path}")]
    FileAlreadyExists { path: String },

    #[error("Invalid path: {path}")]
    InvalidPath { path: String },

    #[error("Quota exceeded for namespace: {namespace}")]
    QuotaExceeded { namespace: String },

    #[error("IO error: {message}")]
    IoError { message: String },

    #[error("Encoding error: {message}")]
    EncodingError { message: String },

    #[error("Compression error: {message}")]
    CompressionError { message: String },

    #[error("Event system error: {message}")]
    EventError { message: String },
}

impl From<VfsError> for AppError {
    fn from(err: VfsError) -> Self {
        AppError::VirtualFileSystem {
            message: err.to_string(),
        }
    }
}

/// Unique identifier for a file in the VFS
pub type FileId = String;

/// Virtual file system path (e.g., "/uploads/image.jpg")
pub type VfsPath = String;

/// Namespace identifier for collection-specific filesystems
pub type VfsNamespace = String;

/// File metadata in the virtual file system
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileMetadata {
    /// Unique identifier for the file
    pub id: FileId,
    /// File name (without path)
    pub name: String,
    /// Virtual path within the namespace
    pub path: VfsPath,
    /// MIME type of the file
    pub mime_type: String,
    /// File size in bytes
    pub size: u64,
    /// SHA256 hash of file content
    pub content_hash: String,
    /// When the file was created (Unix timestamp in seconds)
    pub created_at: u64,
    /// When the file was last modified (Unix timestamp in seconds)
    pub modified_at: u64,
    /// Custom metadata as key-value pairs
    pub custom_metadata: HashMap<String, String>,
    /// Whether the file is compressed
    pub compressed: bool,
    /// Compression algorithm used (if any)
    pub compression_type: Option<String>,
    /// Tags for organization and search
    pub tags: Vec<String>,
}

/// Configuration for a VFS namespace
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VfsNamespaceConfig {
    /// Namespace identifier (usually collection name)
    pub namespace: VfsNamespace,
    /// Maximum storage quota in bytes (None = unlimited)
    pub quota_bytes: Option<u64>,
    /// Whether to enable compression for new files
    pub enable_compression: bool,
    /// Allowed MIME types (None = all allowed)
    pub allowed_mime_types: Option<Vec<String>>,
    /// Maximum file size in bytes
    pub max_file_size: Option<u64>,
    /// Whether to enable content deduplication
    pub enable_deduplication: bool,
    /// Backup configuration
    pub backup_config: Option<VfsBackupConfig>,
}

impl Default for VfsNamespaceConfig {
    fn default() -> Self {
        Self {
            namespace: String::new(),
            quota_bytes: Some(1024 * 1024 * 1024), // 1GB default
            enable_compression: true,
            allowed_mime_types: None,               // Allow all by default
            max_file_size: Some(100 * 1024 * 1024), // 100MB default
            enable_deduplication: true,
            backup_config: None,
        }
    }
}

/// Backup configuration for VFS namespaces
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VfsBackupConfig {
    /// Whether backups are enabled
    pub enabled: bool,
    /// Backup retention period in days
    pub retention_days: u32,
    /// Automatic backup interval in hours
    pub backup_interval_hours: u32,
    /// Whether to compress backups
    pub compress_backups: bool,
}

/// Request to create or update a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteRequest {
    /// Virtual path for the file
    pub path: VfsPath,
    /// File content as bytes
    pub content: Vec<u8>,
    /// MIME type (auto-detected if None)
    pub mime_type: Option<String>,
    /// Custom metadata
    pub custom_metadata: Option<HashMap<String, String>>,
    /// Tags for organization
    pub tags: Option<Vec<String>>,
    /// Whether to overwrite if file exists
    pub overwrite: bool,
}

/// Request to read a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadRequest {
    /// Either path or file ID
    pub identifier: FileIdentifier,
    /// Whether to include file content in response
    pub include_content: bool,
}

/// File identifier - either by path or by ID
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileIdentifier {
    Path(VfsPath),
    Id(FileId),
}

/// Response containing file data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadResponse {
    /// File metadata
    pub metadata: FileMetadata,
    /// File content (if requested)
    pub content: Option<Vec<u8>>,
}

/// Request to list files in a directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListRequest {
    /// Directory path (empty string for root)
    pub directory: String,
    /// Whether to include subdirectories recursively
    pub recursive: bool,
    /// Filter by MIME type prefix (e.g., "image/")
    pub mime_filter: Option<String>,
    /// Filter by tags
    pub tag_filter: Option<Vec<String>>,
    /// Pagination offset
    pub offset: Option<usize>,
    /// Pagination limit
    pub limit: Option<usize>,
}

/// Response containing list of files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileListResponse {
    /// List of file metadata
    pub files: Vec<FileMetadata>,
    /// Total count (useful for pagination)
    pub total_count: usize,
    /// Whether there are more results
    pub has_more: bool,
}

/// Statistics about VFS namespace usage
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VfsUsageStats {
    /// Namespace identifier
    pub namespace: VfsNamespace,
    /// Total number of files
    pub file_count: usize,
    /// Total storage used in bytes
    pub storage_used: u64,
    /// Storage quota in bytes (None = unlimited)
    pub storage_quota: Option<u64>,
    /// Number of directories
    pub directory_count: usize,
    /// Last updated timestamp (Unix timestamp in seconds)
    pub last_updated: u64,
}

/// Main VFS service trait
#[async_trait::async_trait]
pub trait VirtualFileSystem: Send + Sync {
    /// Create a new namespace with the given configuration
    async fn create_namespace(&self, config: VfsNamespaceConfig) -> VfsResult<()>;

    /// Delete a namespace and all its files
    async fn delete_namespace(&self, namespace: &VfsNamespace) -> VfsResult<()>;

    /// Write a file to the VFS
    async fn write_file(
        &self,
        namespace: &VfsNamespace,
        request: FileWriteRequest,
    ) -> VfsResult<FileMetadata>;

    /// Read a file from the VFS
    async fn read_file(
        &self,
        namespace: &VfsNamespace,
        request: FileReadRequest,
    ) -> VfsResult<FileReadResponse>;

    /// Delete a file from the VFS
    async fn delete_file(
        &self,
        namespace: &VfsNamespace,
        identifier: FileIdentifier,
    ) -> VfsResult<()>;

    /// List files in a directory
    async fn list_files(
        &self,
        namespace: &VfsNamespace,
        request: FileListRequest,
    ) -> VfsResult<FileListResponse>;

    /// Get usage statistics for a namespace
    async fn get_usage_stats(&self, namespace: &VfsNamespace) -> VfsResult<VfsUsageStats>;

    /// Create a backup of a namespace
    async fn create_backup(&self, namespace: &VfsNamespace) -> VfsResult<String>;

    /// Restore from a backup
    async fn restore_backup(&self, namespace: &VfsNamespace, backup_id: &str) -> VfsResult<()>;

    /// Get namespace configuration
    async fn get_namespace_config(&self, namespace: &VfsNamespace)
        -> VfsResult<VfsNamespaceConfig>;

    /// Update namespace configuration
    async fn update_namespace_config(&self, config: VfsNamespaceConfig) -> VfsResult<()>;
}

/// VFS service bridge for connecting implementations to core abstractions
pub trait VfsServiceBridge: Send + Sync {
    /// Get the VFS implementation
    fn vfs(&self) -> Box<dyn VirtualFileSystem>;
}

/// No-op VFS implementation for when VFS is disabled
#[derive(Debug, Default)]
pub struct NoOpVfs;

#[async_trait::async_trait]
impl VirtualFileSystem for NoOpVfs {
    async fn create_namespace(&self, _config: VfsNamespaceConfig) -> VfsResult<()> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn delete_namespace(&self, _namespace: &VfsNamespace) -> VfsResult<()> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn write_file(
        &self,
        _namespace: &VfsNamespace,
        _request: FileWriteRequest,
    ) -> VfsResult<FileMetadata> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn read_file(
        &self,
        _namespace: &VfsNamespace,
        _request: FileReadRequest,
    ) -> VfsResult<FileReadResponse> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn delete_file(
        &self,
        _namespace: &VfsNamespace,
        _identifier: FileIdentifier,
    ) -> VfsResult<()> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn list_files(
        &self,
        _namespace: &VfsNamespace,
        _request: FileListRequest,
    ) -> VfsResult<FileListResponse> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn get_usage_stats(&self, _namespace: &VfsNamespace) -> VfsResult<VfsUsageStats> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn create_backup(&self, _namespace: &VfsNamespace) -> VfsResult<String> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn restore_backup(&self, _namespace: &VfsNamespace, _backup_id: &str) -> VfsResult<()> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn get_namespace_config(
        &self,
        _namespace: &VfsNamespace,
    ) -> VfsResult<VfsNamespaceConfig> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }

    async fn update_namespace_config(&self, _config: VfsNamespaceConfig) -> VfsResult<()> {
        Err(VfsError::IoError {
            message: "VFS not enabled".to_string(),
        })
    }
}
