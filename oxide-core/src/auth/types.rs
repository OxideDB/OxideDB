//! Authentication types and enums
//!
//! This module contains all the core authentication types, enums, and configuration
//! structures used throughout the authentication system.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::RwLock;
use ts_rs::TS;

const DEFAULT_JWT_KEY_ID: &str = "default";

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
    /// JWT signing key ring used for key rotation.
    pub jwt_key_ring: JwtKeyRing,
    /// Token expiration time in hours
    pub token_expiry_hours: i64,
    /// Refresh token expiration time in days
    pub refresh_token_expiry_days: i64,
    /// Optional JWT issuer that generated tokens must contain.
    pub jwt_issuer: Option<String>,
    /// Optional JWT audience that generated tokens must contain.
    pub jwt_audience: Option<String>,
    /// Authentication configurations per collection (with interior mutability)
    pub auth_collections: RwLock<HashMap<String, AuthCollectionConfig>>,
}

impl AuthServiceConfig {
    /// Create a new auth service configuration
    pub fn new(jwt_secret: String) -> Self {
        let jwt_key_ring = JwtKeyRing::single(jwt_secret.clone());
        Self::with_jwt_key_ring(jwt_key_ring)
    }

    /// Create a new auth service configuration with a JWT key ring.
    pub fn with_jwt_key_ring(jwt_key_ring: JwtKeyRing) -> Self {
        let jwt_secret = jwt_key_ring.active_secret().to_string();
        Self {
            jwt_secret,
            jwt_key_ring,
            token_expiry_hours: 24,
            refresh_token_expiry_days: 30,
            jwt_issuer: std::env::var("OXIDEDB_JWT_ISSUER")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            jwt_audience: std::env::var("OXIDEDB_JWT_AUDIENCE")
                .ok()
                .filter(|value| !value.trim().is_empty()),
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

/// A single HMAC signing key accepted for JWT verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JwtSigningKey {
    key_id: String,
    secret: String,
}

impl JwtSigningKey {
    /// Create a JWT signing key with a key identifier and secret.
    pub fn new(key_id: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            key_id: key_id.into().trim().to_string(),
            secret: secret.into(),
        }
    }

    /// Return the key identifier used in the JWT `kid` header.
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Return the HMAC secret associated with this key.
    pub fn secret(&self) -> &str {
        &self.secret
    }
}

/// JWT signing and verification keys, with one active signing key.
#[derive(Debug, Clone)]
pub struct JwtKeyRing {
    active_key_id: String,
    key_order: Vec<String>,
    keys: HashMap<String, String>,
}

impl JwtKeyRing {
    /// Create a single-key JWT key ring for legacy deployments.
    pub fn single(secret: String) -> Self {
        let key = JwtSigningKey::new(DEFAULT_JWT_KEY_ID, secret);
        let active_key_id = key.key_id().to_string();
        let mut keys = HashMap::new();
        keys.insert(active_key_id.clone(), key.secret().to_string());

        Self {
            active_key_id: active_key_id.clone(),
            key_order: vec![active_key_id],
            keys,
        }
    }

    /// Create a JWT key ring from an active key id and accepted keys.
    pub fn new(
        active_key_id: impl Into<String>,
        keys: Vec<JwtSigningKey>,
    ) -> Result<Self, crate::AppError> {
        let active_key_id = active_key_id.into().trim().to_string();
        validate_jwt_key_id(&active_key_id)?;

        let mut key_map = HashMap::new();
        let mut key_order = Vec::new();
        let mut seen = HashSet::new();

        for key in keys {
            validate_jwt_key_id(key.key_id())?;

            if key.secret().is_empty() {
                return Err(crate::AppError::validation(
                    "jwt_key_secret",
                    &format!("JWT key '{}' secret cannot be empty", key.key_id()),
                ));
            }

            if !seen.insert(key.key_id().to_string()) {
                return Err(crate::AppError::validation(
                    "jwt_key_id",
                    &format!("Duplicate JWT key id '{}'", key.key_id()),
                ));
            }

            key_order.push(key.key_id().to_string());
            key_map.insert(key.key_id().to_string(), key.secret().to_string());
        }

        if !key_map.contains_key(&active_key_id) {
            return Err(crate::AppError::validation(
                "jwt_key_id",
                &format!(
                    "Active JWT key id '{}' is not present in the key ring",
                    active_key_id
                ),
            ));
        }

        Ok(Self {
            active_key_id,
            key_order,
            keys: key_map,
        })
    }

    /// Return the active JWT key id used when signing new tokens.
    pub fn active_key_id(&self) -> &str {
        &self.active_key_id
    }

    /// Return the active JWT signing secret.
    pub fn active_secret(&self) -> &str {
        self.keys
            .get(&self.active_key_id)
            .map(String::as_str)
            .unwrap_or_default()
    }

    /// Return the secret for a specific JWT key id.
    pub fn secret_for(&self, key_id: &str) -> Option<&str> {
        self.keys.get(key_id).map(String::as_str)
    }

    /// Return accepted verification secrets, with the active key first.
    pub fn accepted_secrets(&self) -> Vec<&str> {
        let mut secrets = Vec::new();

        if let Some(secret) = self.secret_for(&self.active_key_id) {
            secrets.push(secret);
        }

        for key_id in &self.key_order {
            if key_id != &self.active_key_id {
                if let Some(secret) = self.secret_for(key_id) {
                    secrets.push(secret);
                }
            }
        }

        secrets
    }

    /// Return the number of accepted JWT keys.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Return all accepted JWT signing keys.
    pub fn signing_keys(&self) -> Vec<JwtSigningKey> {
        self.key_order
            .iter()
            .filter_map(|key_id| {
                self.secret_for(key_id)
                    .map(|secret| JwtSigningKey::new(key_id.clone(), secret.to_string()))
            })
            .collect()
    }
}

fn validate_jwt_key_id(key_id: &str) -> Result<(), crate::AppError> {
    if key_id.is_empty() {
        return Err(crate::AppError::validation(
            "jwt_key_id",
            "JWT key id cannot be empty",
        ));
    }

    if key_id.len() > 128 {
        return Err(crate::AppError::validation(
            "jwt_key_id",
            "JWT key id cannot be longer than 128 characters",
        ));
    }

    if !key_id
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(crate::AppError::validation(
            "jwt_key_id",
            "JWT key id may only contain ASCII letters, digits, '.', '_' and '-'",
        ));
    }

    Ok(())
}

/// CRUD operation types for permission rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum CrudOperation {
    Create,
    Read,
    Update,
    Delete,
    List,
}

/// Auth-specific operation types for permission rules
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum AuthOperation {
    Login,
    Register,
    TokenValidation,
    TokenRefresh,
    Logout,
    GetCurrentUser,
    ListAuthCollections,
}

/// Combined operation type that can be either CRUD or Auth
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Crud(CrudOperation),
    Auth(AuthOperation),
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

impl fmt::Display for AuthOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthOperation::Login => write!(f, "login"),
            AuthOperation::Register => write!(f, "register"),
            AuthOperation::TokenValidation => write!(f, "token_validation"),
            AuthOperation::TokenRefresh => write!(f, "token_refresh"),
            AuthOperation::Logout => write!(f, "logout"),
            AuthOperation::GetCurrentUser => write!(f, "get_current_user"),
            AuthOperation::ListAuthCollections => write!(f, "list_auth_collections"),
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operation::Crud(op) => write!(f, "{}", op),
            Operation::Auth(op) => write!(f, "{}", op),
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
