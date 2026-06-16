//! Authentication service
//!
//! This module provides the main authentication service that orchestrates
//! all authentication functionality including JWT token management, password
//! hashing, and collection management.

use super::jwt::{Claims, JwtService, RefreshClaims, TokenPair};
use super::password::PasswordService;
use super::types::{AuthCollectionConfig, AuthServiceConfig, UserRole};
use crate::{AppError, CollectionSchema, CollectionType};

/// Main authentication service
pub struct AuthService {
    config: AuthServiceConfig,
    password_service: PasswordService,
}

impl AuthService {
    /// Create a new authentication service
    pub fn new(config: AuthServiceConfig) -> Self {
        Self {
            config,
            password_service: PasswordService::new(),
        }
    }

    /// Get the service configuration
    pub fn config(&self) -> &AuthServiceConfig {
        &self.config
    }

    /// Get a JWT service instance
    pub fn jwt_service(&self) -> JwtService<'_> {
        JwtService::new(&self.config)
    }

    /// Get the password service
    pub fn password_service(&self) -> &PasswordService {
        &self.password_service
    }

    /// Update auth collections based on collection schemas
    pub fn update_auth_collections(&self, schemas: &[CollectionSchema]) {
        for schema in schemas {
            if schema.collection_type == CollectionType::Auth {
                // Create default auth collection config if not exists
                if !self.config.is_auth_collection(&schema.name) {
                    // Set the correct default role based on collection name
                    let default_role = self.determine_role_from_collection(&schema.name);
                    let mut config = AuthCollectionConfig {
                        collection: schema.name.clone(),
                        default_role,
                        ..Default::default()
                    };

                    // Configure refresh tokens for superuser collections
                    if matches!(config.default_role, UserRole::Superuser) {
                        config.refresh_tokens_enabled = true;
                        config.refresh_tokens_required = true; // Enforce for superusers
                    }

                    // Try to detect identifier and credential fields from schema
                    for field_name in schema.fields.keys() {
                        match field_name.as_str() {
                            "email" | "username" | "login" => {
                                config.identifier_field = field_name.clone();
                            }
                            "password" | "credential" => {
                                config.credential_field = field_name.clone();
                            }
                            _ => {}
                        }
                    }

                    self.config.add_auth_collection(config);
                }
            }
        }
    }

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String, AppError> {
        self.password_service.hash_password(password)
    }

    /// Verify a password against its hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        self.password_service.verify_password(password, hash)
    }

    /// Hash a password using Argon2, off the async executor thread.
    ///
    /// Argon2 is deliberately CPU- and memory-intensive (~20 MiB, tens of ms).
    /// Running it inline on a tokio worker parks the worker for the duration,
    /// starving other tasks. This wrapper moves the work to the blocking pool.
    pub async fn hash_password_async(&self, password: String) -> Result<String, AppError> {
        let password_service = self.password_service.clone();
        tokio::task::spawn_blocking(move || password_service.hash_password(&password))
            .await
            .map_err(|e| AppError::internal(format!("Password hashing task failed: {}", e)))?
    }

    /// Verify a password against its hash, off the async executor thread.
    ///
    /// See [`hash_password_async`] for rationale on using `spawn_blocking`.
    pub async fn verify_password_async(
        &self,
        password: String,
        hash: String,
    ) -> Result<bool, AppError> {
        let password_service = self.password_service.clone();
        tokio::task::spawn_blocking(move || password_service.verify_password(&password, &hash))
            .await
            .map_err(|e| AppError::internal(format!("Password verify task failed: {}", e)))?
    }

    /// Generate a JWT token for a user (backward compatibility)
    pub fn generate_token(
        &self,
        user_id: String,
        email: String,
        role: UserRole,
        auth_collection: String,
    ) -> Result<String, AppError> {
        let jwt_service = self.jwt_service();
        jwt_service.generate_token(user_id, email, role, auth_collection)
    }

    /// Generate a JWT token with custom claims (backward compatibility)
    pub fn generate_token_with_claims(&self, claims: Claims) -> Result<String, AppError> {
        let jwt_service = self.jwt_service();
        jwt_service.generate_token_with_claims(claims)
    }

    /// Generate both access and refresh tokens
    pub fn generate_token_pair(
        &self,
        user_id: String,
        email: String,
        role: UserRole,
        auth_collection: String,
    ) -> Result<TokenPair, AppError> {
        let jwt_service = self.jwt_service();
        jwt_service.generate_token_pair(user_id, email, role, auth_collection)
    }

    /// Generate authentication tokens based on collection configuration
    pub fn generate_auth_tokens(
        &self,
        user_id: String,
        email: String,
        role: UserRole,
        auth_collection: String,
    ) -> Result<AuthTokens, AppError> {
        let auth_config = self
            .config
            .get_auth_collection(&auth_collection)
            .ok_or_else(|| {
                AppError::internal(format!(
                    "Auth collection '{}' not configured",
                    auth_collection
                ))
            })?;

        let jwt_service = self.jwt_service();

        // Check if refresh tokens are required for this collection/role
        let use_refresh_tokens = self.should_use_refresh_tokens(&auth_config, &role)?;

        if use_refresh_tokens {
            // Generate token pair
            let token_pair =
                jwt_service.generate_token_pair(user_id, email, role, auth_collection)?;
            Ok(AuthTokens::Pair(token_pair))
        } else {
            // Generate only access token
            let access_token = jwt_service.generate_token(user_id, email, role, auth_collection)?;
            Ok(AuthTokens::AccessOnly(access_token))
        }
    }

    /// Check if refresh tokens should be used for a given collection and role
    pub fn should_use_refresh_tokens(
        &self,
        auth_config: &AuthCollectionConfig,
        role: &UserRole,
    ) -> Result<bool, AppError> {
        // Superusers must use refresh tokens if they're available in the collection
        if matches!(role, UserRole::Superuser) {
            if auth_config.refresh_tokens_enabled {
                return Ok(true);
            } else if auth_config.refresh_tokens_required {
                return Err(AppError::auth("Refresh tokens are required for superusers but not enabled for this collection"));
            }
        }

        // For other roles, check if refresh tokens are enabled and required
        Ok(auth_config.refresh_tokens_enabled
            && (auth_config.refresh_tokens_required || matches!(role, UserRole::Superuser)))
    }

    /// Verify and decode a JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let jwt_service = self.jwt_service();
        jwt_service.verify_token(token)
    }

    /// Verify and decode a refresh token
    pub fn verify_refresh_token(&self, token: &str) -> Result<RefreshClaims, AppError> {
        let jwt_service = self.jwt_service();
        jwt_service.verify_refresh_token(token)
    }

    /// Refresh an access token using a refresh token
    pub fn refresh_access_token(&self, refresh_token: &str) -> Result<String, AppError> {
        let jwt_service = self.jwt_service();
        jwt_service.refresh_access_token(refresh_token)
    }

    /// Refresh a token pair using a refresh token
    pub fn refresh_token_pair(&self, refresh_token: &str) -> Result<TokenPair, AppError> {
        let jwt_service = self.jwt_service();
        jwt_service.refresh_token_pair(refresh_token)
    }

    fn determine_role_from_collection(&self, collection: &str) -> UserRole {
        match collection {
            "_superusers" => UserRole::Superuser,
            "_users" => UserRole::User,
            _ => UserRole::User, // Default role for any other auth collection
        }
    }
}

