//! JWT token management
//!
//! This module handles JWT token creation, validation, and claims management
//! for the authentication system.

use super::types::{AuthServiceConfig, UserRole};
use crate::AppError;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// JWT Claims structure for access tokens
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
    /// Token type ("access")
    pub typ: String,
    /// Custom claims from the auth collection config
    pub custom: HashMap<String, serde_json::Value>,
}

/// JWT Claims structure for refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshClaims {
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
    /// Token type ("refresh")
    pub typ: String,
    /// Token version/jti (for invalidation)
    pub jti: String,
}

impl Claims {
    /// Create new JWT claims for access token
    pub fn new(
        user_id: String,
        email: String,
        role: String,
        auth_collection: String,
        expiry_hours: i64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        let exp = now + (expiry_hours * 3600);

        Self {
            sub: user_id,
            email,
            role,
            auth_collection,
            exp,
            iat: now,
            typ: "access".to_string(),
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

    /// Check if token is an access token
    pub fn is_access_token(&self) -> bool {
        self.typ == "access"
    }
}

impl RefreshClaims {
    /// Create new JWT claims for refresh token
    pub fn new(
        user_id: String,
        email: String,
        role: String,
        auth_collection: String,
        expiry_days: i64,
    ) -> Self {
        let now = chrono::Utc::now().timestamp();
        let exp = now + (expiry_days * 24 * 3600);

        Self {
            sub: user_id,
            email,
            role,
            auth_collection,
            exp,
            iat: now,
            typ: "refresh".to_string(),
            jti: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Get the user role
    pub fn user_role(&self) -> Result<UserRole, String> {
        self.role.parse()
    }

    /// Check if token is a refresh token
    pub fn is_refresh_token(&self) -> bool {
        self.typ == "refresh"
    }
}

/// Response containing both access and refresh tokens
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPair {
    /// Access token for API requests
    pub access_token: String,
    /// Refresh token for obtaining new access tokens
    pub refresh_token: String,
    /// Access token expiration time in seconds
    pub access_token_expires_in: i64,
    /// Refresh token expiration time in seconds
    pub refresh_token_expires_in: i64,
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

    /// Generate a JWT access token for a user
    pub fn generate_token(
        &self,
        user_id: String,
        email: String,
        role: UserRole,
        auth_collection: String,
    ) -> Result<String, AppError> {
        let claims = Claims::new(
            user_id,
            email,
            role.to_string(),
            auth_collection,
            self.config.token_expiry_hours,
        );

        self.generate_token_with_claims(claims)
    }

    /// Generate a JWT access token with custom claims
    pub fn generate_token_with_claims(&self, claims: Claims) -> Result<String, AppError> {
        let header = Header::default();
        let encoding_key = EncodingKey::from_secret(self.config.jwt_secret.as_ref());

        encode(&header, &claims, &encoding_key)
            .map_err(|e| AppError::internal(format!("Token generation failed: {}", e)))
    }

    /// Generate a refresh token for a user
    pub fn generate_refresh_token(
        &self,
        user_id: String,
        email: String,
        role: UserRole,
        auth_collection: String,
    ) -> Result<String, AppError> {
        let claims = RefreshClaims::new(
            user_id,
            email,
            role.to_string(),
            auth_collection,
            self.config.refresh_token_expiry_days,
        );

        let header = Header::default();
        let encoding_key = EncodingKey::from_secret(self.config.jwt_secret.as_ref());

        encode(&header, &claims, &encoding_key)
            .map_err(|e| AppError::internal(format!("Refresh token generation failed: {}", e)))
    }

    /// Generate both access and refresh tokens
    pub fn generate_token_pair(
        &self,
        user_id: String,
        email: String,
        role: UserRole,
        auth_collection: String,
    ) -> Result<TokenPair, AppError> {
        let access_token = self.generate_token(
            user_id.clone(),
            email.clone(),
            role.clone(),
            auth_collection.clone(),
        )?;
        let refresh_token = self.generate_refresh_token(user_id, email, role, auth_collection)?;

        Ok(TokenPair {
            access_token,
            refresh_token,
            access_token_expires_in: self.config.token_expiry_hours * 3600,
            refresh_token_expires_in: self.config.refresh_token_expiry_days * 24 * 3600,
        })
    }

    /// Verify and decode a JWT access token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let decoding_key = DecodingKey::from_secret(self.config.jwt_secret.as_ref());
        let validation = Validation::default();

        let decoded = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| AppError::auth(format!("Token verification failed: {}", e)))?;

        // Verify this is an access token
        if !decoded.claims.is_access_token() {
            return Err(AppError::auth(
                "Invalid token type - expected access token".to_string(),
            ));
        }

        Ok(decoded.claims)
    }

    /// Verify and decode a JWT refresh token
    pub fn verify_refresh_token(&self, token: &str) -> Result<RefreshClaims, AppError> {
        let decoding_key = DecodingKey::from_secret(self.config.jwt_secret.as_ref());
        let validation = Validation::default();

        let decoded = decode::<RefreshClaims>(token, &decoding_key, &validation)
            .map_err(|e| AppError::auth(format!("Refresh token verification failed: {}", e)))?;

        // Verify this is a refresh token
        if !decoded.claims.is_refresh_token() {
            return Err(AppError::auth(
                "Invalid token type - expected refresh token".to_string(),
            ));
        }

        Ok(decoded.claims)
    }

    /// Generate a new access token from a refresh token
    pub fn refresh_access_token(&self, refresh_token: &str) -> Result<String, AppError> {
        let refresh_claims = self.verify_refresh_token(refresh_token)?;

        // Generate new access token with same user information
        let role = refresh_claims
            .user_role()
            .map_err(|e| AppError::auth(format!("Invalid role in refresh token: {}", e)))?;

        self.generate_token(
            refresh_claims.sub,
            refresh_claims.email,
            role,
            refresh_claims.auth_collection,
        )
    }

    /// Generate a new token pair from a refresh token
    pub fn refresh_token_pair(&self, refresh_token: &str) -> Result<TokenPair, AppError> {
        let refresh_claims = self.verify_refresh_token(refresh_token)?;

        // Generate new token pair with same user information
        let role = refresh_claims
            .user_role()
            .map_err(|e| AppError::auth(format!("Invalid role in refresh token: {}", e)))?;

        self.generate_token_pair(
            refresh_claims.sub,
            refresh_claims.email,
            role,
            refresh_claims.auth_collection,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> AuthServiceConfig {
        AuthServiceConfig::new("test_secret_key_for_jwt_testing".to_string())
    }

    #[test]
    fn test_access_token_generation_and_verification() {
        let config = create_test_config();
        let jwt_service = JwtService::new(&config);

        let token = jwt_service
            .generate_token(
                "user123".to_string(),
                "test@example.com".to_string(),
                UserRole::User,
                "users".to_string(),
            )
            .unwrap();

        let claims = jwt_service.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "user");
        assert_eq!(claims.auth_collection, "users");
        assert!(claims.is_access_token());
    }

    #[test]
    fn test_refresh_token_generation_and_verification() {
        let config = create_test_config();
        let jwt_service = JwtService::new(&config);

        let token = jwt_service
            .generate_refresh_token(
                "user123".to_string(),
                "test@example.com".to_string(),
                UserRole::User,
                "users".to_string(),
            )
            .unwrap();

        let claims = jwt_service.verify_refresh_token(&token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "user");
        assert_eq!(claims.auth_collection, "users");
        assert!(claims.is_refresh_token());
        assert!(!claims.jti.is_empty());
    }

    #[test]
    fn test_token_pair_generation() {
        let config = create_test_config();
        let jwt_service = JwtService::new(&config);

        let token_pair = jwt_service
            .generate_token_pair(
                "user123".to_string(),
                "test@example.com".to_string(),
                UserRole::User,
                "users".to_string(),
            )
            .unwrap();

        // Verify access token
        let access_claims = jwt_service.verify_token(&token_pair.access_token).unwrap();
        assert!(access_claims.is_access_token());

        // Verify refresh token
        let refresh_claims = jwt_service
            .verify_refresh_token(&token_pair.refresh_token)
            .unwrap();
        assert!(refresh_claims.is_refresh_token());

        // Both should have same user info
        assert_eq!(access_claims.sub, refresh_claims.sub);
        assert_eq!(access_claims.email, refresh_claims.email);
        assert_eq!(access_claims.role, refresh_claims.role);
    }

    #[test]
    fn test_token_refresh() {
        let config = create_test_config();
        let jwt_service = JwtService::new(&config);

        let original_pair = jwt_service
            .generate_token_pair(
                "user123".to_string(),
                "test@example.com".to_string(),
                UserRole::User,
                "users".to_string(),
            )
            .unwrap();

        // Use refresh token to get new access token
        let new_access_token = jwt_service
            .refresh_access_token(&original_pair.refresh_token)
            .unwrap();
        let new_claims = jwt_service.verify_token(&new_access_token).unwrap();

        assert_eq!(new_claims.sub, "user123");
        assert_eq!(new_claims.email, "test@example.com");
        assert!(new_claims.is_access_token());
    }

    #[test]
    fn test_token_type_validation() {
        let config = create_test_config();
        let jwt_service = JwtService::new(&config);

        let access_token = jwt_service
            .generate_token(
                "user123".to_string(),
                "test@example.com".to_string(),
                UserRole::User,
                "users".to_string(),
            )
            .unwrap();

        let refresh_token = jwt_service
            .generate_refresh_token(
                "user123".to_string(),
                "test@example.com".to_string(),
                UserRole::User,
                "users".to_string(),
            )
            .unwrap();

        // Access token should not verify as refresh token
        assert!(jwt_service.verify_refresh_token(&access_token).is_err());

        // Refresh token should not verify as access token
        assert!(jwt_service.verify_token(&refresh_token).is_err());
    }
}
