//! Authentication-related operations for SQLite database

use super::connection::SqliteDb;
use crate::db::{AuthRequest, AuthResponse, Db, ListParams, RefreshTokenRecord, RegisterRequest};
use oxide_core::{
    auth::{AuthCollectionConfig, AuthTokens},
    AfterEventContext, AfterEventType, AppError, BeforeEventContext, BeforeEventType,
    CollectionType,
};
use tokio::task::spawn_blocking;
use tracing::{info, warn};
use uuid::Uuid;

impl SqliteDb {
    /// Authenticate a user against a specific auth collection
    pub async fn authenticate_user(
        &self,
        auth_request: AuthRequest,
        auth_config: &AuthCollectionConfig,
    ) -> Result<AuthResponse, AppError> {
        info!(
            "🔑 Authenticating user in collection '{}': {}",
            auth_request.collection, auth_request.identifier
        );

        // Find user by identifier in the specified collection
        let user_record = self
            .find_user_by_identifier(
                &auth_request.collection,
                &auth_config.identifier_field,
                &auth_request.identifier,
            )
            .await?;

        // Create context for BeforeUserAuth event
        let mut auth_context = BeforeEventContext::new_read(
            auth_request.collection.clone(),
            user_record.id.clone(),
            serde_json::json!({
                auth_config.identifier_field.clone(): auth_request.identifier.clone()
            }),
        );

        // Dispatch BeforeUserAuth event
        self.event_bus
            .dispatch_before(BeforeEventType::UserAuth, &mut auth_context)
            .await?;

        // Verify credential - hash is stored in the credential field
        let credential_hash = user_record
            .data
            .get(&auth_config.credential_field)
            .and_then(|h| h.as_str())
            .ok_or_else(|| AppError::auth("Invalid user data: missing credential"))?;

        if !self
            .auth_service
            .verify_password(&auth_request.credential, credential_hash)?
        {
            warn!(
                "Authentication failed for user: {} - invalid credential",
                auth_request.identifier
            );
            return Err(AppError::auth("Invalid credentials"));
        }

        // Generate authentication tokens based on collection configuration
        let role = auth_config.default_role.clone();
        let auth_tokens = self.auth_service.generate_auth_tokens(
            user_record.id.clone(),
            auth_request.identifier.clone(),
            role.clone(),
            auth_request.collection.clone(),
        )?;

        // Extract tokens from the response
        let (access_token, refresh_token) = match auth_tokens {
            AuthTokens::AccessOnly(token) => (token, None),
            AuthTokens::Pair(pair) => (pair.access_token, Some(pair.refresh_token)),
        };

        // Dispatch AfterUserAuth event
        let request_context =
            oxide_core::event::context::RequestContext::authenticated(user_record.id.clone());
        self.event_bus
            .dispatch_after(
                AfterEventType::UserAuthenticated,
                &AfterEventContext::UserAuthenticated {
                    event_id: Uuid::new_v4().to_string(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    user_id: user_record.id.clone(),
                    email: auth_request.identifier.clone(),
                    request_context,
                },
            )
            .await?;

        let collection_name = auth_request.collection.clone();
        let identifier = auth_request.identifier.clone();

        let response = AuthResponse {
            user_id: user_record.id,
            token: access_token,
            refresh_token,
            auth_collection: auth_request.collection,
            role: role.to_string(),
            user_data: user_record.data,
        };

        info!(
            "✅ User authenticated successfully: {} from collection '{}' (refresh_token: {})",
            identifier,
            collection_name,
            response.refresh_token.is_some()
        );
        Ok(response)
    }

    /// Register a new user in a specific auth collection
    pub async fn register_user(
        &self,
        register_request: RegisterRequest,
        auth_config: &AuthCollectionConfig,
    ) -> Result<String, AppError> {
        info!(
            "📝 Registering new user in collection '{}': {}",
            register_request.collection, register_request.identifier
        );

        // Check if registration is enabled for this collection
        if !auth_config.registration_enabled {
            return Err(AppError::auth(
                "Registration is not enabled for this collection",
            ));
        }

        // Check if user already exists in any auth collection to prevent duplicates
        let auth_collections = self.list_auth_collections().await?;
        for schema in &auth_collections {
            if self
                .find_user_by_identifier(
                    &schema.name,
                    &auth_config.identifier_field,
                    &register_request.identifier,
                )
                .await
                .is_ok()
            {
                return Err(AppError::conflict(
                    "User with this identifier already exists",
                ));
            }
        }

        // Create user data - hooks will handle validation, hashing, and field additions
        let mut user_data = serde_json::json!({
            auth_config.identifier_field.clone(): register_request.identifier.clone(),
            auth_config.credential_field.clone(): register_request.credential.clone(),
        });

        // Add additional data if provided
        if let Some(serde_json::Value::Object(additional_map)) = register_request.additional_data {
            if let serde_json::Value::Object(ref mut user_map) = user_data {
                for (key, value) in additional_map {
                    user_map.insert(key, value);
                }
            }
        }

        // Set default verification status based on auth config
        if !user_data.as_object().unwrap().contains_key("verified") {
            user_data.as_object_mut().unwrap().insert(
                "verified".to_string(),
                serde_json::Value::Bool(!auth_config.email_verification_required),
            );
        }

        // Use the standard record creation flow - hooks will handle all transformations
        let record =
            <Self as Db>::create_record(self, &register_request.collection, user_data).await?;

        // Dispatch user registration event
        let request_context = oxide_core::event::context::RequestContext::anonymous();
        self.event_bus
            .dispatch_after(
                AfterEventType::UserRegistered,
                &AfterEventContext::UserRegistered {
                    event_id: Uuid::new_v4().to_string(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    user_id: record.id.clone(),
                    email: register_request.identifier.clone(),
                    metadata: record.data.clone(),
                    request_context,
                },
            )
            .await?;

        info!(
            "✅ User registered successfully: {} in collection '{}'",
            register_request.identifier, register_request.collection
        );
        Ok(record.id)
    }

    /// Find a user by identifier in a specific auth collection
    pub async fn find_user_by_identifier(
        &self,
        collection: &str,
        identifier_field: &str,
        identifier_value: &str,
    ) -> Result<crate::Record, AppError> {
        let records = <Self as Db>::list_records(self, collection, ListParams::default()).await?;

        for record in records {
            if let Some(user_identifier) =
                record.data.get(identifier_field).and_then(|e| e.as_str())
            {
                if user_identifier == identifier_value {
                    return Ok(record);
                }
            }
        }

        Err(AppError::not_found("user", identifier_value))
    }

    /// List all auth collections
    pub async fn list_auth_collections(
        &self,
    ) -> Result<Vec<oxide_core::CollectionSchema>, AppError> {
        let all_collections = <Self as Db>::list_collections(self).await?;
        Ok(all_collections
            .into_iter()
            .filter(|schema| schema.collection_type == CollectionType::Auth)
            .collect())
    }

    /// Store metadata for a newly issued refresh token.
    pub async fn store_refresh_token(&self, token: RefreshTokenRecord) -> Result<(), AppError> {
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;
            let now = current_timestamp();

            conn.execute(
                r#"
                INSERT INTO auth_refresh_tokens (
                    token_hash,
                    user_id,
                    auth_collection,
                    jti,
                    expires_at,
                    revoked_at,
                    replaced_by_hash,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?6)
                "#,
                (
                    &token.token_hash,
                    &token.user_id,
                    &token.auth_collection,
                    &token.jti,
                    token.expires_at,
                    now,
                ),
            )
            .map_err(|e| AppError::database(format!("Failed to store refresh token: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Atomically revoke an existing active refresh token and persist its replacement.
    pub async fn rotate_refresh_token(
        &self,
        old_token_hash: &str,
        new_token: RefreshTokenRecord,
    ) -> Result<(), AppError> {
        let old_token_hash = old_token_hash.to_string();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let mut conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;
            let now = current_timestamp();

            let tx = conn
                .transaction()
                .map_err(|e| AppError::database(format!("Failed to start transaction: {}", e)))?;

            let rows_affected = tx
                .execute(
                    r#"
                    UPDATE auth_refresh_tokens
                    SET revoked_at = ?1,
                        replaced_by_hash = ?2,
                        updated_at = ?1
                    WHERE token_hash = ?3
                      AND revoked_at IS NULL
                      AND expires_at > ?1
                    "#,
                    (now, &new_token.token_hash, &old_token_hash),
                )
                .map_err(|e| {
                    AppError::database(format!("Failed to revoke refresh token: {}", e))
                })?;

            if rows_affected == 0 {
                return Err(AppError::auth("Invalid or expired refresh token"));
            }

            tx.execute(
                r#"
                INSERT INTO auth_refresh_tokens (
                    token_hash,
                    user_id,
                    auth_collection,
                    jti,
                    expires_at,
                    revoked_at,
                    replaced_by_hash,
                    created_at,
                    updated_at
                )
                VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?6)
                "#,
                (
                    &new_token.token_hash,
                    &new_token.user_id,
                    &new_token.auth_collection,
                    &new_token.jti,
                    new_token.expires_at,
                    now,
                ),
            )
            .map_err(|e| AppError::database(format!("Failed to store refresh token: {}", e)))?;

            tx.commit()
                .map_err(|e| AppError::database(format!("Failed to commit transaction: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Revoke a refresh token if it is present.
    pub async fn revoke_refresh_token(&self, token_hash: &str) -> Result<(), AppError> {
        let token_hash = token_hash.to_string();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;
            let now = current_timestamp();

            conn.execute(
                r#"
                UPDATE auth_refresh_tokens
                SET revoked_at = COALESCE(revoked_at, ?1),
                    updated_at = ?1
                WHERE token_hash = ?2
                "#,
                (now, &token_hash),
            )
            .map_err(|e| AppError::database(format!("Failed to revoke refresh token: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }
}

fn current_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::{
        auth::{AuthService, AuthServiceConfig},
        InMemoryEventBus,
    };
    use std::sync::Arc;

    fn test_db() -> SqliteDb {
        let event_bus = Arc::new(InMemoryEventBus::new());
        let auth_service = Arc::new(AuthService::new(AuthServiceConfig::new(
            "refresh-token-test-secret".to_string(),
        )));

        SqliteDb::new(":memory:", event_bus, auth_service).unwrap()
    }

    fn refresh_token_record(hash: &str, jti: &str) -> RefreshTokenRecord {
        RefreshTokenRecord {
            token_hash: hash.to_string(),
            user_id: "user-1".to_string(),
            auth_collection: "_users".to_string(),
            jti: jti.to_string(),
            expires_at: current_timestamp() + 3600,
        }
    }

    #[tokio::test]
    async fn refresh_token_rotation_is_single_use() {
        let db = test_db();
        db.initialize().await.unwrap();

        db.store_refresh_token(refresh_token_record("old-hash", "old-jti"))
            .await
            .unwrap();
        db.rotate_refresh_token("old-hash", refresh_token_record("new-hash", "new-jti"))
            .await
            .unwrap();

        let replay_result = db
            .rotate_refresh_token(
                "old-hash",
                refresh_token_record("replay-hash", "replay-jti"),
            )
            .await;
        assert!(matches!(replay_result, Err(AppError::Auth { .. })));

        db.revoke_refresh_token("new-hash").await.unwrap();
        let revoked_result = db
            .rotate_refresh_token("new-hash", refresh_token_record("next-hash", "next-jti"))
            .await;
        assert!(matches!(revoked_result, Err(AppError::Auth { .. })));
    }
}
