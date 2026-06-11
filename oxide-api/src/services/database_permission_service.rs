//! Database permission service implementation
//!
//! This module provides permission management functionality using the database
//! as the backend for storing and retrieving permission information.

use async_trait::async_trait;
use oxide_core::{
    auth::{CollectionPermissions, PermissionContext, PermissionService},
    AppError,
};
use oxide_db::Db;
use std::sync::Arc;

/// Database-backed permission service
///
/// This service implements the `PermissionService` trait and uses the database
/// to store and retrieve permission information for collections.
pub struct DatabasePermissionService {
    db: Arc<dyn Db>,
}

impl DatabasePermissionService {
    /// Create a new database permission service
    pub fn new(db: Arc<dyn Db>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PermissionService for DatabasePermissionService {
    /// Check if a user is authenticated
    fn check_authentication(&self, context: &PermissionContext) -> Result<bool, AppError> {
        Ok(context.user_claims.is_some())
    }

    /// Check if a user can perform an operation on a collection
    fn check_permission(
        &self,
        permissions: &CollectionPermissions,
        context: &PermissionContext,
    ) -> Result<bool, AppError> {
        // First check: if the user is a superuser, they should have access to everything
        // unless explicitly denied (PermissionLevel::None)
        if context.is_superuser() {
            // Get the operation rule for the requested operation
            let rule = match permissions.get_operation_rule(&context.operation) {
                Some(rule) => rule,
                None => {
                    // If no rule is defined, superuser has access
                    return Ok(true);
                }
            };

            // Only deny superuser access if explicitly set to None
            if matches!(rule.permission, oxide_core::auth::PermissionLevel::None) {
                return Ok(false);
            } else {
                return Ok(true);
            }
        }

        // For non-superusers, proceed with normal permission checking
        let rule = match permissions.get_operation_rule(&context.operation) {
            Some(rule) => rule,
            None => return Ok(false), // No rule means no access for non-superusers
        };

        match &rule.permission {
            oxide_core::auth::PermissionLevel::None => Ok(false),
            oxide_core::auth::PermissionLevel::Public => Ok(true),
            oxide_core::auth::PermissionLevel::AuthenticatedOnly => {
                Ok(context.user_claims.is_some())
            }
            oxide_core::auth::PermissionLevel::SuperuserOnly => Ok(matches!(
                context.user_role(),
                Some(oxide_core::auth::UserRole::Superuser)
            )),
            oxide_core::auth::PermissionLevel::Rule(rule_expr) => {
                // Import the rule evaluator from the rules module
                use oxide_core::auth::rules::RuleEvaluator;
                let evaluator = RuleEvaluator::new(context);
                evaluator.evaluate(rule_expr)
            }
        }
    }

    /// Store permissions for a collection
    async fn store_permissions(&self, permissions: &CollectionPermissions) -> Result<(), AppError> {
        // Delegate to the database's permission system
        self.db.store_permissions(permissions).await
    }

    /// Get permissions for a collection
    async fn get_permissions(
        &self,
        collection: &str,
    ) -> Result<Option<CollectionPermissions>, AppError> {
        // Delegate to the database's permission system
        self.db.get_permissions(collection).await
    }

    /// Delete permissions for a collection (revert to defaults)
    async fn delete_permissions(&self, collection: &str) -> Result<(), AppError> {
        // Delegate to the database's permission system
        self.db.delete_permissions(collection).await
    }

    /// List all collections that have custom permissions
    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError> {
        // For now, return all collections since we don't have a separate permissions table
        // In a full implementation, this would query a permissions table for collections with custom permissions
        let collections = self.db.list_collections().await?;
        Ok(collections.into_iter().map(|schema| schema.name).collect())
    }
}
