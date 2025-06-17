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
            if matches!(rule.permission, PermissionLevel::None) {
                return Ok(false);
            } else {
                return Ok(true);
            }
        }

        // For non-superusers, proceed with normal permission checking
        // For now, implement basic permission checking here
        // In a full implementation, this would delegate to the database's permission service
        use oxide_core::auth::PermissionLevel;
        
        // Get the operation rule for the requested operation
        let rule = match permissions.get_operation_rule(&context.operation) {
            Some(rule) => rule,
            None => {
                // If no rule is defined, default to no access for non-superusers
                return Ok(false);
            }
        };

        // Check permission level
        match &rule.permission {
            PermissionLevel::None => Ok(false),
            PermissionLevel::Public => Ok(true),
            PermissionLevel::AuthenticatedOnly => Ok(context.is_authenticated()),
            PermissionLevel::SuperuserOnly => Ok(context.is_superuser()),
            PermissionLevel::Rule(rule_expr) => {
                // Evaluate the custom rule using the comprehensive rule evaluator
                use oxide_core::auth::RuleEvaluator;
                
                let evaluator = RuleEvaluator::new(context);
                evaluator.evaluate(rule_expr)
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