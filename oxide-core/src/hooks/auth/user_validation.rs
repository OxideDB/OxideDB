//! User Validation Hook
//!
//! This hook validates user data in authentication collections to ensure
//! data integrity and security requirements are met.

use crate::{BeforeEventContext, AppError};
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
            auth_collections: vec!["users".to_string(), "superusers".to_string()],
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
        let email_pattern = config.email_regex.as_deref()
            .unwrap_or(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$");
        
        let email_regex = Arc::new(
            Regex::new(email_pattern)
                .map_err(|e| AppError::validation("email_regex", &format!("Invalid regex pattern: {}", e)))?
        );

        Ok(Self {
            config,
            email_regex,
        })
    }

    /// Hook that processes BeforeRecordCreate events for user validation
    pub fn handle_before_record_create(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.should_process_collection(&context.collection) {
            debug!("Validating user data for create in collection: {}", context.collection);
            self.validate_user_data(context)?;
        }
        Ok(())
    }

    /// Hook that processes BeforeRecordUpdate events for user validation
    pub fn handle_before_record_update(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.should_process_collection(&context.collection) {
            debug!("Validating user data for update in collection: {}", context.collection);
            self.validate_user_data(context)?;
        }
        Ok(())
    }

    /// Check if this collection should have user validation applied
    fn should_process_collection(&self, collection: &str) -> bool {
        self.config.auth_collections.contains(&collection.to_string())
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
            return Err(AppError::validation("email", "Email is too long (max 254 characters)"));
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
                &format!("Password must be at least {} characters long", self.config.min_password_length)
            ));
        }

        // Additional password strength checks
        if password.len() > 128 {
            return Err(AppError::validation("password", "Password is too long (max 128 characters)"));
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
            "password", "123456", "password123", "admin", "qwerty",
            "letmein", "welcome", "monkey", "dragon", "secret"
        ];

        weak_passwords.iter().any(|&weak| password.to_lowercase() == weak)
    }

    /// Validate that required fields are present
    fn validate_required_fields(&self, context: &BeforeEventContext) -> Result<(), AppError> {
        // For auth collections, email is always required
        if !context.data.get("email").is_some() {
            return Err(AppError::validation("email", "Email is required"));
        }

        // For new records, either password or passwordHash must be present
        if context.record_id.is_none() { // This is a create operation
            let has_password = context.data.get("password").is_some();
            let has_password_hash = context.data.get("passwordHash").is_some();
            
            if !has_password && !has_password_hash {
                return Err(AppError::validation("password", "Password or passwordHash is required"));
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
        let mut context = BeforeEventContext {
            collection: "users".to_string(),
            data: json!({
                "email": "test@example.com",
                "password": "strongPassword123"
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

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
        let mut context = BeforeEventContext {
            collection: "custom_users".to_string(),
            data: json!({
                "email": "john@company.com",
                "password": "verylongpassword123"
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        assert!(hook.handle_before_record_create(&mut context).is_ok());

        // Should reject other domains
        context.data = json!({
            "email": "john@example.com",
            "password": "verylongpassword123"
        });

        assert!(hook.handle_before_record_create(&mut context).is_err());
    }
} 