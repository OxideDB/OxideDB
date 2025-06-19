//! Authentication types and enums
//!
//! This module contains all the core authentication types, enums, and configuration
//! structures used throughout the authentication system.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;
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
    /// Whether refresh tokens are enabled for this collection
    pub refresh_tokens_enabled: bool,
    /// Whether refresh tokens are required for this collection (enforced for superusers)
    pub refresh_tokens_required: bool,
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
            refresh_tokens_enabled: false,
            refresh_tokens_required: false,
        }
    }
}

/// Authentication service configuration
#[derive(Debug)]
pub struct AuthServiceConfig {
    /// JWT secret for token signing
    pub jwt_secret: String,
    /// Token expiration time in hours
    pub token_expiry_hours: i64,
    /// Refresh token expiration time in days
    pub refresh_token_expiry_days: i64,
    /// Authentication configurations per collection (with interior mutability)
    pub auth_collections: RwLock<HashMap<String, AuthCollectionConfig>>,
}

impl AuthServiceConfig {
    /// Create a new auth service configuration
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret,
            token_expiry_hours: 24,
            refresh_token_expiry_days: 30,
            auth_collections: RwLock::new(HashMap::new()),
        }
    }

    /// Add an auth collection configuration
    pub fn add_auth_collection(&self, config: AuthCollectionConfig) {
        let mut collections = self.auth_collections.write().unwrap();
        collections.insert(config.collection.clone(), config);
    }

    /// Get auth collection configuration
    pub fn get_auth_collection(&self, collection: &str) -> Option<AuthCollectionConfig> {
        let collections = self.auth_collections.read().unwrap();
        collections.get(collection).cloned()
    }

    /// List all auth collection names
    pub fn list_auth_collections(&self) -> Vec<String> {
        let collections = self.auth_collections.read().unwrap();
        collections.keys().cloned().collect()
    }

    /// Check if a collection is configured for authentication
    pub fn is_auth_collection(&self, collection: &str) -> bool {
        let collections = self.auth_collections.read().unwrap();
        collections.contains_key(collection)
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
    /// Custom rule expression (e.g., "@req.user.id = @record.user_id")
    Rule(String),
} 