//! Services for the API layer
//!
//! This module provides service implementations that bridge between
//! the API handlers and the core business logic.

use oxide_core::{AppError, CollectionPermissions};
use oxide_core::auth::{PermissionService, PermissionContext};
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
    fn check_authentication(&self, context: &PermissionContext) -> Result<bool, AppError> {
        // Basic authentication check - user is authenticated if they have valid claims
        Ok(context.is_authenticated())
    }

    fn check_permission(&self, permissions: &CollectionPermissions, context: &PermissionContext) -> Result<bool, AppError> {
        // For now, implement basic permission checking here
        // In a full implementation, this would delegate to the database's permission service
        use oxide_core::auth::PermissionLevel;
        
        // Get the operation rule for the requested operation
        let rule = match permissions.get_operation_rule(&context.operation) {
            Some(rule) => rule,
            None => {
                // If no rule is defined, default to superuser only
                return Ok(context.is_superuser());
            }
        };

        // Check permission level
        match &rule.permission {
            PermissionLevel::None => Ok(false),
            PermissionLevel::Public => Ok(true),
            PermissionLevel::AuthenticatedOnly => Ok(context.is_authenticated()),
            PermissionLevel::SuperuserOnly => Ok(context.is_superuser()),
            PermissionLevel::Rule(_rule_expr) => {
                // For basic rules, just check authentication
                // A full implementation would parse and evaluate the rule expression
                Ok(context.is_authenticated())
            }
        }
    }

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