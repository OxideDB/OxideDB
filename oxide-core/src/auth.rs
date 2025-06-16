//! Authentication and authorization system
//!
//! This module provides the core authentication and authorization functionality
//! for OxideDB, including JWT token management, password hashing, and permission
//! checking. It supports multiple authentication methods and collection-based
//! authentication configuration.

use crate::{AppError, CollectionSchema, CollectionType};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use async_trait::async_trait;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use ts_rs::TS;

/// User roles in the system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Regular user with basic permissions
    User,
    /// Superuser with administrative privileges
    Superuser,
    /// Custom role (for future extensibility)
    Custom(String),
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Superuser => write!(f, "superuser"),
            UserRole::Custom(role) => write!(f, "{}", role),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(UserRole::User),
            "superuser" => Ok(UserRole::Superuser),
            _ => Ok(UserRole::Custom(s.to_string())),
        }
    }
}

/// Authentication method types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// Email and password authentication
    EmailPassword,
    /// OAuth authentication (future)
    OAuth { provider: String },
    /// SAML authentication (future)
    Saml { provider: String },
    /// LDAP authentication (future)
    Ldap { server: String },
}

/// Authentication configuration for a collection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AuthCollectionConfig {
    /// Collection name
    pub collection: String,
    /// Authentication method
    pub auth_method: AuthMethod,
    /// Field name for the identifier (e.g., "email", "username")
    pub identifier_field: String,
    /// Field name for the credential (e.g., "password")
    pub credential_field: String,
    /// Default user role for this collection
    pub default_role: UserRole,
    /// Whether registration is enabled for this collection
    pub registration_enabled: bool,
    /// Whether email verification is required
    pub email_verification_required: bool,
    /// Custom fields to include in JWT claims
    pub custom_claim_fields: Vec<String>,
}

impl Default for AuthCollectionConfig {
    fn default() -> Self {
        Self {
            collection: String::new(),
            auth_method: AuthMethod::EmailPassword,
            identifier_field: "email".to_string(),
            credential_field: "password".to_string(),
            default_role: UserRole::User,
            registration_enabled: true,
            email_verification_required: false,
            custom_claim_fields: Vec::new(),
        }
    }
}

/// Authentication service configuration
#[derive(Debug, Clone)]
pub struct AuthServiceConfig {
    /// JWT secret for token signing
    pub jwt_secret: String,
    /// Token expiration time in hours
    pub token_expiry_hours: i64,
    /// Authentication configurations per collection
    pub auth_collections: HashMap<String, AuthCollectionConfig>,
}

impl AuthServiceConfig {
    /// Create a new auth service configuration
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret,
            token_expiry_hours: 24,
            auth_collections: HashMap::new(),
        }
    }

    /// Add an auth collection configuration
    pub fn add_auth_collection(&mut self, config: AuthCollectionConfig) {
        self.auth_collections.insert(config.collection.clone(), config);
    }

    /// Get auth collection configuration
    pub fn get_auth_collection(&self, collection: &str) -> Option<&AuthCollectionConfig> {
        self.auth_collections.get(collection)
    }

    /// List all auth collection names
    pub fn list_auth_collections(&self) -> Vec<String> {
        self.auth_collections.keys().cloned().collect()
    }

    /// Check if a collection is configured for authentication
    pub fn is_auth_collection(&self, collection: &str) -> bool {
        self.auth_collections.contains_key(collection)
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

/// JWT Claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Email address
    pub email: String,
    /// User role
    pub role: String,
    /// Collection the user authenticated from
    pub auth_collection: String,
    /// Expiration timestamp
    pub exp: i64,
    /// Issued at timestamp
    pub iat: i64,
    /// Custom claims from the auth collection config
    pub custom: HashMap<String, serde_json::Value>,
}

impl Claims {
    /// Create new claims with default expiration
    pub fn new(user_id: String, email: String, role: String, auth_collection: String, expiry_hours: i64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            sub: user_id,
            email,
            role,
            auth_collection,
            exp: now + (expiry_hours * 3600),
            iat: now,
            custom: HashMap::new(),
        }
    }

    /// Add a custom claim
    pub fn add_custom_claim(&mut self, key: String, value: serde_json::Value) {
        self.custom.insert(key, value);
    }

    /// Get user role as enum
    pub fn user_role(&self) -> Result<UserRole, String> {
        self.role.parse()
    }
}

/// Authentication service for handling JWT tokens and password hashing
#[derive(Clone)]
pub struct AuthService {
    config: AuthServiceConfig,
}

impl AuthService {
    /// Create a new AuthService with configuration
    pub fn new(config: AuthServiceConfig) -> Self {
        Self { config }
    }

    /// Get the service configuration
    pub fn config(&self) -> &AuthServiceConfig {
        &self.config
    }

