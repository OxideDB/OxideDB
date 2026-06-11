//! User Validation Hook
//!
//! This hook validates user data in authentication collections to ensure
//! data integrity and security requirements are met.

use crate::{AppError, BeforeEventContext};
use regex::Regex;
use std::sync::Arc;
use tracing::{debug, warn};

/// Configuration for user validation
#[derive(Debug, Clone)]
pub struct UserValidationConfig {
    /// Collections that should have user validation applied
    pub auth_collections: Vec<String>,
    /// Minimum password length
    pub min_password_length: usize,
    /// Whether email validation is required
    pub validate_email: bool,
    /// Whether to check for duplicate emails (requires database access)
    pub check_duplicate_email: bool,
    /// Custom email regex pattern (None uses default)
    pub email_regex: Option<String>,
}

impl Default for UserValidationConfig {
    fn default() -> Self {
        Self {
            auth_collections: vec!["_users".to_string(), "_superusers".to_string()],
            min_password_length: 8,
            validate_email: true,
            check_duplicate_email: false, // Requires DB access, handled elsewhere
            email_regex: None,
        }
    }
}

/// User validation hook for authentication collections
pub struct UserValidationHook {
    config: UserValidationConfig,
    email_regex: Arc<Regex>,
}

impl UserValidationHook {
    /// Create a new user validation hook with default configuration
    pub fn new() -> Result<Self, AppError> {
        let config = UserValidationConfig::default();
        Self::with_config(config)
    }

