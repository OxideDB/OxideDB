//! OxideDB Virtual File System
//!
//! This crate provides a secure, isolated virtual file system for OxideDB
//! that allows collections to store files without direct filesystem access.
//!
//! ## Features
//!
//! - Namespace isolation per collection
//! - Content deduplication using SHA256 hashing
//! - Compression support (gzip)
//! - Quota management and usage tracking
//! - Backup and restore functionality
//! - Metadata storage with custom fields
//! - MIME type validation and filtering

pub mod error;
pub mod storage;
pub mod service;
pub mod bridge;
pub mod utils;
pub mod backup;

pub use error::VfsError;
pub use service::VfsService;
pub use storage::FileSystemStorage;
pub use bridge::VfsServiceBridge;
pub use utils::*;

// Re-export core types for convenience
pub use oxide_core::{
    VirtualFileSystem, FileMetadata, VfsNamespaceConfig, VfsBackupConfig,
    FileWriteRequest, FileReadRequest, FileReadResponse, FileListRequest, FileListResponse,
    FileIdentifier, VfsUsageStats, VfsResult, FileId, VfsPath, VfsNamespace
}; 