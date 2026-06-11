//! Phone number field type definition
//!
//! This is an example of how to add a new field type to the system.
//! This field type validates phone numbers with basic format checking.

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Phone number field type (text with phone number validation)
#[derive(Debug, Clone)]
pub struct PhoneFieldType;

impl FieldTypeDefinition for PhoneFieldType {
    fn type_name(&self) -> &'static str {
        "phone"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if let Some(phone) = value.as_str() {
            // Basic phone number validation - must contain only digits, spaces, hyphens, parentheses, and plus
            let valid_chars = phone.chars().all(|c| {
                c.is_ascii_digit() || c == ' ' || c == '-' || c == '(' || c == ')' || c == '+'
            });

            if !valid_chars {
                return Err(format!(
                    "Field '{}' must be a valid phone number",
                    field_name
                ));
            }

            // Must have at least 10 digits
            let digit_count = phone.chars().filter(|c| c.is_ascii_digit()).count();
            if digit_count < 10 {
                return Err(format!(
                    "Field '{}' must contain at least 10 digits",
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
                // Normalize phone number by removing all non-digit characters except +
                let normalized: String = s
                    .chars()
                    .filter(|c| c.is_ascii_digit() || *c == '+')
                    .collect();

                if normalized.len() >= 10 {
                    Ok(JsonValue::String(normalized))
                } else {
                    Err("Invalid phone number format".to_string())
                }
            }
            _ => Err("Phone number must be a string".to_string()),
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
    fn test_phone_validation() {
        let field_type = PhoneFieldType;

        // Valid phone numbers
        assert!(field_type
            .validate("test", &json!("+1-555-123-4567"))
            .is_ok());
        assert!(field_type
            .validate("test", &json!("(555) 123-4567"))
            .is_ok());
        assert!(field_type.validate("test", &json!("5551234567")).is_ok());

        // Invalid phone numbers
        assert!(field_type.validate("test", &json!("123")).is_err()); // Too short
        assert!(field_type.validate("test", &json!("abc-def-ghij")).is_err()); // No digits
        assert!(field_type.validate("test", &json!("555-123-456a")).is_err()); // Invalid character

        // Invalid types
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!(true)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }

    #[test]
    fn test_phone_conversion() {
        let field_type = PhoneFieldType;

        // Normalize phone numbers
        assert_eq!(
            field_type.convert_value(&json!("+1-555-123-4567")).unwrap(),
            json!("+15551234567")
        );
        assert_eq!(
            field_type.convert_value(&json!("(555) 123-4567")).unwrap(),
            json!("5551234567")
        );

        // Invalid phone format
        assert!(field_type.convert_value(&json!("123")).is_err());

        // Non-string input
        assert!(field_type.convert_value(&json!(123)).is_err());
    }
}
