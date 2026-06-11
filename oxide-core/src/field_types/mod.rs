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
//! - Validation rules support regex patterns and custom constraints
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
pub mod boolean;
pub mod date;
pub mod email;
pub mod file;
pub mod json;
pub mod number;
pub mod password;
pub mod phone;
pub mod relationship;
pub mod select;
pub mod text;
pub mod url;
pub use boolean::BooleanFieldType;
pub use date::DateFieldType;
pub use email::EmailFieldType;
pub use file::{FileFieldConfig, FileFieldType, FileReference};
pub use json::JsonFieldType;
pub use number::NumberFieldType;
pub use password::PasswordFieldType;
pub use phone::PhoneFieldType;
pub use relationship::{RelationshipConfig, RelationshipFieldType};
pub use select::{SelectConfig, SelectFieldType};
pub use text::TextFieldType;
pub use url::UrlFieldType;

/// Validation rules that can be applied to field values
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct ValidationRules {
    /// Regular expression pattern for string validation
    pub regex: Option<String>,
    /// Minimum length for strings or minimum value for numbers
    pub min: Option<f64>,
    /// Maximum length for strings or maximum value for numbers
    pub max: Option<f64>,
    /// Custom error message for validation failures
    pub message: Option<String>,
    /// Whether to allow empty values (overrides field-level required setting)
    pub allow_empty: Option<bool>,
}

impl ValidationRules {
    /// Create empty validation rules
    pub fn new() -> Self {
        Self {
            regex: None,
            min: None,
            max: None,
            message: None,
            allow_empty: None,
        }
    }

    /// Add a regex pattern for validation
    pub fn with_regex(mut self, pattern: String) -> Self {
        self.regex = Some(pattern);
        self
    }

    /// Add minimum value/length constraint
    pub fn with_min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Add maximum value/length constraint  
    pub fn with_max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Add custom error message
    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    /// Set whether to allow empty values
    pub fn with_allow_empty(mut self, allow_empty: bool) -> Self {
        self.allow_empty = Some(allow_empty);
        self
    }

    /// Validate a value against these rules
    pub fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        // Check empty values
        let is_empty = match value {
            JsonValue::Null => true,
            JsonValue::String(s) => s.is_empty(),
            JsonValue::Array(a) => a.is_empty(),
            _ => false,
        };

        if is_empty {
            if let Some(allow_empty) = self.allow_empty {
                if !allow_empty {
                    return Err(self.custom_message(field_name, "Field cannot be empty"));
                }
            }
            // If empty and allowed, skip other validations
            return Ok(());
        }

        // Regex validation for strings
        if let Some(pattern) = &self.regex {
            if let Some(text) = value.as_str() {
                match regex::Regex::new(pattern) {
                    Ok(re) => {
                        if !re.is_match(text) {
                            return Err(self.custom_message(
                                field_name,
                                &format!("Field must match pattern: {}", pattern),
                            ));
                        }
                    }
                    Err(_) => {
                        return Err(format!("Invalid regex pattern: {}", pattern));
                    }
                }
            }
        }

        // Length/value constraints
        if let Some(min) = self.min {
            match value {
                JsonValue::String(s) if (s.len() as f64) < min => {
                    return Err(self.custom_message(
                        field_name,
                        &format!("Field must be at least {} characters long", min),
                    ));
                }
                JsonValue::Number(n) => {
                    if let Some(num_val) = n.as_f64() {
                        if num_val < min {
                            return Err(self.custom_message(
                                field_name,
                                &format!("Field must be at least {}", min),
                            ));
                        }
                    }
                }
                JsonValue::Array(a) if (a.len() as f64) < min => {
                    return Err(self.custom_message(
                        field_name,
                        &format!("Field must have at least {} items", min),
                    ));
                }
                _ => {}
            }
        }

