//! Authentication utilities for OxideDB
//!
//! This module provides password hashing and JWT token management
//! for the authentication system, plus comprehensive permission rules
//! for API access control similar to PocketBase.

use crate::AppError;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::collections::HashMap;
use ts_rs::TS;
use async_trait;

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,    // Subject (user id)
    pub email: String,  // User email
    pub exp: i64,       // Expiration time (timestamp)
    pub iat: i64,       // Issued at time (timestamp)
    pub role: String,   // User role (user or superuser)
}

impl Claims {
    /// Create new claims for a user
    pub fn new(user_id: String, email: String, role: String, expires_in_hours: i64) -> Self {
        let now = Utc::now();
        let exp = now + Duration::hours(expires_in_hours);
        
        Self {
            sub: user_id,
            email,
            exp: exp.timestamp(),
            iat: now.timestamp(),
            role,
        }
    }
}

/// User role enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    User,
    Superuser,
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Superuser => write!(f, "superuser"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(UserRole::User),
            "superuser" => Ok(UserRole::Superuser),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

/// CRUD operation types for permission rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum CrudOperation {
    Create,
    Read,
    Update,
    Delete,
    List,
}

impl fmt::Display for CrudOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrudOperation::Create => write!(f, "create"),
            CrudOperation::Read => write!(f, "read"),
            CrudOperation::Update => write!(f, "update"),
            CrudOperation::Delete => write!(f, "delete"),
            CrudOperation::List => write!(f, "list"),
        }
    }
}

/// Permission level for accessing resources
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    /// No access allowed
    None,
    /// Only superusers can access
    SuperuserOnly,
    /// Only authenticated users can access
    AuthenticatedOnly,
    /// Public access allowed (no authentication required)
    Public,
    /// Custom rule expression (e.g., "@request.auth.id = @record.owner_id")
    Rule(String),
}

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
    pub fn new_public(collection: String) -> Self {
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

    /// Set permission for a specific operation
    pub fn set_operation_permission(&mut self, operation: CrudOperation, permission: PermissionLevel, filter: Option<String>) {
        self.rules.insert(operation.clone(), OperationRule {
            operation,
            permission,
            filter,
        });
        self.update_timestamp();
    }

    /// Get permission rule for a specific operation
    pub fn get_operation_rule(&self, operation: &CrudOperation) -> Option<&OperationRule> {
        self.rules.get(operation)
    }

    /// Check if an operation is allowed for a given user role
    pub fn is_operation_allowed(&self, operation: &CrudOperation, user_role: Option<&UserRole>) -> bool {
        let rule = match self.rules.get(operation) {
            Some(rule) => rule,
            None => return false, // No rule means no access
        };

        match &rule.permission {
            PermissionLevel::None => false,
            PermissionLevel::Public => true,
            PermissionLevel::AuthenticatedOnly => user_role.is_some(),
            PermissionLevel::SuperuserOnly => {
                matches!(user_role, Some(UserRole::Superuser))
            }
            PermissionLevel::Rule(_) => {
                // For custom rules, we need additional context evaluation
                // For now, we'll require at least authentication
                user_role.is_some()
            }
        }
    }

    /// Update the updated_at timestamp
    pub fn update_timestamp(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
    }
}

/// Context for evaluating permission rules
#[derive(Debug, Clone)]
pub struct PermissionContext {
    /// The authenticated user's claims (if any)
    pub user_claims: Option<Claims>,
    /// The record being accessed (for rules like "@request.auth.id = @record.owner_id")
    pub record: Option<serde_json::Value>,
    /// Request data (for create/update operations)
    pub request_data: Option<serde_json::Value>,
    /// Collection being accessed
    pub collection: String,
    /// Operation being performed
    pub operation: CrudOperation,
}

impl PermissionContext {
    /// Create a new permission context
    pub fn new(
        collection: String,
        operation: CrudOperation,
        user_claims: Option<Claims>,
        record: Option<serde_json::Value>,
        request_data: Option<serde_json::Value>,
    ) -> Self {
        Self {
            user_claims,
            record,
            request_data,
            collection,
            operation,
        }
    }

    /// Get user role from claims
    pub fn user_role(&self) -> Option<UserRole> {
        self.user_claims.as_ref()
            .and_then(|claims| claims.role.parse().ok())
    }

    /// Get user ID from claims
    pub fn user_id(&self) -> Option<&str> {
        self.user_claims.as_ref().map(|claims| claims.sub.as_str())
    }
}

