//! Password Hashing Hook
//!
//! This hook automatically hashes plain text passwords in authentication
//! collections before they are stored in the database. It follows the
//! hook-first architecture by modifying data in BeforeRecordCreate and
//! BeforeRecordUpdate events.
//!
//! The hook can operate in two modes:
//! 1. Schema-aware mode: Automatically detects password fields from collection schemas
//! 2. Configuration mode: Uses explicit field names from configuration

use crate::{BeforeEventContext, AppError, AuthService, CollectionSchema};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{info, warn, debug};

/// Configuration for password hashing behavior
#[derive(Debug, Clone)]
pub struct PasswordHashConfig {
    /// Whether to use schema-aware mode (auto-detect password fields from schema)
    pub schema_aware: bool,
    /// Collections that should have passwords hashed (only used if schema_aware is false)
    pub auth_collections: Vec<String>,
    /// Field name containing the plain text password (only used if schema_aware is false)
    pub password_field: String,
    /// Whether to add metadata about the hashing operation
    pub add_metadata: bool,
    /// Optional pepper for password hashing
    pub pepper: Option<String>,
}

impl Default for PasswordHashConfig {
    fn default() -> Self {
        Self {
            schema_aware: true,
            auth_collections: vec!["_users".to_string(), "_superusers".to_string()],
            password_field: "password".to_string(),
            add_metadata: true,
            pepper: None,
        }
    }
}

/// Password hashing hook that automatically hashes passwords in auth collections
pub struct PasswordHashingHook {
    auth_service: Arc<AuthService>,
    config: PasswordHashConfig,
    schemas: Arc<RwLock<HashMap<String, CollectionSchema>>>,
}

