//! Email field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Email field type (text with email validation)
#[derive(Debug, Clone)]
pub struct EmailFieldType;

impl FieldTypeDefinition for EmailFieldType {
    fn type_name(&self) -> &'static str {
        "email"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if let Some(email) = value.as_str() {
            if !email.contains('@') {
                return Err(format!(
                    "Field '{}' must be a valid email address",
                    field_name
                ));
            }
        } else {
            return Err(format!("Field '{}' must be a string", field_name));
        }
        Ok(())
    }

    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        match value {
            JsonValue::String(s) => {
                // Basic email validation
                if s.contains('@') && s.contains('.') {
                    Ok(value.clone())
                } else {
                    Err("Invalid email format".to_string())
                }
            }
            _ => Err("Email must be a string".to_string()),
        }
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
    fn test_email_validation() {
        let field_type = EmailFieldType;

        // Valid emails
        assert!(field_type
            .validate("test", &json!("user@example.com"))
            .is_ok());
        assert!(field_type
            .validate("test", &json!("test@domain.org"))
            .is_ok());

        // Invalid emails
        assert!(field_type
            .validate("test", &json!("invalid-email"))
            .is_err());
        assert!(field_type.validate("test", &json!("no-at-sign")).is_err());

        // Invalid types
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!(true)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }

    #[test]
    fn test_email_conversion() {
        let field_type = EmailFieldType;

        // Valid email
        assert_eq!(
            field_type
                .convert_value(&json!("test@example.com"))
                .unwrap(),
            json!("test@example.com")
        );

        // Invalid email format
        assert!(field_type.convert_value(&json!("invalid-email")).is_err());
        assert!(field_type.convert_value(&json!("no-at-sign")).is_err());

        // Non-string input
        assert!(field_type.convert_value(&json!(123)).is_err());
    }
}
