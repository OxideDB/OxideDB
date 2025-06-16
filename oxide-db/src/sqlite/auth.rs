//! Authentication-related operations for SQLite database

use super::connection::SqliteDb;
use crate::db::{Db, ListParams, AuthRequest, AuthResponse, RegisterRequest};
use oxide_core::{
    AppError, CollectionType,
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType,
    auth::{AuthCollectionConfig, Claims},
};
use tracing::{info, debug, warn};

impl SqliteDb {
    /// Initialize authentication collections (users and superusers) - legacy support
    pub(super) async fn initialize_auth_collections(&self) -> Result<(), AppError> {
        use oxide_core::auth::create_auth_collections;
        
        let (users_schema, superusers_schema) = create_auth_collections();
        
        // Create users collection if it doesn't exist
        if !self.collection_exists("users").await? {
            self.create_collection_with_schema(users_schema).await?;
            info!("✅ Created users auth collection");
        } else {
            debug!("Users auth collection already exists");
        }
        
        // Create superusers collection if it doesn't exist
        if !self.collection_exists("superusers").await? {
            self.create_collection_with_schema(superusers_schema).await?;
            info!("✅ Created superusers auth collection");
        } else {
            debug!("Superusers auth collection already exists");
        }
        
        info!("✅ Auth collections initialized");
        Ok(())
    }

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
        let mut auth_context = BeforeEventContext {
            collection: auth_request.collection.clone(),
            data: serde_json::json!({
                auth_config.identifier_field.clone(): auth_request.identifier.clone()
            }),
            metadata: serde_json::json!({}),
            record_id: Some(user_record.id.clone()),
            old_data: Some(user_record.data.clone()),
        };
        
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

        // Generate JWT token with custom claims
        let mut claims = Claims::new(
            user_record.id.clone(),
            auth_request.identifier.clone(),
            auth_config.default_role.to_string(),
            auth_request.collection.clone(),
            self.auth_service.config().token_expiry_hours,
        );

        // Add custom claims from the auth config
        for field_name in &auth_config.custom_claim_fields {
            if let Some(value) = user_record.data.get(field_name) {
                claims.add_custom_claim(field_name.clone(), value.clone());
            }
        }

        let token = self.auth_service.generate_token_with_claims(claims)?;

        // Dispatch AfterUserAuth event
        self.event_bus
            .dispatch_after(AfterEventType::UserAuthenticated, &AfterEventContext::UserAuthenticated {
                user_id: user_record.id.clone(),
                email: auth_request.identifier.clone(),
            })
            .await?;

        let collection_name = auth_request.collection.clone();
        let identifier = auth_request.identifier.clone();
        
        let response = AuthResponse {
            user_id: user_record.id,
            token,
            auth_collection: auth_request.collection,
            role: auth_config.default_role.to_string(),
            user_data: user_record.data,
        };

        info!("✅ User authenticated successfully: {} from collection '{}'", identifier, collection_name);
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
        self.event_bus
            .dispatch_after(AfterEventType::UserRegistered, &AfterEventContext::UserRegistered {
                user_id: record.id.clone(),
                email: register_request.identifier.clone(),
                metadata: record.data.clone(),
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