impl PasswordHashingHook {
    /// Create a new password hashing hook with default configuration
    pub fn new(auth_service: Arc<AuthService>) -> Self {
        Self {
            auth_service,
            config: PasswordHashConfig::default(),
            schemas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new password hashing hook with custom configuration
    pub fn with_config(auth_service: Arc<AuthService>, config: PasswordHashConfig) -> Self {
        Self {
            auth_service,
            config,
            schemas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a schema for a collection
    pub fn register_schema(&self, collection: String, schema: CollectionSchema) -> Result<(), AppError> {
        let mut schemas = self.schemas.write().map_err(|_| {
            AppError::internal("Failed to acquire write lock for schemas")
        })?;
        
        schemas.insert(collection.clone(), schema);
        debug!("Registered schema for collection: {}", collection);
        Ok(())
    }

    /// Hook that processes BeforeRecordCreate events to hash passwords
    pub fn handle_before_record_create(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        debug!("Processing password hashing for create in collection: {}", context.collection);
        self.hash_passwords_in_data(context)?;
        Ok(())
    }

    /// Hook that processes BeforeRecordUpdate events to hash passwords
    pub fn handle_before_record_update(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        debug!("Processing password hashing for update in collection: {}", context.collection);
        self.hash_passwords_in_data(context)?;
        Ok(())
    }

    /// Main password hashing logic that works in both schema-aware and config modes
    fn hash_passwords_in_data(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.config.schema_aware {
            self.hash_passwords_schema_aware(context)
        } else {
            self.hash_passwords_config_mode(context)
        }
    }

    /// Hash passwords using schema information (automatically detect password fields)
    fn hash_passwords_schema_aware(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        let schemas = self.schemas.read().map_err(|_| {
            AppError::internal("Failed to acquire read lock for schemas")
        })?;

        if let Some(schema) = schemas.get(&context.collection) {
            // Find all password fields in the schema
            let password_fields: Vec<_> = schema.fields
                .iter()
                .filter(|(_, field_def)| field_def.field_type.requires_hashing())
                .collect();

            if password_fields.is_empty() {
                debug!("No password fields found in schema for collection: {}", context.collection);
                return Ok(());
            }

            // Hash each password field found
            for (field_name, field_def) in password_fields {
                self.hash_single_password_field(context, field_name, field_def)?;
            }
        } else {
            debug!("No schema found for collection '{}', falling back to config mode for password hashing", context.collection);
            // Fall back to config mode when no schema is available
            return self.hash_passwords_config_mode(context);
        }

        Ok(())
    }

    /// Hash passwords using configuration mode (legacy approach)
    fn hash_passwords_config_mode(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if !self.config.auth_collections.contains(&context.collection) {
            return Ok(());
        }

        // Check if the data contains a plain text password
        if let Some(password_value) = context.data.get(&self.config.password_field) {
            if let Some(password_str) = password_value.as_str() {
                if password_str.is_empty() {
                    warn!("Empty password provided for {} collection", context.collection);
                    return Err(AppError::validation("password", "Password cannot be empty"));
                }

                info!("🔒 Hashing password field '{}' for {} collection", self.config.password_field, context.collection);
                
                // Hash the password using the auth service
                let password_hash = self.auth_service.hash_password(password_str)?;
                
                // Modify the data directly in the context
                if let Some(data_obj) = context.data.as_object_mut() {
                    // Add the hashed password directly to the original field
                    data_obj.insert(self.config.password_field.to_string(), serde_json::Value::String(password_hash));
                    
                    self.add_hashing_metadata(context, &self.config.password_field)?;
                    
                    info!("✅ Password hashed and replaced in {} collection", context.collection);
                } else {
                    warn!("⚠️ Data is not an object, cannot hash password");
                    return Err(AppError::validation("data", "Expected object with password field"));
                }
            }
        }

        Ok(())
    }

    /// Hash a single password field from schema
    fn hash_single_password_field(
        &self, 
        context: &mut BeforeEventContext, 
        field_name: &str, 
        _field_def: &crate::FieldDefinition
    ) -> Result<(), AppError> {
        if let Some(password_value) = context.data.get(field_name) {
            if let Some(password_str) = password_value.as_str() {
                if password_str.is_empty() {
                    warn!("Empty password provided for field '{}' in {} collection", field_name, context.collection);
                    return Err(AppError::validation(field_name, "Password cannot be empty"));
                }

                info!("🔒 Hashing password field '{}' for {} collection", field_name, context.collection);
                
                // Hash the password using the auth service
                let password_hash = self.auth_service.hash_password(password_str)?;
                
                // Modify the data directly in the context
                if let Some(data_obj) = context.data.as_object_mut() {
                    // Add the hashed password directly to the original field
                    data_obj.insert(field_name.to_string(), serde_json::Value::String(password_hash));
                    
                    self.add_hashing_metadata(context, field_name)?;
                    
                    info!("✅ Password field '{}' hashed and replaced in {} collection", field_name, context.collection);
                } else {
                    warn!("⚠️ Data is not an object, cannot hash password");
                    return Err(AppError::validation("data", "Expected object with password field"));
                }
            }
        }

        Ok(())
    }

    /// Add metadata about the hashing operation
    fn add_hashing_metadata(&self, context: &mut BeforeEventContext, field_name: &str) -> Result<(), AppError> {
        if self.config.add_metadata {
            if let Some(metadata_obj) = context.metadata.as_object_mut() {
                metadata_obj.insert("password_hashed".to_string(), serde_json::Value::Bool(true));
                metadata_obj.insert("hash_algorithm".to_string(), serde_json::Value::String("argon2".to_string()));
                metadata_obj.insert("hashed_at".to_string(), serde_json::Value::String(
                    chrono::Utc::now().to_rfc3339()
                ));
                metadata_obj.insert("hashed_field".to_string(), serde_json::Value::String(field_name.to_string()));
            }
        }
        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &PasswordHashConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AuthService;
    use serde_json::json;

    #[test]
    fn test_password_hashing_hook() {
        let auth_config = crate::auth::AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let mut config = PasswordHashConfig::default();
        config.schema_aware = false; // Use config mode for this test
        let hook = PasswordHashingHook::with_config(auth_service, config);

        let mut context = BeforeEventContext::new_create(
            "users".to_string(),
            json!({
                "email": "test@example.com",
                "password": "plain_password_123"
            }),
        );

        // Test password hashing
        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Verify password was hashed and stored in the same field
        assert!(context.data.get("password").is_some());
        let password_value = context.data.get("password").unwrap().as_str().unwrap();
        assert_ne!(password_value, "plain_password_123"); // Should be hashed
        assert!(password_value.starts_with("$argon2")); // Should be argon2 hash
        
        // Verify metadata was added
        assert_eq!(context.metadata.get("password_hashed"), Some(&json!(true)));
        assert_eq!(context.metadata.get("hash_algorithm"), Some(&json!("argon2")));
    }

    #[test]
    fn test_custom_config() {
        let auth_config = crate::auth::AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let config = PasswordHashConfig {
            schema_aware: false,
            auth_collections: vec!["custom_users".to_string()],
            password_field: "pwd".to_string(),
            add_metadata: false,
            pepper: None,
        };
        let hook = PasswordHashingHook::with_config(auth_service, config);

        let mut context = BeforeEventContext::new_create(
            "custom_users".to_string(),
            json!({
                "email": "test@example.com",
                "pwd": "plain_password_123"
            }),
        );

        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Verify password was hashed and stored in the same field
        assert!(context.data.get("pwd").is_some());
        let password_value = context.data.get("pwd").unwrap().as_str().unwrap();
        assert_ne!(password_value, "plain_password_123"); // Should be hashed
        assert!(password_value.starts_with("$argon2")); // Should be argon2 hash
        assert!(context.metadata.get("password_hashed").is_none()); // No metadata
    }

    #[test]
    fn test_schema_aware_password_hashing() {
        let auth_config = crate::auth::AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let hook = PasswordHashingHook::new(auth_service); // Uses schema_aware: true by default

        // Create a schema with password fields
        let mut schema = crate::CollectionSchema::new("users".to_string(), crate::CollectionType::Base);
        schema.add_field("email".to_string(), crate::FieldDefinition {
            field_type: crate::FieldType::Email,
            required: true,
            unique: true,
            default: None,
            validation: None,
        });
        schema.add_field("password".to_string(), crate::FieldDefinition {
            field_type: crate::FieldType::Password,
            required: true,
            unique: false,
            default: None,
            validation: None,
        });
        schema.add_field("backup_password".to_string(), crate::FieldDefinition {
            field_type: crate::FieldType::Password,
            required: false,
            unique: false,
            default: None,
            validation: None,
        });

        // Register the schema
        assert!(hook.register_schema("users".to_string(), schema).is_ok());

        let mut context = BeforeEventContext::new_create(
            "users".to_string(),
            json!({
                "email": "test@example.com",
                "password": "main_password_123",
                "backup_password": "backup_password_456"
            }),
        );

        // Test schema-aware password hashing
        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Verify both password fields were hashed and stored in the same fields
        assert!(context.data.get("password").is_some());
        assert!(context.data.get("backup_password").is_some());
        
        let main_password = context.data.get("password").unwrap().as_str().unwrap();
        let backup_password = context.data.get("backup_password").unwrap().as_str().unwrap();
        
        assert_ne!(main_password, "main_password_123"); // Should be hashed
        assert_ne!(backup_password, "backup_password_456"); // Should be hashed
        assert!(main_password.starts_with("$argon2")); // Should be argon2 hash
        assert!(backup_password.starts_with("$argon2")); // Should be argon2 hash
        
        // Verify email field was not affected
        assert_eq!(context.data.get("email").unwrap(), "test@example.com");
        
        // Verify metadata was added
        assert_eq!(context.metadata.get("password_hashed"), Some(&json!(true)));
        assert_eq!(context.metadata.get("hash_algorithm"), Some(&json!("argon2")));
    }
} 