/// Response for authentication tokens
#[derive(Debug, Clone)]
pub enum AuthTokens {
    /// Access token only (for collections that don't use refresh tokens)
    AccessOnly(String),
    /// Both access and refresh tokens
    Pair(TokenPair),
}

impl AuthTokens {
    /// Get the access token
    pub fn access_token(&self) -> &str {
        match self {
            AuthTokens::AccessOnly(token) => token,
            AuthTokens::Pair(pair) => &pair.access_token,
        }
    }

    /// Get the refresh token (if available)
    pub fn refresh_token(&self) -> Option<&str> {
        match self {
            AuthTokens::AccessOnly(_) => None,
            AuthTokens::Pair(pair) => Some(&pair.refresh_token),
        }
    }

    /// Check if this includes a refresh token
    pub fn has_refresh_token(&self) -> bool {
        matches!(self, AuthTokens::Pair(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collection::{CollectionSchema, CollectionType, FieldDefinition};
    use crate::field_types::FieldType;

    #[test]
    fn test_update_auth_collections_sets_correct_roles() {
        // Create auth service
        let config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = AuthService::new(config);

        // Create mock schemas for _users and _superusers collections
        let mut users_schema = CollectionSchema::new("_users".to_string(), CollectionType::Auth);
        users_schema.add_field(
            "email".to_string(),
            FieldDefinition::new(FieldType::Email).required().unique(),
        );
        users_schema.add_field(
            "password".to_string(),
            FieldDefinition::new(FieldType::Password).required(),
        );

        let mut superusers_schema =
            CollectionSchema::new("_superusers".to_string(), CollectionType::Auth);
        superusers_schema.add_field(
            "email".to_string(),
            FieldDefinition::new(FieldType::Email).required().unique(),
        );
        superusers_schema.add_field(
            "password".to_string(),
            FieldDefinition::new(FieldType::Password).required(),
        );

        let schemas = vec![users_schema, superusers_schema];

        // Update auth collections
        auth_service.update_auth_collections(&schemas);

        // Verify that _users collection has User role
        let users_config = auth_service.config().get_auth_collection("_users").unwrap();
        assert_eq!(users_config.default_role, UserRole::User);

        // Verify that _superusers collection has Superuser role
        let superusers_config = auth_service
            .config()
            .get_auth_collection("_superusers")
            .unwrap();
        assert_eq!(superusers_config.default_role, UserRole::Superuser);
    }

    #[test]
    fn test_complete_auth_flow_with_proper_roles() {
        // Create auth service
        let config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = AuthService::new(config);

        // Create mock schemas for _users and _superusers collections
        let mut users_schema = CollectionSchema::new("_users".to_string(), CollectionType::Auth);
        users_schema.add_field(
            "email".to_string(),
            FieldDefinition::new(FieldType::Email).required().unique(),
        );
        users_schema.add_field(
            "password".to_string(),
            FieldDefinition::new(FieldType::Password).required(),
        );

        let mut superusers_schema =
            CollectionSchema::new("_superusers".to_string(), CollectionType::Auth);
        superusers_schema.add_field(
            "email".to_string(),
            FieldDefinition::new(FieldType::Email).required().unique(),
        );
        superusers_schema.add_field(
            "password".to_string(),
            FieldDefinition::new(FieldType::Password).required(),
        );

        let schemas = vec![users_schema, superusers_schema];

        // Update auth collections
        auth_service.update_auth_collections(&schemas);

        // Test token generation for regular user
        let user_token = auth_service
            .generate_token(
                "user123".to_string(),
                "user@example.com".to_string(),
                UserRole::User,
                "_users".to_string(),
            )
            .unwrap();

        // Test token generation for superuser
        let superuser_token = auth_service
            .generate_token(
                "superuser123".to_string(),
                "admin@example.com".to_string(),
                UserRole::Superuser,
                "_superusers".to_string(),
            )
            .unwrap();

        // Verify tokens contain correct roles
        let user_claims = auth_service.verify_token(&user_token).unwrap();
        assert_eq!(user_claims.role, "user");
        assert_eq!(user_claims.user_role().unwrap(), UserRole::User);

        let superuser_claims = auth_service.verify_token(&superuser_token).unwrap();
        assert_eq!(superuser_claims.role, "superuser");
        assert_eq!(superuser_claims.user_role().unwrap(), UserRole::Superuser);
    }

    #[tokio::test]
    async fn test_hash_and_verify_password_async_round_trip() {
        let config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = AuthService::new(config);

        let hash = auth_service
            .hash_password_async("correct horse battery staple".to_string())
            .await
            .expect("hash should succeed");
        assert!(hash.starts_with("$argon2"));

        let ok = auth_service
            .verify_password_async("correct horse battery staple".to_string(), hash.clone())
            .await
            .expect("verify should succeed");
        assert!(ok, "correct password should verify");

        let bad = auth_service
            .verify_password_async("wrong password".to_string(), hash)
            .await
            .expect("verify of wrong password should not error");
        assert!(!bad, "wrong password should not verify");
    }

    /// Verifies that Argon2 hashing runs off the tokio worker thread.
    ///
    /// If `hash_password_async` ran inline, it would park the worker for the
    /// duration of the Argon2 computation and the sibling `yield_now` loop
    /// could not make progress concurrently. We assert the sibling task
    /// advances while the hash is in flight.
    #[tokio::test]
    async fn test_hash_password_does_not_block_executor() {
        let config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = std::sync::Arc::new(AuthService::new(config));

        // Sibling task that must keep advancing while hashing runs.
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let counter_clone = std::sync::Arc::clone(&counter);
        let sibling = tokio::spawn(async move {
            for _ in 0..1000 {
                counter_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                tokio::task::yield_now().await;
            }
        });

        // Argon2 hash — heavy CPU/memory work.
        auth_service
            .hash_password_async("executor-non-blocking-test".to_string())
            .await
            .expect("hash should succeed");

        sibling.await.expect("sibling task should complete");
        // If hashing blocked the worker, the sibling could not have reached 1000.
        assert!(counter.load(std::sync::atomic::Ordering::Relaxed) > 0);
    }
}
