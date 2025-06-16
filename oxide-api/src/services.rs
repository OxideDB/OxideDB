//! Services for the API layer
//!
//! This module provides service implementations that bridge between
//! the API handlers and the core business logic.

use oxide_core::{AppError, CollectionPermissions};
use oxide_core::auth::PermissionService;
use oxide_db::Db;
use std::sync::Arc;

/// Database-backed permission service
///
/// This service implements the PermissionService trait by delegating
/// to the database's permission storage methods.
pub struct DatabasePermissionService {
    db: Arc<dyn Db>,
}

impl DatabasePermissionService {
    /// Create a new database permission service
    pub fn new(db: Arc<dyn Db>) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl PermissionService for DatabasePermissionService {
    async fn store_permissions(&self, permissions: &CollectionPermissions) -> Result<(), AppError> {
        self.db.store_permissions(permissions).await
    }

    async fn get_permissions(&self, collection: &str) -> Result<Option<CollectionPermissions>, AppError> {
        self.db.get_permissions(collection).await
    }

    async fn delete_permissions(&self, collection: &str) -> Result<(), AppError> {
        self.db.delete_permissions(collection).await
    }

    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError> {
        self.db.list_collections_with_permissions().await
    }
} 