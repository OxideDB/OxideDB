//! Field type definitions for collection schemas
//!
//! This module provides an extensible field type system for OxideDB collection schemas.
//! Each field type is defined in its own module, making it easy to add new field types
//! and customize their behavior.
//!
//! ## Architecture
//!
//! - Each field type implements the `FieldTypeDefinition` trait
//! - Field types can define custom validation logic
//! - Field types can specify if they require special processing (e.g., hashing)
//! - The main `FieldType` enum aggregates all available field types
//!
//! ## Adding New Field Types
//!
//! To add a new field type:
//! 1. Create a new module file (e.g., `my_type.rs`)
//! 2. Implement the `FieldTypeDefinition` trait
//! 3. Add the new variant to the `FieldType` enum
//! 4. Update the match statements in the implementation

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;

// Field type modules
pub mod text;
pub mod number;
pub mod boolean;
pub mod date;
pub mod json;
pub mod email;
pub mod url;
pub mod password;
pub mod phone;
pub mod relationship;
pub use text::TextFieldType;
pub use number::NumberFieldType;
pub use boolean::BooleanFieldType;
pub use date::DateFieldType;
pub use json::JsonFieldType;
pub use email::EmailFieldType;
pub use url::UrlFieldType;
pub use password::PasswordFieldType;
pub use phone::PhoneFieldType;
pub use relationship::{RelationshipFieldType, RelationshipConfig};

/// Trait that all field types must implement
pub trait FieldTypeDefinition {
    /// Get the string identifier for this field type
    fn type_name(&self) -> &'static str;
    
    /// Validate a value against this field type
    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String>;
    
    /// Check if this field type requires special processing (e.g., hashing)
    fn requires_hashing(&self) -> bool {
        false
    }
    
    /// Convert a value to the expected type (for auto-conversion)
    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        // Default implementation: no conversion
        Ok(value.clone())
    }
    
    /// Get the SQL column type for database storage
    fn sql_type(&self) -> &'static str;
}

/// Supported field types in collection schemas
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
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
    /// Phone number field (text with phone number validation)
    Phone,
    /// Relationship field (references to other collections)
    Relationship(RelationshipConfig),
}

impl FieldType {
    /// Get the field type definition for this field type
    pub fn definition(&self) -> Box<dyn FieldTypeDefinition> {
        match self {
            FieldType::Text => Box::new(TextFieldType),
            FieldType::Number => Box::new(NumberFieldType),
            FieldType::Boolean => Box::new(BooleanFieldType),
            FieldType::Date => Box::new(DateFieldType),
            FieldType::Json => Box::new(JsonFieldType),
            FieldType::Email => Box::new(EmailFieldType),
            FieldType::Url => Box::new(UrlFieldType),
            FieldType::Password => Box::new(PasswordFieldType),
            FieldType::Phone => Box::new(PhoneFieldType),
            FieldType::Relationship(config) => Box::new(RelationshipFieldType::new(config.clone())),
        }
    }
    
    /// Check if this field type should be automatically hashed
    pub fn requires_hashing(&self) -> bool {
        self.definition().requires_hashing()
    }
    
    /// Validate a value against this field type
    pub fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        self.definition().validate(field_name, value)
    }
    
    /// Convert a value to the expected type
    pub fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        self.definition().convert_value(value)
    }
    
    /// Get the SQL column type for database storage
    pub fn sql_type(&self) -> &'static str {
        self.definition().sql_type()
    }
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.definition().type_name())
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
    
    #[test]
    fn test_field_type_validation() {
        use serde_json::json;
        
        // Test text validation
        assert!(FieldType::Text.validate("test", &json!("hello")).is_ok());
        assert!(FieldType::Text.validate("test", &json!(123)).is_err());
        
        // Test number validation
        assert!(FieldType::Number.validate("test", &json!(123)).is_ok());
        assert!(FieldType::Number.validate("test", &json!("hello")).is_err());
        
        // Test email validation
        assert!(FieldType::Email.validate("test", &json!("test@example.com")).is_ok());
        assert!(FieldType::Email.validate("test", &json!("invalid-email")).is_err());
    }

    #[test]
    fn test_relationship_field_type() {
        use serde_json::json;
        
        // Test single relationship
        let single_config = RelationshipConfig {
            target_collection: "users".to_string(),
            multiple: false,
            cascade_delete: false,
            display_field: None,
        };
        let single_relationship = FieldType::Relationship(single_config);
        
        assert_eq!(single_relationship.to_string(), "relationship");
        assert!(single_relationship.validate("test", &json!("user-123")).is_ok());
        assert!(single_relationship.validate("test", &json!(["id1", "id2"])).is_err());
        
        // Test multiple relationship
        let multiple_config = RelationshipConfig {
            target_collection: "tags".to_string(),
            multiple: true,
            cascade_delete: false,
            display_field: Some("name".to_string()),
        };
        let multiple_relationship = FieldType::Relationship(multiple_config);
        
        assert!(multiple_relationship.validate("test", &json!(["tag-12345678", "tag-87654321"])).is_ok());
        assert!(multiple_relationship.validate("test", &json!("single-id")).is_err());
    }
} 