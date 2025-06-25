//! VFS Service Bridge Implementation
//!
//! This module provides the bridge between oxide-core's VFS abstractions
//! and the concrete VFS implementation, following the established pattern
//! used by the logging system.

use oxide_core::{VfsServiceBridge as CoreVfsServiceBridge, VirtualFileSystem};
use crate::service::VfsService;
use std::sync::Arc;

/// Bridge implementation that connects VfsService to core abstractions
pub struct VfsServiceBridge {
    vfs: Arc<VfsService>,
}

impl VfsServiceBridge {
    /// Create a new VFS service bridge
    pub fn new(vfs: Arc<VfsService>) -> Self {
        Self { vfs }
    }

    /// Get the underlying VFS service
    pub fn service(&self) -> &VfsService {
        &self.vfs
    }
}

impl CoreVfsServiceBridge for VfsServiceBridge {
    fn vfs(&self) -> Box<dyn VirtualFileSystem> {
        Box::new(VfsServiceWrapper {
            inner: self.vfs.clone(),
        })
    }
}

/// Wrapper around Arc<VfsService> to implement VirtualFileSystem
#[derive(Clone)]
struct VfsServiceWrapper {
    inner: Arc<VfsService>,
}

#[async_trait::async_trait]
impl VirtualFileSystem for VfsServiceWrapper {
    async fn create_namespace(&self, config: oxide_core::VfsNamespaceConfig) -> oxide_core::VfsResult<()> {
        self.inner.create_namespace(config).await
    }

    async fn delete_namespace(&self, namespace: &oxide_core::VfsNamespace) -> oxide_core::VfsResult<()> {
        self.inner.delete_namespace(namespace).await
    }

    async fn write_file(
        &self,
        namespace: &oxide_core::VfsNamespace,
        request: oxide_core::FileWriteRequest,
    ) -> oxide_core::VfsResult<oxide_core::FileMetadata> {
        self.inner.write_file(namespace, request).await
    }

    async fn read_file(
        &self,
        namespace: &oxide_core::VfsNamespace,
        request: oxide_core::FileReadRequest,
    ) -> oxide_core::VfsResult<oxide_core::FileReadResponse> {
        self.inner.read_file(namespace, request).await
    }

    async fn delete_file(
        &self,
        namespace: &oxide_core::VfsNamespace,
        identifier: oxide_core::FileIdentifier,
    ) -> oxide_core::VfsResult<()> {
        self.inner.delete_file(namespace, identifier).await
    }

    async fn list_files(
        &self,
        namespace: &oxide_core::VfsNamespace,
        request: oxide_core::FileListRequest,
    ) -> oxide_core::VfsResult<oxide_core::FileListResponse> {
        self.inner.list_files(namespace, request).await
    }

    async fn get_usage_stats(&self, namespace: &oxide_core::VfsNamespace) -> oxide_core::VfsResult<oxide_core::VfsUsageStats> {
        self.inner.get_usage_stats(namespace).await
    }

    async fn create_backup(&self, namespace: &oxide_core::VfsNamespace) -> oxide_core::VfsResult<String> {
        self.inner.create_backup(namespace).await
    }

    async fn restore_backup(&self, namespace: &oxide_core::VfsNamespace, backup_id: &str) -> oxide_core::VfsResult<()> {
        self.inner.restore_backup(namespace, backup_id).await
    }

    async fn get_namespace_config(&self, namespace: &oxide_core::VfsNamespace) -> oxide_core::VfsResult<oxide_core::VfsNamespaceConfig> {
        self.inner.get_namespace_config(namespace).await
    }

    async fn update_namespace_config(&self, config: oxide_core::VfsNamespaceConfig) -> oxide_core::VfsResult<()> {
        self.inner.update_namespace_config(config).await
    }
} 