/// Permission service trait for database operations
///
/// This trait abstracts permission storage operations to avoid circular dependencies
/// between oxide-core and oxide-db.
#[async_trait::async_trait]
pub trait PermissionService: Send + Sync {
    /// Store permissions for a collection
    async fn store_permissions(&self, permissions: &CollectionPermissions) -> Result<(), AppError>;

    /// Get permissions for a collection
    async fn get_permissions(&self, collection: &str) -> Result<Option<CollectionPermissions>, AppError>;

    /// Delete permissions for a collection (revert to defaults)
    async fn delete_permissions(&self, collection: &str) -> Result<(), AppError>;

    /// List all collections that have custom permissions
    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError>;
}

/// Authentication service for password hashing and JWT management
pub struct AuthService {
    jwt_secret: String,
}

impl AuthService {
    /// Create a new AuthService with a JWT secret
    pub fn new(jwt_secret: String) -> Self {
        Self { jwt_secret }
    }

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String, AppError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::auth(format!("Failed to hash password: {}", e)))?;
        
        Ok(password_hash.to_string())
    }

    /// Verify a password against a hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::auth(format!("Invalid password hash: {}", e)))?;
        
        let argon2 = Argon2::default();
        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Generate a JWT token for a user
    pub fn generate_token(&self, user_id: String, email: String, role: UserRole) -> Result<String, AppError> {
        let claims = Claims::new(user_id, email, role.to_string(), 24); // 24 hours expiry
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| AppError::auth(format!("Failed to generate token: {}", e)))?;
        
        Ok(token)
    }

    /// Verify and decode a JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|e| AppError::auth(format!("Invalid token: {}", e)))?;
        
        Ok(token_data.claims)
    }

    /// Evaluate a permission rule in the given context
    pub fn evaluate_permission_rule(&self, rule: &str, context: &PermissionContext) -> Result<bool, AppError> {
        // Simple rule evaluation - in a real system you'd want a proper expression parser
        // For now, we'll support basic patterns like "@request.auth.id = @record.owner_id"
        
        if rule.is_empty() {
            return Ok(true);
        }

        // Handle "@request.auth.id = @record.owner_id" pattern
        if rule.contains("@request.auth.id") && rule.contains("@record.owner_id") {
            let user_id = context.user_id().ok_or_else(|| AppError::auth("No authenticated user"))?;
            let record = context.record.as_ref().ok_or_else(|| AppError::auth("No record context"))?;
            let owner_id = record.get("owner_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::auth("Record has no owner_id"))?;
            
            return Ok(user_id == owner_id);
        }

        // Handle "@request.auth.role = 'admin'" pattern
        if rule.contains("@request.auth.role") {
            if let Some(role) = context.user_role() {
                if rule.contains("'superuser'") || rule.contains("\"superuser\"") {
                    return Ok(role == UserRole::Superuser);
                }
                if rule.contains("'user'") || rule.contains("\"user\"") {
                    return Ok(role == UserRole::User);
                }
            }
            return Ok(false);
        }

        // Default: if we can't parse the rule, require authentication
        Ok(context.user_claims.is_some())
    }

    /// Check if a user can perform an operation on a collection
    pub fn check_permission(
        &self,
        permissions: &CollectionPermissions,
        context: &PermissionContext,
    ) -> Result<bool, AppError> {
        let rule = match permissions.get_operation_rule(&context.operation) {
            Some(rule) => rule,
            None => return Ok(false), // No rule means no access
        };

        match &rule.permission {
            PermissionLevel::None => Ok(false),
            PermissionLevel::Public => Ok(true),
            PermissionLevel::AuthenticatedOnly => Ok(context.user_claims.is_some()),
            PermissionLevel::SuperuserOnly => {
                Ok(matches!(context.user_role(), Some(UserRole::Superuser)))
            }
            PermissionLevel::Rule(rule_expr) => {
                self.evaluate_permission_rule(rule_expr, context)
            }
        }
    }
}