    /// Update auth collection configurations from database schemas
    pub fn update_auth_collections(&mut self, schemas: &[CollectionSchema]) {
        // Clear existing auth collections
        self.config.auth_collections.clear();

        // Add configurations for all auth collections
        for schema in schemas {
            if schema.collection_type == CollectionType::Auth {
                let default_role = if schema.name.contains("superuser") || schema.name.contains("admin") {
                    UserRole::Superuser
                } else {
                    UserRole::User
                };

                // Auto-detect identifier and credential fields from schema
                let identifier_field = if schema.fields.contains_key("username") {
                    "username".to_string()
                } else {
                    "email".to_string() // default to email
                };

                let credential_field = "password".to_string(); // always use password

                // Check if email verification field exists
                let email_verification_required = schema.fields.contains_key("verified") || 
                                                 schema.fields.contains_key("email_verified");

                let config = AuthCollectionConfig {
                    collection: schema.name.clone(),
                    default_role,
                    identifier_field,
                    credential_field,
                    email_verification_required,
                    ..Default::default()
                };

                self.config.add_auth_collection(config);
            }
        }
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
    pub fn generate_token(&self, user_id: String, email: String, role: UserRole, auth_collection: String) -> Result<String, AppError> {
        let claims = Claims::new(user_id, email, role.to_string(), auth_collection.clone(), self.config.token_expiry_hours);
        
        // Add custom claims if configured for this auth collection
        if let Some(_auth_config) = self.config.get_auth_collection(&auth_collection) {
            // Custom claim fields would be populated from the user record data
            // This is handled in the authentication flow
        }
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_ref()),
        )
        .map_err(|e| AppError::auth(format!("Failed to generate token: {}", e)))?;
        
        Ok(token)
    }

    /// Generate a JWT token with custom claims
    pub fn generate_token_with_claims(&self, claims: Claims) -> Result<String, AppError> {
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.config.jwt_secret.as_ref()),
        )
        .map_err(|e| AppError::auth(format!("Failed to generate token: {}", e)))?;
        
        Ok(token)
    }

    /// Verify and decode a JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.config.jwt_secret.as_ref()),
            &Validation::default(),
        )
        .map_err(|e| AppError::auth(format!("Invalid token: {}", e)))?;

        Ok(token_data.claims)
    }
}

/// Permission context for authorization checks
pub struct PermissionContext {
    /// The user's JWT claims (if authenticated)
    pub user_claims: Option<Claims>,
    /// The CRUD operation being performed
    pub operation: CrudOperation,
    /// The collection being accessed
    pub collection: String,
    /// The specific record ID (if applicable)
    pub record_id: Option<String>,
    /// Additional request metadata
    pub metadata: HashMap<String, serde_json::Value>,
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
        }
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
        matches!(self.user_role(), Some(UserRole::Superuser))
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

impl DefaultPermissionService {
    /// Evaluate a custom permission rule (placeholder for future implementation)
    fn evaluate_permission_rule(
        &self,
        _rule_expr: &str,
        _context: &PermissionContext,
    ) -> Result<bool, AppError> {
        // TODO: Implement rule evaluation engine
        // For now, default to requiring authentication
        Ok(_context.user_claims.is_some())
    }
}

/// Create default auth collections schemas (legacy support)
pub fn create_auth_collections() -> (CollectionSchema, CollectionSchema) {
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
        default: Some(serde_json::json!(true)), // Superusers are verified by default
        validation: None,
    });
    
    superusers_schema.fields = superusers_fields;
    
    (users_schema, superusers_schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_role_parsing() {
        assert_eq!("user".parse::<UserRole>().unwrap(), UserRole::User);
        assert_eq!("superuser".parse::<UserRole>().unwrap(), UserRole::Superuser);
        assert_eq!("admin".parse::<UserRole>().unwrap(), UserRole::Custom("admin".to_string()));
    }

    #[test]
    fn test_auth_service_config() {
        let mut config = AuthServiceConfig::new("secret".to_string());
        
        let auth_config = AuthCollectionConfig {
            collection: "users".to_string(),
            auth_method: AuthMethod::EmailPassword,
            identifier_field: "email".to_string(),
            credential_field: "password".to_string(),
            default_role: UserRole::User,
            registration_enabled: true,
            email_verification_required: false,
            custom_claim_fields: vec!["name".to_string()],
        };
        
        config.add_auth_collection(auth_config);
        
        assert!(config.is_auth_collection("users"));
        assert!(!config.is_auth_collection("posts"));
        assert_eq!(config.list_auth_collections(), vec!["users"]);
    }

    #[test]
    fn test_claims_creation() {
        let claims = Claims::new(
            "user123".to_string(),
            "test@example.com".to_string(),
            "user".to_string(),
            "users".to_string(),
            24,
        );
        
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "user");
        assert_eq!(claims.auth_collection, "users");
        assert!(claims.exp > claims.iat);
    }
} 