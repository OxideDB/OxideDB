//! JWT token management
//!
//! This module handles JWT token creation, validation, and claims management
//! for the authentication system.

use crate::AppError;
use super::types::{UserRole, AuthServiceConfig};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// Create new JWT claims
    pub fn new(user_id: String, email: String, role: String, auth_collection: String, expiry_hours: i64) -> Self {
        let now = chrono::Utc::now().timestamp();
        let exp = now + (expiry_hours * 3600);

        Self {
            sub: user_id,
            email,
            role,
            auth_collection,
            exp,
            iat: now,
            custom: HashMap::new(),
        }
    }

    /// Add a custom claim
    pub fn add_custom_claim(&mut self, key: String, value: serde_json::Value) {
        self.custom.insert(key, value);
    }

    /// Get the user role
    pub fn user_role(&self) -> Result<UserRole, String> {
        self.role.parse()
    }
}

/// JWT token management service
pub struct JwtService<'a> {
    config: &'a AuthServiceConfig,
}

impl<'a> JwtService<'a> {
    /// Create a new JWT service
    pub fn new(config: &'a AuthServiceConfig) -> Self {
        Self { config }
    }

    /// Generate a JWT token for a user
    pub fn generate_token(&self, user_id: String, email: String, role: UserRole, auth_collection: String) -> Result<String, AppError> {
        let claims = Claims::new(
            user_id,
            email,
            role.to_string(),
            auth_collection,
            self.config.token_expiry_hours,
        );

        self.generate_token_with_claims(claims)
    }

    /// Generate a JWT token with custom claims
    pub fn generate_token_with_claims(&self, claims: Claims) -> Result<String, AppError> {
        let header = Header::default();
        let encoding_key = EncodingKey::from_secret(self.config.jwt_secret.as_ref());

        encode(&header, &claims, &encoding_key)
            .map_err(|e| AppError::internal(format!("Token generation failed: {}", e)))
    }

    /// Verify and decode a JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let decoding_key = DecodingKey::from_secret(self.config.jwt_secret.as_ref());
        let validation = Validation::default();

        decode::<Claims>(token, &decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| AppError::auth(format!("Token verification failed: {}", e)))
    }
} 