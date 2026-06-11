//! Password field type definition
//!
//! The Password field type is special because it requires automatic hashing
//! before storage. When the `PasswordHashingHook` is configured in schema-aware
//! mode (default), any field defined with `FieldType::Password` will:
//!
//! 1. Automatically hash the plain text password before storage
//! 2. Store the hash directly in the same password field
//! 3. Add metadata about the hashing operation
//!
//! ## Example Usage
//!
//! ```rust
//! use oxide_core::{CollectionSchema, CollectionType, FieldDefinition, FieldType};
//!
//! let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
//! schema.add_field("password".to_string(), FieldDefinition {
//!     field_type: FieldType::Password,  // This will be auto-hashed
//!     required: true,
//!     unique: false,
//!     default: None,
//!     validation: None,
//!     index: false,
//! });
//! ```
//!
//! With this schema, when a record is created with:
//! ```json
//! {"email": "user@example.com", "password": "plaintext123"}
//! ```
//!
//! The stored data becomes:
//! ```json
//! {"email": "user@example.com", "password": "$argon2..."}
//! ```

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Password field type (automatically hashed before storage)
#[derive(Debug, Clone)]
pub struct PasswordFieldType;

impl FieldTypeDefinition for PasswordFieldType {
    fn type_name(&self) -> &'static str {
        "password"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if !value.is_string() {
            return Err(format!("Field '{}' must be a string", field_name));
        }

        if let Some(password) = value.as_str() {
            if password.is_empty() {
                return Err(format!("Field '{}' cannot be empty", field_name));
            }
            // Note: Password will be hashed by the password hashing hook
        }

        Ok(())
    }

    fn requires_hashing(&self) -> bool {
        true
    }

    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        // Password fields should be strings
        match value {
            JsonValue::String(_) => Ok(value.clone()),
            _ => Err("Password must be a string".to_string()),
        }
    }

    fn sql_type(&self) -> &'static str {
        "TEXT" // Store as hashed string
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_password_validation() {
        let field_type = PasswordFieldType;

        // Valid passwords
        assert!(field_type.validate("test", &json!("secret123")).is_ok());
        assert!(field_type
            .validate("test", &json!("complex_password!"))
            .is_ok());

        // Empty password should fail
        assert!(field_type.validate("test", &json!("")).is_err());

        // Invalid types
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!(true)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }

    #[test]
    fn test_password_conversion() {
        let field_type = PasswordFieldType;

        // String stays string
        assert_eq!(
            field_type.convert_value(&json!("password123")).unwrap(),
            json!("password123")
        );

        // Non-string input fails
        assert!(field_type.convert_value(&json!(123)).is_err());
        assert!(field_type.convert_value(&json!(true)).is_err());
    }

    #[test]
    fn test_password_requires_hashing() {
        let field_type = PasswordFieldType;
        assert!(field_type.requires_hashing());
    }
}
