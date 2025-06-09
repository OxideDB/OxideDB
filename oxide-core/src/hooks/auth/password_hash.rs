//! Password Hashing Hook
//!
//! This hook automatically hashes plain text passwords in authentication
//! collections before they are stored in the database. It follows the
//! hook-first architecture by modifying data in BeforeRecordCreate and
//! BeforeRecordUpdate events.

use crate::{BeforeEventContext, AppError, AuthService};
use std::sync::Arc;
use tracing::{info, warn, debug};

/// Configuration for password hashing behavior
#[derive(Debug, Clone)]
pub struct PasswordHashConfig {
    /// Collections that should have passwords hashed
    pub auth_collections: Vec<String>,
    /// Field name containing the plain text password
    pub password_field: String,
    /// Field name for the hashed password output
    pub password_hash_field: String,
    /// Whether to remove the plain text password after hashing
    pub remove_plain_password: bool,
    /// Whether to add metadata about the hashing operation
    pub add_metadata: bool,
}

impl Default for PasswordHashConfig {
    fn default() -> Self {
        Self {
            auth_collections: vec!["users".to_string(), "superusers".to_string()],
            password_field: "password".to_string(),
            password_hash_field: "passwordHash".to_string(),
            remove_plain_password: true,
            add_metadata: true,
        }
    }
}

/// Password hashing hook that automatically hashes passwords in auth collections
pub struct PasswordHashingHook {
    auth_service: Arc<AuthService>,
    config: PasswordHashConfig,
}

impl PasswordHashingHook {
    /// Create a new password hashing hook with default configuration
    pub fn new(auth_service: Arc<AuthService>) -> Self {
        Self {
            auth_service,
            config: PasswordHashConfig::default(),
        }
    }

    /// Create a new password hashing hook with custom configuration
    pub fn with_config(auth_service: Arc<AuthService>, config: PasswordHashConfig) -> Self {
        Self {
            auth_service,
            config,
        }
    }

    /// Hook that processes BeforeRecordCreate events to hash passwords
    pub fn handle_before_record_create(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.should_process_collection(&context.collection) {
            debug!("Processing password hashing for create in collection: {}", context.collection);
            self.hash_password_if_present(context)?;
        }
        Ok(())
    }

    /// Hook that processes BeforeRecordUpdate events to hash passwords
    pub fn handle_before_record_update(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.should_process_collection(&context.collection) {
            debug!("Processing password hashing for update in collection: {}", context.collection);
            self.hash_password_if_present(context)?;
        }
        Ok(())
    }

    /// Check if this collection should have passwords hashed
    fn should_process_collection(&self, collection: &str) -> bool {
        self.config.auth_collections.contains(&collection.to_string())
    }

    /// Hash password if present in the data
    fn hash_password_if_present(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        // Check if the data contains a plain text password
        if let Some(password_value) = context.data.get(&self.config.password_field) {
            if let Some(password_str) = password_value.as_str() {
                if password_str.is_empty() {
                    warn!("Empty password provided for {} collection", context.collection);
                    return Err(AppError::validation("password", "Password cannot be empty"));
                }

                info!("🔒 Hashing password for {} collection", context.collection);
                
                // Hash the password using the auth service
                let password_hash = self.auth_service.hash_password(password_str)?;
                
                // Modify the data directly in the context
                if let Some(data_obj) = context.data.as_object_mut() {
                    // Remove the plain text password if configured to do so
                    if self.config.remove_plain_password {
                        data_obj.remove(&self.config.password_field);
                    }
                    
                    // Add the hashed password
                    data_obj.insert(
                        self.config.password_hash_field.clone(), 
                        serde_json::Value::String(password_hash)
                    );
                    
                    // Add metadata about the transformation if configured
                    if self.config.add_metadata {
                        if let Some(metadata_obj) = context.metadata.as_object_mut() {
                            metadata_obj.insert("password_hashed".to_string(), serde_json::Value::Bool(true));
                            metadata_obj.insert("hash_algorithm".to_string(), serde_json::Value::String("argon2".to_string()));
                            metadata_obj.insert("hashed_at".to_string(), serde_json::Value::String(
                                chrono::Utc::now().to_rfc3339()
                            ));
                        }
                    }
                    
                    info!("✅ Password hashed and replaced in {} collection", context.collection);
                } else {
                    warn!("⚠️ Data is not an object, cannot hash password");
                    return Err(AppError::validation("data", "Expected object with password field"));
                }
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
        let auth_service = Arc::new(AuthService::new("test_secret".to_string()));
        let hook = PasswordHashingHook::new(auth_service);

        let mut context = BeforeEventContext {
            collection: "users".to_string(),
            data: json!({
                "email": "test@example.com",
                "password": "plain_password_123"
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        // Test password hashing
        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Verify password was hashed and original removed
        assert!(context.data.get("password").is_none());
        assert!(context.data.get("passwordHash").is_some());
        
        // Verify metadata was added
        assert_eq!(context.metadata.get("password_hashed"), Some(&json!(true)));
        assert_eq!(context.metadata.get("hash_algorithm"), Some(&json!("argon2")));
    }

    #[test]
    fn test_custom_config() {
        let auth_service = Arc::new(AuthService::new("test_secret".to_string()));
        let config = PasswordHashConfig {
            auth_collections: vec!["custom_users".to_string()],
            password_field: "pwd".to_string(),
            password_hash_field: "pwd_hash".to_string(),
            remove_plain_password: false,
            add_metadata: false,
        };
        let hook = PasswordHashingHook::with_config(auth_service, config);

        let mut context = BeforeEventContext {
            collection: "custom_users".to_string(),
            data: json!({
                "email": "test@example.com",
                "pwd": "plain_password_123"
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Verify custom field names and that original password is kept
        assert!(context.data.get("pwd").is_some());
        assert!(context.data.get("pwd_hash").is_some());
        assert!(context.metadata.get("password_hashed").is_none());
    }
} 