        if let Some(max) = self.max {
            match value {
                JsonValue::String(s) if (s.len() as f64) > max => {
                    return Err(self.custom_message(
                        field_name,
                        &format!("Field must be at most {} characters long", max),
                    ));
                }
                JsonValue::Number(n) => {
                    if let Some(num_val) = n.as_f64() {
                        if num_val > max {
                            return Err(self.custom_message(
                                field_name,
                                &format!("Field must be at most {}", max),
                            ));
                        }
                    }
                }
                JsonValue::Array(a) if (a.len() as f64) > max => {
                    return Err(self.custom_message(
                        field_name,
                        &format!("Field must have at most {} items", max),
                    ));
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Get custom error message or use default
    fn custom_message(&self, field_name: &str, default: &str) -> String {
        self.message
            .as_ref()
            .map(|msg| msg.replace("{field}", field_name))
            .unwrap_or_else(|| format!("Field '{}': {}", field_name, default))
    }
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait that all field types must implement
pub trait FieldTypeDefinition {
    /// Get the string identifier for this field type
    fn type_name(&self) -> &'static str;

    /// Validate a value against this field type
    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String>;

    /// Validate a value with additional validation rules
    fn validate_with_rules(
        &self,
        field_name: &str,
        value: &JsonValue,
        rules: &ValidationRules,
    ) -> Result<(), String> {
        // First validate against the base field type
        self.validate(field_name, value)?;
        // Then apply additional validation rules
        rules.validate(field_name, value)
    }

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
    /// File field (references to files in virtual filesystem)
    File(FileFieldConfig),
    /// Select field (dropdown with predefined options)
    Select(SelectConfig),
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
            FieldType::File(config) => Box::new(FileFieldType::new(config.clone())),
            FieldType::Select(config) => Box::new(SelectFieldType::new(config.clone())),
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
        assert!(FieldType::Email
            .validate("test", &json!("test@example.com"))
            .is_ok());
        assert!(FieldType::Email
            .validate("test", &json!("invalid-email"))
            .is_err());
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
        assert!(single_relationship
            .validate("test", &json!("user-123"))
            .is_ok());
        assert!(single_relationship
            .validate("test", &json!(["id1", "id2"]))
            .is_err());

        // Test multiple relationship
        let multiple_config = RelationshipConfig {
            target_collection: "tags".to_string(),
            multiple: true,
            cascade_delete: false,
            display_field: Some("name".to_string()),
        };
        let multiple_relationship = FieldType::Relationship(multiple_config);

        assert!(multiple_relationship
            .validate("test", &json!(["tag-12345678", "tag-87654321"]))
            .is_ok());
        assert!(multiple_relationship
            .validate("test", &json!("single-id"))
            .is_err());
    }

    #[test]
    fn test_select_field_type() {
        use serde_json::json;

        // Test single select
        let single_config = SelectConfig {
            options: vec![
                "option1".to_string(),
                "option2".to_string(),
                "option3".to_string(),
            ],
            multiple: false,
            allow_empty: true,
        };
        let single_select = FieldType::Select(single_config);

        assert_eq!(single_select.to_string(), "select");
        assert!(single_select.validate("test", &json!("option1")).is_ok());
        assert!(single_select.validate("test", &json!("option2")).is_ok());
        assert!(single_select.validate("test", &json!("invalid")).is_err());
        assert!(single_select.validate("test", &json!(["option1"])).is_err());

        // Test multiple select
        let multiple_config = SelectConfig {
            options: vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()],
            multiple: true,
            allow_empty: true,
        };
        let multiple_select = FieldType::Select(multiple_config);

        assert!(multiple_select
            .validate("test", &json!(["tag1", "tag2"]))
            .is_ok());
        assert!(multiple_select
            .validate("test", &json!(["tag1", "invalid"]))
            .is_err());
        assert!(multiple_select.validate("test", &json!("tag1")).is_err());
    }

    #[test]
    fn test_validation_rules_regex() {
        use serde_json::json;

        // Test email regex validation
        let email_rules = ValidationRules::new().with_regex(r"^[^@]+@[^@]+\.[^@]+$".to_string());

        assert!(email_rules
            .validate("email", &json!("user@example.com"))
            .is_ok());
        assert!(email_rules
            .validate("email", &json!("test@domain.org"))
            .is_ok());
        assert!(email_rules
            .validate("email", &json!("invalid-email"))
            .is_err());
        assert!(email_rules.validate("email", &json!("no-at-sign")).is_err());

        // Test username regex validation
        let username_rules = ValidationRules::new().with_regex(r"^[a-zA-Z0-9_]+$".to_string());

        assert!(username_rules
            .validate("username", &json!("user123"))
            .is_ok());
        assert!(username_rules
            .validate("username", &json!("user_name"))
            .is_ok());
        assert!(username_rules
            .validate("username", &json!("user-name"))
            .is_err());
        assert!(username_rules
            .validate("username", &json!("user@name"))
            .is_err());
    }

    #[test]
    fn test_validation_rules_min_max() {
        use serde_json::json;

        // Test string length validation
        let length_rules = ValidationRules::new().with_min(3.0).with_max(10.0);

        assert!(length_rules.validate("text", &json!("hello")).is_ok());
        assert!(length_rules.validate("text", &json!("ab")).is_err()); // too short
        assert!(length_rules
            .validate("text", &json!("this_is_too_long"))
            .is_err()); // too long

        // Test number validation
        let number_rules = ValidationRules::new().with_min(0.0).with_max(100.0);

        assert!(number_rules.validate("score", &json!(50)).is_ok());
        assert!(number_rules.validate("score", &json!(0)).is_ok());
        assert!(number_rules.validate("score", &json!(100)).is_ok());
        assert!(number_rules.validate("score", &json!(-10)).is_err()); // too low
        assert!(number_rules.validate("score", &json!(150)).is_err()); // too high

        // Test array length validation
        let array_rules = ValidationRules::new().with_min(2.0).with_max(5.0);

        assert!(array_rules
            .validate("tags", &json!(["tag1", "tag2", "tag3"]))
            .is_ok());
        assert!(array_rules.validate("tags", &json!(["tag1"])).is_err()); // too few
        assert!(array_rules
            .validate(
                "tags",
                &json!(["tag1", "tag2", "tag3", "tag4", "tag5", "tag6"])
            )
            .is_err()); // too many
    }

    #[test]
    fn test_validation_rules_empty_values() {
        use serde_json::json;

        // Test allow_empty = false
        let strict_rules = ValidationRules::new().with_allow_empty(false);

        assert!(strict_rules.validate("field", &json!("hello")).is_ok());
        assert!(strict_rules.validate("field", &json!(null)).is_err());
        assert!(strict_rules.validate("field", &json!("")).is_err());
        assert!(strict_rules.validate("field", &json!([])).is_err());

        // Test allow_empty = true (default behavior)
        let permissive_rules = ValidationRules::new().with_allow_empty(true);

        assert!(permissive_rules.validate("field", &json!("hello")).is_ok());
        assert!(permissive_rules.validate("field", &json!(null)).is_ok());
        assert!(permissive_rules.validate("field", &json!("")).is_ok());
        assert!(permissive_rules.validate("field", &json!([])).is_ok());
    }

    #[test]
    fn test_validation_rules_custom_message() {
        use serde_json::json;

        let rules = ValidationRules::new()
            .with_regex(r"^[A-Z]+$".to_string())
            .with_message("Field {field} must contain only uppercase letters".to_string());

        let result = rules.validate("test_field", &json!("lowercase"));
        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("test_field"));
        assert!(error_msg.contains("must contain only uppercase letters"));
    }

    #[test]
    fn test_validation_rules_combined() {
        use serde_json::json;

        // Test combined regex and length validation
        let combined_rules = ValidationRules::new()
            .with_regex(r"^[a-zA-Z0-9_]+$".to_string())
            .with_min(3.0)
            .with_max(20.0)
            .with_message("Username must be 3-20 characters and contain only letters, numbers, and underscores".to_string());

        assert!(combined_rules
            .validate("username", &json!("valid_user123"))
            .is_ok());
        assert!(combined_rules.validate("username", &json!("ab")).is_err()); // too short
        assert!(combined_rules
            .validate("username", &json!("this_username_is_way_too_long"))
            .is_err()); // too long
        assert!(combined_rules
            .validate("username", &json!("invalid-user"))
            .is_err()); // invalid chars
    }

    #[test]
    fn test_field_type_with_validation_rules() {
        use serde_json::json;

        let text_field = TextFieldType;
        let rules = ValidationRules::new()
            .with_min(5.0)
            .with_regex(r"^[A-Z].*".to_string()); // Must start with uppercase

        // Valid: starts with uppercase and long enough
        assert!(text_field
            .validate_with_rules("title", &json!("Hello World"), &rules)
            .is_ok());

        // Invalid: doesn't start with uppercase
        assert!(text_field
            .validate_with_rules("title", &json!("hello world"), &rules)
            .is_err());

        // Invalid: too short
        assert!(text_field
            .validate_with_rules("title", &json!("Hi"), &rules)
            .is_err());

        // Invalid: wrong type (should fail base validation first)
        assert!(text_field
            .validate_with_rules("title", &json!(123), &rules)
            .is_err());
    }
}