/// Create default auth collections schemas
pub fn create_auth_collections() -> (crate::collection::CollectionSchema, crate::collection::CollectionSchema) {
    use crate::collection::{CollectionSchema, CollectionType, FieldDefinition};
    use crate::field_types::FieldType;
    use std::collections::HashMap;
    
    // Users collection schema
    let mut users_schema = CollectionSchema::new("users".to_string(), CollectionType::Auth);
    let mut users_fields = HashMap::new();
    
    users_fields.insert("email".to_string(), FieldDefinition {
        field_type: FieldType::Email,
        required: true,
        unique: true, // Email must be unique
        default: None,
        validation: None,
    });
    
    users_fields.insert("password".to_string(), FieldDefinition {
        field_type: FieldType::Password,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    
    users_fields.insert("verified".to_string(), FieldDefinition {
        field_type: FieldType::Boolean,
        required: false,
        unique: false,
        default: Some(serde_json::json!(false)),
        validation: None,
    });
    
    users_schema.fields = users_fields;
    
    // Superusers collection schema
    let mut superusers_schema = CollectionSchema::new("superusers".to_string(), CollectionType::Auth);
    let mut superusers_fields = HashMap::new();
    
    superusers_fields.insert("email".to_string(), FieldDefinition {
        field_type: FieldType::Email,
        required: true,
        unique: true, // Email must be unique
        default: None,
        validation: None,
    });
    
    superusers_fields.insert("password".to_string(), FieldDefinition {
        field_type: FieldType::Password,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    
    superusers_fields.insert("verified".to_string(), FieldDefinition {
        field_type: FieldType::Boolean,
        required: false,
        unique: false,
        default: Some(serde_json::json!(true)),
        validation: None,
    });
    
    superusers_schema.fields = superusers_fields;
    
    (users_schema, superusers_schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let auth_service = AuthService::new("test_secret".to_string());
        
        let password = "test_password_123";
        let hash = auth_service.hash_password(password).unwrap();
        
        assert!(auth_service.verify_password(password, &hash).unwrap());
        assert!(!auth_service.verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_jwt_tokens() {
        let auth_service = AuthService::new("test_secret".to_string());
        
        let user_id = "user123".to_string();
        let email = "test@example.com".to_string();
        let role = UserRole::User;
        
        let token = auth_service.generate_token(user_id.clone(), email.clone(), role).unwrap();
        let claims = auth_service.verify_token(&token).unwrap();
        
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
        assert_eq!(claims.role, "user");
    }

    #[test]
    fn test_collection_permissions() {
        let mut permissions = CollectionPermissions::new("test_collection".to_string());
        
        // Default should be superuser only
        assert!(!permissions.is_operation_allowed(&CrudOperation::Create, None));
        assert!(!permissions.is_operation_allowed(&CrudOperation::Create, Some(&UserRole::User)));
        assert!(permissions.is_operation_allowed(&CrudOperation::Create, Some(&UserRole::Superuser)));
        
        // Change to public
        permissions.set_operation_permission(CrudOperation::Create, PermissionLevel::Public, None);
        assert!(permissions.is_operation_allowed(&CrudOperation::Create, None));
        assert!(permissions.is_operation_allowed(&CrudOperation::Create, Some(&UserRole::User)));
        assert!(permissions.is_operation_allowed(&CrudOperation::Create, Some(&UserRole::Superuser)));
        
        // Change to authenticated only
        permissions.set_operation_permission(CrudOperation::Read, PermissionLevel::AuthenticatedOnly, None);
        assert!(!permissions.is_operation_allowed(&CrudOperation::Read, None));
        assert!(permissions.is_operation_allowed(&CrudOperation::Read, Some(&UserRole::User)));
        assert!(permissions.is_operation_allowed(&CrudOperation::Read, Some(&UserRole::Superuser)));
    }

    #[test]
    fn test_permission_rule_evaluation() {
        let auth_service = AuthService::new("test_secret".to_string());
        
        // Test owner-based rule
        let claims = Claims::new("user123".to_string(), "test@example.com".to_string(), "user".to_string(), 24);
        let record = serde_json::json!({"id": "record1", "owner_id": "user123"});
        let context = PermissionContext::new(
            "test_collection".to_string(),
            CrudOperation::Read,
            Some(claims),
            Some(record),
            None,
        );
        
        assert!(auth_service.evaluate_permission_rule("@request.auth.id = @record.owner_id", &context).unwrap());
        
        // Test with different owner
        let record_different_owner = serde_json::json!({"id": "record1", "owner_id": "user456"});
        let context_different = PermissionContext::new(
            "test_collection".to_string(),
            CrudOperation::Read,
            context.user_claims.clone(),
            Some(record_different_owner),
            None,
        );
        
        assert!(!auth_service.evaluate_permission_rule("@request.auth.id = @record.owner_id", &context_different).unwrap());
    }
} 