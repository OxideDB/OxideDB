//! Permission system
//!
//! This module provides the permission system for controlling access to collections
//! and operations, including permission rules, contexts, and service traits.

use crate::AppError;
use super::types::{CrudOperation, PermissionLevel, UserRole};
use super::jwt::Claims;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use ts_rs::TS;

/// Permission rule for a specific operation on a collection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OperationRule {
    /// The CRUD operation this rule applies to
    pub operation: CrudOperation,
    /// The permission level for this operation
    pub permission: PermissionLevel,
    /// Optional filter for list operations (like PocketBase filter syntax)
    pub filter: Option<String>,
}

/// Complete permission rules for a collection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CollectionPermissions {
    /// Collection name
    pub collection: String,
    /// Rules for each CRUD operation
    pub rules: HashMap<CrudOperation, OperationRule>,
    /// Whether this collection requires authentication by default
    pub auth_required: bool,
    /// Timestamp when permissions were created
    pub created_at: i64,
    /// Timestamp when permissions were last updated
    pub updated_at: i64,
}

impl CollectionPermissions {
    /// Create new permission rules for a collection with default restrictions
    pub fn new(collection: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut rules = HashMap::new();
        
        // Default rules: superuser only for all operations
        for operation in [CrudOperation::Create, CrudOperation::Read, CrudOperation::Update, CrudOperation::Delete, CrudOperation::List] {
            rules.insert(operation.clone(), OperationRule {
                operation: operation.clone(),
                permission: PermissionLevel::SuperuserOnly,
                filter: None,
            });
        }

        Self {
            collection,
            rules,
            auth_required: true,
            created_at: now,
            updated_at: now,
        }
    }

    /// Create permissive rules for a collection (public access)
    pub fn public(collection: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut rules = HashMap::new();
        
        // Public rules: allow all operations for everyone
        for operation in [CrudOperation::Create, CrudOperation::Read, CrudOperation::Update, CrudOperation::Delete, CrudOperation::List] {
            rules.insert(operation.clone(), OperationRule {
                operation: operation.clone(),
                permission: PermissionLevel::Public,
                filter: None,
            });
        }

        Self {
            collection,
            rules,
            auth_required: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Get the operation rule for a specific CRUD operation
    pub fn get_operation_rule(&self, operation: &CrudOperation) -> Option<&OperationRule> {
        self.rules.get(operation)
    }

    /// Set the permission level for a specific operation
    pub fn set_operation_permission(&mut self, operation: CrudOperation, permission: PermissionLevel) {
        let rule = OperationRule {
            operation: operation.clone(),
            permission,
            filter: None,
        };
        self.rules.insert(operation, rule);
        self.update_timestamp();
    }

    /// Update the updated_at timestamp
    fn update_timestamp(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
    }
}

/// Context for permission evaluation
#[derive(Debug, Clone)]
pub struct PermissionContext {
    /// The user's JWT claims (if authenticated)
    pub user_claims: Option<Claims>,
    /// The CRUD operation being performed
    pub operation: CrudOperation,
    /// The collection being accessed
    pub collection: String,
    /// The specific record ID (if applicable)
    pub record_id: Option<String>,
    /// Additional request metadata (headers, etc.)
    pub metadata: HashMap<String, serde_json::Value>,
    /// Record data for evaluation (if available)
    pub record_data: Option<serde_json::Value>,
}

impl PermissionContext {
    /// Create a new permission context
    pub fn new(
        user_claims: Option<Claims>,
        operation: CrudOperation,
        collection: String,
        record_id: Option<String>,
    ) -> Self {
        Self {
            user_claims,
            operation,
            collection,
            record_id,
            metadata: HashMap::new(),
            record_data: None,
        }
    }

    /// Add request metadata (headers, etc.)
    pub fn with_metadata(mut self, metadata: HashMap<String, serde_json::Value>) -> Self {
        self.metadata = metadata;
        self
    }

    /// Add record data for evaluation
    pub fn with_record_data(mut self, record_data: serde_json::Value) -> Self {
        self.record_data = Some(record_data);
        self
    }

    /// Get the user's role if authenticated
    pub fn user_role(&self) -> Option<UserRole> {
        self.user_claims.as_ref()
            .and_then(|claims| claims.user_role().ok())
    }

    /// Check if the user is authenticated
    pub fn is_authenticated(&self) -> bool {
        self.user_claims.is_some()
    }

    /// Check if the user is a superuser
    pub fn is_superuser(&self) -> bool {
        let user_role = self.user_role();
        let is_super = matches!(user_role, Some(UserRole::Superuser));
        
        // Enhanced debug logging for superuser check
        if let Some(claims) = &self.user_claims {
            tracing::debug!(
                "🔍 Superuser check: user_id={}, role_string='{}', parsed_role={:?}, is_superuser={}",
                claims.sub,
                claims.role,
                user_role,
                is_super
            );
        }
        
        is_super
    }
}

/// Permission service trait for checking authorization
#[async_trait::async_trait]
pub trait PermissionService: Send + Sync {
    /// Check if a user is authenticated
    fn check_authentication(&self, context: &PermissionContext) -> Result<bool, AppError>;

    /// Check if a user can perform an operation on a collection
    fn check_permission(&self, permissions: &CollectionPermissions, context: &PermissionContext) -> Result<bool, AppError>;

    /// Store permissions for a collection
    async fn store_permissions(&self, permissions: &CollectionPermissions) -> Result<(), AppError>;

    /// Get permissions for a collection
    async fn get_permissions(&self, collection: &str) -> Result<Option<CollectionPermissions>, AppError>;

    /// Delete permissions for a collection (revert to defaults)
    async fn delete_permissions(&self, collection: &str) -> Result<(), AppError>;

    /// List all collections that have custom permissions
    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError>;
}

/// Default permission service implementation
pub struct DefaultPermissionService;

#[async_trait]
impl PermissionService for DefaultPermissionService {
    /// Check if a user is authenticated
    fn check_authentication(
        &self,
        context: &PermissionContext,
    ) -> Result<bool, AppError> {
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
            if matches!(rule.permission, PermissionLevel::None) {
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
            PermissionLevel::None => Ok(false),
            PermissionLevel::Public => Ok(true),
            PermissionLevel::AuthenticatedOnly => Ok(context.user_claims.is_some()),
            PermissionLevel::SuperuserOnly => {
                Ok(matches!(context.user_role(), Some(UserRole::Superuser)))
            }
            PermissionLevel::Rule(rule_expr) => {
                // Import the rule evaluator from the rules module
                use super::rules::RuleEvaluator;
                let evaluator = RuleEvaluator::new(context);
                evaluator.evaluate(rule_expr)
            }
        }
    }

    /// Store permissions for a collection (default implementation returns error)
    async fn store_permissions(&self, _permissions: &CollectionPermissions) -> Result<(), AppError> {
        Err(AppError::internal("Permission storage not implemented in default service"))
    }

    /// Get permissions for a collection (default implementation returns None)
    async fn get_permissions(&self, _collection: &str) -> Result<Option<CollectionPermissions>, AppError> {
        Ok(None)
    }

    /// Delete permissions for a collection (default implementation returns error)
    async fn delete_permissions(&self, _collection: &str) -> Result<(), AppError> {
        Err(AppError::internal("Permission deletion not implemented in default service"))
    }

    /// List all collections that have custom permissions (default implementation returns empty)
    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError> {
        Ok(Vec::new())
    }
} 