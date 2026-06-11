//! Select field type implementation
//!
//! This field type allows creating dropdown/select fields with predefined options.
//! It supports both single and multiple selection modes.

use super::FieldTypeDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;

/// Configuration for select fields
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct SelectConfig {
    /// Available options for selection
    pub options: Vec<String>,
    /// Whether multiple values can be selected
    pub multiple: bool,
    /// Whether empty/null values are allowed
    pub allow_empty: bool,
}

/// Select field type implementation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectFieldType {
    /// Configuration for this select field
    pub config: SelectConfig,
}

impl SelectFieldType {
    /// Create a new select field type
    pub fn new(config: SelectConfig) -> Self {
        Self { config }
    }

    /// Create a single-select field with given options
    pub fn single(options: Vec<String>) -> Self {
        Self::new(SelectConfig {
            options,
            multiple: false,
            allow_empty: true,
        })
    }

    /// Create a multi-select field with given options
    pub fn multiple(options: Vec<String>) -> Self {
        Self::new(SelectConfig {
            options,
            multiple: true,
            allow_empty: true,
        })
    }

    /// Set whether empty values are allowed
    pub fn with_allow_empty(mut self, allow_empty: bool) -> Self {
        self.config.allow_empty = allow_empty;
        self
    }

    /// Validate a single option value
    fn validate_single_option(&self, value: &str) -> Result<(), String> {
        if !self.config.options.contains(&value.to_string()) {
            return Err(format!(
                "Value '{}' is not a valid option. Valid options are: [{}]",
                value,
                self.config.options.join(", ")
            ));
        }
        Ok(())
    }

    /// Validate multiple option values
    fn validate_multiple_options(&self, values: &[JsonValue]) -> Result<(), String> {
        for value in values {
            if let JsonValue::String(s) = value {
                self.validate_single_option(s)?;
            } else {
                return Err("All values in multiple select must be strings".to_string());
            }
        }
        Ok(())
    }
}

impl FieldTypeDefinition for SelectFieldType {
    fn type_name(&self) -> &'static str {
        "select"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        // Handle null/empty values
        if value.is_null() {
            if self.config.allow_empty {
                return Ok(());
            } else {
                return Err(format!("Field '{}' cannot be empty", field_name));
            }
        }

        if self.config.multiple {
            // Multiple selection mode
            match value {
                JsonValue::Array(arr) => {
                    if arr.is_empty() && !self.config.allow_empty {
                        return Err(format!("Field '{}' cannot be empty", field_name));
                    }
                    self.validate_multiple_options(arr)?;
                }
                _ => {
                    return Err(format!(
                        "Field '{}' must be an array for multiple select",
                        field_name
                    ));
                }
            }
        } else {
            // Single selection mode
            match value {
                JsonValue::String(s) => {
                    if s.is_empty() && !self.config.allow_empty {
                        return Err(format!("Field '{}' cannot be empty", field_name));
                    }
                    if !s.is_empty() {
                        self.validate_single_option(s)?;
                    }
                }
                _ => {
                    return Err(format!(
                        "Field '{}' must be a string for single select",
                        field_name
                    ));
                }
            }
        }

        Ok(())
    }

    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        // For select fields, we don't do automatic conversion
        // Values must match the expected format exactly
        Ok(value.clone())
    }

    fn sql_type(&self) -> &'static str {
        "TEXT"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_single_select_validation() {
        let field_type = SelectFieldType::single(vec![
            "option1".to_string(),
            "option2".to_string(),
            "option3".to_string(),
        ]);

        // Valid options
        assert!(field_type.validate("test", &json!("option1")).is_ok());
        assert!(field_type.validate("test", &json!("option2")).is_ok());
        assert!(field_type.validate("test", &json!("option3")).is_ok());

        // Invalid option
        assert!(field_type.validate("test", &json!("invalid")).is_err());

        // Wrong type
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!(["option1"])).is_err());

        // Empty value (should be allowed by default)
        assert!(field_type.validate("test", &json!("")).is_ok());
        assert!(field_type.validate("test", &json!(null)).is_ok());
    }

    #[test]
    fn test_multiple_select_validation() {
        let field_type = SelectFieldType::multiple(vec![
            "tag1".to_string(),
            "tag2".to_string(),
            "tag3".to_string(),
        ]);

        // Valid multiple selections
        assert!(field_type.validate("test", &json!(["tag1"])).is_ok());
        assert!(field_type
            .validate("test", &json!(["tag1", "tag2"]))
            .is_ok());
        assert!(field_type
            .validate("test", &json!(["tag1", "tag2", "tag3"]))
            .is_ok());

        // Invalid options in array
        assert!(field_type
            .validate("test", &json!(["tag1", "invalid"]))
            .is_err());

        // Wrong type
        assert!(field_type.validate("test", &json!("tag1")).is_err());
        assert!(field_type.validate("test", &json!(123)).is_err());

        // Empty array (should be allowed by default)
        assert!(field_type.validate("test", &json!([])).is_ok());
        assert!(field_type.validate("test", &json!(null)).is_ok());
    }

    #[test]
    fn test_select_with_no_empty_allowed() {
        let field_type =
            SelectFieldType::single(vec!["option1".to_string()]).with_allow_empty(false);

        // Valid option
        assert!(field_type.validate("test", &json!("option1")).is_ok());

        // Empty values should be rejected
        assert!(field_type.validate("test", &json!("")).is_err());
        assert!(field_type.validate("test", &json!(null)).is_err());
    }

    #[test]
    fn test_select_field_type_name() {
        let field_type = SelectFieldType::single(vec!["option1".to_string()]);
        assert_eq!(field_type.type_name(), "select");
    }

    #[test]
    fn test_select_sql_type() {
        let single_select = SelectFieldType::single(vec!["option1".to_string()]);
        let multiple_select = SelectFieldType::multiple(vec!["option1".to_string()]);

        assert_eq!(single_select.sql_type(), "TEXT");
        assert_eq!(multiple_select.sql_type(), "TEXT");
    }

    #[test]
    fn test_select_convert_value() {
        let field_type = SelectFieldType::single(vec!["option1".to_string()]);
        let value = json!("option1");

        // Select fields don't do conversion, they return the value as-is
        assert_eq!(field_type.convert_value(&value).unwrap(), value);
    }
}
