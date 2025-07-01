//! Authentication-related operations for SQLite database

use super::connection::SqliteDb;
use crate::db::{Db, ListParams, AuthRequest, AuthResponse, RegisterRequest};
use oxide_core::{
    AppError, CollectionType,
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType,
    auth::{AuthCollectionConfig, AuthTokens},
};
use tracing::{info, debug, warn};
use uuid::Uuid;

impl SqliteDb {
    /// Authenticate a user against a specific auth collection
    pub async fn authenticate_user(&self, auth_request: AuthRequest, auth_config: &AuthCollectionConfig) -> Result<AuthResponse, AppError> {
        info!("🔑 Authenticating user in collection '{}': {}", auth_request.collection, auth_request.identifier);
        
        // Find user by identifier in the specified collection
        let user_record = self.find_user_by_identifier(
            &auth_request.collection,
            &auth_config.identifier_field,
            &auth_request.identifier,
        ).await?;

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
        let credential_hash = user_record.data.get(&auth_config.credential_field)
            .and_then(|h| h.as_str())
            .ok_or_else(|| AppError::auth("Invalid user data: missing credential"))?;

        if !self.auth_service.verify_password(&auth_request.credential, credential_hash)? {
            warn!("Authentication failed for user: {} - invalid credential", auth_request.identifier);
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
        let request_context = oxide_core::event::context::RequestContext::authenticated(user_record.id.clone());
        self.event_bus
            .dispatch_after(AfterEventType::UserAuthenticated, &AfterEventContext::UserAuthenticated {
                event_id: Uuid::new_v4().to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                user_id: user_record.id.clone(),
                email: auth_request.identifier.clone(),
                request_context,
            })
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

        info!("✅ User authenticated successfully: {} from collection '{}' (refresh_token: {})", 
              identifier, collection_name, response.refresh_token.is_some());
        Ok(response)
    }

    /// Register a new user in a specific auth collection
    pub async fn register_user(&self, register_request: RegisterRequest, auth_config: &AuthCollectionConfig) -> Result<String, AppError> {
        info!("📝 Registering new user in collection '{}': {}", register_request.collection, register_request.identifier);

        // Check if registration is enabled for this collection
        if !auth_config.registration_enabled {
            return Err(AppError::auth("Registration is not enabled for this collection"));
        }

        // Check if user already exists in any auth collection to prevent duplicates
        let auth_collections = self.list_auth_collections().await?;
        for schema in &auth_collections {
            if let Ok(_) = self.find_user_by_identifier(
                &schema.name,
                &auth_config.identifier_field,
                &register_request.identifier,
            ).await {
                return Err(AppError::conflict("User with this identifier already exists"));
            }
        }

        // Create user data - hooks will handle validation, hashing, and field additions
        let mut user_data = serde_json::json!({
            auth_config.identifier_field.clone(): register_request.identifier.clone(),
            auth_config.credential_field.clone(): register_request.credential.clone(),
        });

        // Add additional data if provided
        if let Some(additional) = register_request.additional_data {
            if let serde_json::Value::Object(additional_map) = additional {
                if let serde_json::Value::Object(ref mut user_map) = user_data {
                    for (key, value) in additional_map {
                        user_map.insert(key, value);
                    }
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
        let record = <Self as Db>::create_record(self, &register_request.collection, user_data).await?;

        // Dispatch user registration event
        let request_context = oxide_core::event::context::RequestContext::anonymous();
        self.event_bus
            .dispatch_after(AfterEventType::UserRegistered, &AfterEventContext::UserRegistered {
                event_id: Uuid::new_v4().to_string(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                user_id: record.id.clone(),
                email: register_request.identifier.clone(),
                metadata: record.data.clone(),
                request_context,
            })
            .await?;

        info!("✅ User registered successfully: {} in collection '{}'", register_request.identifier, register_request.collection);
        Ok(record.id)
    }

    /// Find a user by identifier in a specific auth collection
    pub async fn find_user_by_identifier(&self, collection: &str, identifier_field: &str, identifier_value: &str) -> Result<crate::Record, AppError> {
        let records = <Self as Db>::list_records(self, collection, ListParams::default()).await?;
        
        for record in records {
            if let Some(user_identifier) = record.data.get(identifier_field).and_then(|e| e.as_str()) {
                if user_identifier == identifier_value {
                    return Ok(record);
                }
            }
        }
        
        Err(AppError::not_found("user", identifier_value))
    }

    /// List all auth collections
    pub async fn list_auth_collections(&self) -> Result<Vec<oxide_core::CollectionSchema>, AppError> {
        let all_collections = <Self as Db>::list_collections(self).await?;
        Ok(all_collections.into_iter()
            .filter(|schema| schema.collection_type == CollectionType::Auth)
            .collect())
    }
}
