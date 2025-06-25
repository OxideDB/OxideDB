//! OxideDB Virtual File System
//!
//! This crate provides a high-performance, secure virtual file system for OxideDB.
//! It supports:
//! 
//! - **Content-addressed storage** with automatic deduplication
//! - **Compression** to reduce storage overhead
//! - **Namespace isolation** for collection-specific file storage
//! - **Event-driven architecture** following the hook-first principle
//! - **Comprehensive backup and restore** with incremental support
//! - **Performance monitoring** and caching
//! - **Security validation** and access controls
//!
//! ## Architecture
//!
//! The VFS follows a layered architecture:
//! 
//! - **Service Layer**: `VfsService` implements the `VirtualFileSystem` trait
//! - **Storage Layer**: `FileSystemStorage` handles physical file operations
//! - **Bridge Layer**: `VfsServiceBridge` connects to oxide-core abstractions
//! - **Backup Layer**: Comprehensive backup and restore functionality
//! - **Utilities**: Common functions for hashing, compression, validation
//!
//! ## Usage
//!
//! ```rust,no_run
//! use oxide_vfs::{VfsService, backup};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> oxide_core::VfsResult<()> {
//!     // Create VFS service
//!     let vfs = VfsService::new(PathBuf::from("/var/lib/oxidedb/vfs"), None);
//!     vfs.initialize().await?;
//!     
//!     // Initialize backup service
//!     backup::initialize_backup_service(PathBuf::from("/var/lib/oxidedb/backups"));
//!     
//!     Ok(())
//! }
//! ```

pub mod service;
pub mod storage;
pub mod backup;
pub mod bridge;
pub mod error;
pub mod utils;

// Re-export main types and functions for convenience
pub use service::{VfsService, VfsMetrics};
pub use storage::FileSystemStorage;
pub use bridge::VfsServiceBridge;
pub use backup::{BackupService, BackupMetadata, BackupType, initialize_backup_service};
pub use utils::*;

// Re-export oxide-core VFS types for convenience
pub use oxide_core::{
    VirtualFileSystem, VfsResult, VfsError, VfsNamespace, VfsNamespaceConfig,
    FileMetadata, FileWriteRequest, FileReadRequest, FileReadResponse,
    FileListRequest, FileListResponse, FileIdentifier, VfsUsageStats
};

/// VFS version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the VFS system with default configuration
pub async fn initialize_vfs_system(
    vfs_path: std::path::PathBuf, 
    backup_path: std::path::PathBuf,
    event_bus: Option<std::sync::Arc<dyn oxide_core::EventBus>>
) -> VfsResult<VfsService> {
    
    // Initialize backup service
    backup::initialize_backup_service(backup_path);
    
    // Create and initialize VFS service
    let vfs_service = VfsService::new(vfs_path, event_bus);
    vfs_service.initialize().await?;
    
    Ok(vfs_service)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_vfs_system_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let vfs_path = temp_dir.path().join("vfs");
        let backup_path = temp_dir.path().join("backups");

        let vfs_service = initialize_vfs_system(vfs_path, backup_path, None).await.unwrap();
        
        // VFS should be initialized and ready
        let metrics = vfs_service.get_metrics().await;
        assert_eq!(metrics.total_reads, 0);
        assert_eq!(metrics.total_writes, 0);
    }

    #[tokio::test]
    async fn test_vfs_service_creation() {
        let temp_dir = TempDir::new().unwrap();
        let vfs_service = VfsService::new(temp_dir.path().to_path_buf(), None);
        
        vfs_service.initialize().await.unwrap();
        
        // Should be able to get metrics
        let metrics = vfs_service.get_metrics().await;
        assert_eq!(metrics.total_reads, 0);
    }
} 