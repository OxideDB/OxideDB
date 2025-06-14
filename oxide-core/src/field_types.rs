//! Field type definitions for collection schemas
//!
//! This module defines the supported field types in OxideDB collection schemas.
//! Each field type corresponds to a specific data type and validation rules.
//!
//! ## Password Field Type
//!
//! The `Password` field type is a special field that automatically triggers password
//! hashing when used in collection schemas. When the `PasswordHashingHook` is configured
//! in schema-aware mode (default), any field defined with `FieldType::Password` will:
//!
//! 1. Automatically hash the plain text password before storage
//! 2. Store the hash directly in the same password field
//! 3. Add metadata about the hashing operation
//!
//! ### Example Usage
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

use serde::{Deserialize, Serialize};

/// Supported field types in collection schemas
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// Text/string field
    Text,
    /// Numeric field (integer or float)
    Number,
    /// Boolean field
    Boolean,
    /// Date/timestamp field
    Date,
    /// JSON object field
    Json,
    /// Email field (text with email validation)
    Email,
    /// URL field (text with URL validation)
    Url,
    /// Password field (automatically hashed before storage)
    Password,
}

impl FieldType {
    /// Check if this field type should be automatically hashed
    pub fn requires_hashing(&self) -> bool {
        matches!(self, FieldType::Password)
    }
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::Text => write!(f, "text"),
            FieldType::Number => write!(f, "number"),
            FieldType::Boolean => write!(f, "boolean"),
            FieldType::Date => write!(f, "date"),
            FieldType::Json => write!(f, "json"),
            FieldType::Email => write!(f, "email"),
            FieldType::Url => write!(f, "url"),
            FieldType::Password => write!(f, "password"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing_requirements() {
        assert!(FieldType::Password.requires_hashing());
        assert!(!FieldType::Text.requires_hashing());
        assert!(!FieldType::Email.requires_hashing());
    }

    #[test]
    fn test_field_type_display() {
        assert_eq!(FieldType::Password.to_string(), "password");
        assert_eq!(FieldType::Email.to_string(), "email");
        assert_eq!(FieldType::Text.to_string(), "text");
    }
} 