    /// Create a new user validation hook with custom configuration
    pub fn with_config(config: UserValidationConfig) -> Result<Self, AppError> {
        // Use custom email regex or default
        let email_pattern = config
            .email_regex
            .as_deref()
            .unwrap_or(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$");

        let email_regex = Arc::new(Regex::new(email_pattern).map_err(|e| {
            AppError::validation("email_regex", &format!("Invalid regex pattern: {}", e))
        })?);

        Ok(Self {
            config,
            email_regex,
        })
    }

    /// Hook that processes BeforeRecordCreate events for user validation
    pub fn handle_before_record_create(
        &self,
        context: &mut BeforeEventContext,
    ) -> Result<(), AppError> {
        if self.should_process_collection(&context.collection) {
            debug!(
                "Validating user data for create in collection: {}",
                context.collection
            );
            self.validate_user_data(context)?;
        }
        Ok(())
    }

    /// Hook that processes BeforeRecordUpdate events for user validation
    pub fn handle_before_record_update(
        &self,
        context: &mut BeforeEventContext,
    ) -> Result<(), AppError> {
        if self.should_process_collection(&context.collection) {
            debug!(
                "Validating user data for update in collection: {}",
                context.collection
            );
            self.validate_user_data(context)?;
        }
        Ok(())
    }

    /// Check if this collection should have user validation applied
    fn should_process_collection(&self, collection: &str) -> bool {
        self.config
            .auth_collections
            .contains(&collection.to_string())
    }

    /// Validate user data according to configuration
    fn validate_user_data(&self, context: &BeforeEventContext) -> Result<(), AppError> {
        // Validate email if present and configured
        if self.config.validate_email {
            if let Some(email_value) = context.data.get("email") {
                if let Some(email_str) = email_value.as_str() {
                    self.validate_email(email_str)?;
                } else {
                    return Err(AppError::validation("email", "Email must be a string"));
                }
            }
        }

        // Validate password if present (only for plain text passwords)
        if let Some(password_value) = context.data.get("password") {
            if let Some(password_str) = password_value.as_str() {
                self.validate_password(password_str)?;
            }
        }

        // Validate required fields are present for auth collections
        self.validate_required_fields(context)?;

        Ok(())
    }

    /// Validate email format
    fn validate_email(&self, email: &str) -> Result<(), AppError> {
        if email.is_empty() {
            return Err(AppError::validation("email", "Email cannot be empty"));
        }

        if email.len() > 254 {
            return Err(AppError::validation(
                "email",
                "Email is too long (max 254 characters)",
            ));
        }

        if !self.email_regex.is_match(email) {
            warn!("Invalid email format attempted: {}", email);
            return Err(AppError::validation("email", "Invalid email format"));
        }

        Ok(())
    }

    /// Validate password strength
    fn validate_password(&self, password: &str) -> Result<(), AppError> {
        if password.is_empty() {
            return Err(AppError::validation("password", "Password cannot be empty"));
        }

        if password.len() < self.config.min_password_length {
            return Err(AppError::validation(
                "password",
                &format!(
                    "Password must be at least {} characters long",
                    self.config.min_password_length
                ),
            ));
        }

        // Additional password strength checks
        if password.len() > 128 {
            return Err(AppError::validation(
                "password",
                "Password is too long (max 128 characters)",
            ));
        }

        // Check for common weak passwords
        if self.is_weak_password(password) {
            return Err(AppError::validation("password", "Password is too weak"));
        }

        Ok(())
    }

    /// Check for commonly weak passwords
    fn is_weak_password(&self, password: &str) -> bool {
        let weak_passwords = [
            "password",
            "123456",
            "password123",
            "admin",
            "qwerty",
            "letmein",
            "welcome",
            "monkey",
            "dragon",
            "secret",
        ];

        weak_passwords
            .iter()
            .any(|&weak| password.to_lowercase() == weak)
    }

    /// Validate that required fields are present
    fn validate_required_fields(&self, context: &BeforeEventContext) -> Result<(), AppError> {
        // For auth collections, email is always required
        if context.data.get("email").is_none() {
            return Err(AppError::validation("email", "Email is required"));
        }

        // For new records, password field must be present (it will contain hash after password hashing hook runs)
        if context.record_id.is_none() {
            // This is a create operation
            if context.data.get("password").is_none() {
                return Err(AppError::validation("password", "Password is required"));
            }
        }

        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &UserValidationConfig {
        &self.config
    }
}

impl Default for UserValidationHook {
    fn default() -> Self {
        Self::new().expect("Failed to create default UserValidationHook")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_email_validation() {
        let hook = UserValidationHook::new().unwrap();

        // Valid emails
        assert!(hook.validate_email("test@example.com").is_ok());
        assert!(hook.validate_email("user.name+tag@example.co.uk").is_ok());

        // Invalid emails
        assert!(hook.validate_email("").is_err());
        assert!(hook.validate_email("invalid").is_err());
        assert!(hook.validate_email("@example.com").is_err());
        assert!(hook.validate_email("test@").is_err());
    }

    #[test]
    fn test_password_validation() {
        let hook = UserValidationHook::new().unwrap();

        // Valid passwords
        assert!(hook.validate_password("strongPassword123").is_ok());
        assert!(hook.validate_password("mySecureP@ss").is_ok());

        // Invalid passwords
        assert!(hook.validate_password("").is_err()); // Empty
        assert!(hook.validate_password("short").is_err()); // Too short
        assert!(hook.validate_password("password").is_err()); // Weak
        assert!(hook.validate_password("123456").is_err()); // Weak
    }

    #[test]
    fn test_user_validation_hook() {
        let hook = UserValidationHook::new().unwrap();

        // Valid user data
        let mut context = BeforeEventContext::new_create(
            "_users".to_string(),
            json!({
                "email": "test@example.com",
                "password": "strongPassword123"
            }),
        );

        assert!(hook.handle_before_record_create(&mut context).is_ok());

        // Invalid user data - weak password
        context.data = json!({
            "email": "test@example.com",
            "password": "weak"
        });

        assert!(hook.handle_before_record_create(&mut context).is_err());

        // Invalid user data - bad email
        context.data = json!({
            "email": "invalid-email",
            "password": "strongPassword123"
        });

        assert!(hook.handle_before_record_create(&mut context).is_err());
    }

    #[test]
    fn test_custom_config() {
        let config = UserValidationConfig {
            auth_collections: vec!["custom_users".to_string()],
            min_password_length: 12,
            validate_email: true,
            check_duplicate_email: false,
            email_regex: Some(r"^[a-zA-Z0-9]+@company\.com$".to_string()),
        };

        let hook = UserValidationHook::with_config(config).unwrap();

        // Should accept company.com emails
        let mut context = BeforeEventContext::new_create(
            "custom_users".to_string(),
            json!({
                "email": "john@company.com",
                "password": "verylongpassword123"
            }),
        );

        assert!(hook.handle_before_record_create(&mut context).is_ok());

        // Should reject other domains
        context.data = json!({
            "email": "john@example.com",
            "password": "verylongpassword123"
        });

        assert!(hook.handle_before_record_create(&mut context).is_err());
    }

    #[test]
    fn test_validation_with_password_hash() {
        let hook = UserValidationHook::new().unwrap();

        // Test data that would result from password hashing hook running
        // The password field now contains the hash directly
        let mut context = BeforeEventContext::new_create(
            "_superusers".to_string(),
            json!({
                "email": "admin@example.com",
                "password": "$argon2id$v=19$m=65536,t=3,p=4$abcdef..."
            }),
        );

        // This should pass since password field is present (even though it contains a hash)
        assert!(hook.handle_before_record_create(&mut context).is_ok());

        // Test that missing password still fails
        context.data = json!({
            "email": "admin3@example.com"
        });

        assert!(hook.handle_before_record_create(&mut context).is_err());
    }

    #[test]
    fn test_integration_password_hashing_then_validation() {
        use crate::hooks::auth::password_hash::PasswordHashingHook;
        use crate::AuthService;
        use std::sync::Arc;

        // Simulate the exact scenario from the error:
        // 1. Password hashing hook runs first
        // 2. User validation hook runs second
        // 3. Should NOT fail with "Required field 'password' is missing"

        let auth_config = crate::auth::AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let password_hook = PasswordHashingHook::new(auth_service);
        let validation_hook = UserValidationHook::new().unwrap();

        // Create auth collection schema (like superusers)
        let mut schema =
            crate::CollectionSchema::new("_superusers".to_string(), crate::CollectionType::Auth);
        schema.add_field(
            "email".to_string(),
            crate::FieldDefinition {
                field_type: crate::FieldType::Email,
                required: true,
                unique: true,
                default: None,
                validation: None,
                index: false,
            },
        );
        schema.add_field(
            "password".to_string(),
            crate::FieldDefinition {
                field_type: crate::FieldType::Password,
                required: true,
                unique: false,
                default: None,
                validation: None,
                index: false,
            },
        );

        // Register the schema with the password hashing hook
        assert!(password_hook
            .register_schema("_superusers".to_string(), schema)
            .is_ok());

        // Original data with plain text password
        let mut context = BeforeEventContext::new_create(
            "_superusers".to_string(),
            json!({
                "email": "admin@example.com",
                "password": "securepassword123"
            }),
        );

        // Step 1: Password hashing hook processes the data
        assert!(password_hook
            .handle_before_record_create(&mut context)
            .is_ok());

        // Verify password was hashed and stored in the same field
        assert!(context.data.get("password").is_some());
        let password_value = context.data.get("password").unwrap().as_str().unwrap();
        assert_ne!(password_value, "securepassword123"); // Should be hashed
        assert!(password_value.starts_with("$argon2")); // Should be argon2 hash
        assert!(context.data.get("email").is_some());

        // Step 2: User validation hook should NOT fail
        let validation_result = validation_hook.handle_before_record_create(&mut context);
        assert!(
            validation_result.is_ok(),
            "User validation should pass after password hashing: {:?}",
            validation_result
        );
    }
}
