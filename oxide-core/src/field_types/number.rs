//! Number field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Numeric field type (integer or float)
#[derive(Debug, Clone)]
pub struct NumberFieldType;

impl FieldTypeDefinition for NumberFieldType {
    fn type_name(&self) -> &'static str {
        "number"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if !value.is_number() {
            return Err(format!("Field '{}' must be a number", field_name));
        }
        Ok(())
    }

    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        match value {
            JsonValue::Number(_) => Ok(value.clone()),
            JsonValue::String(s) => s
                .parse::<f64>()
                .map(|n| {
                    JsonValue::Number(
                        serde_json::Number::from_f64(n)
                            .unwrap_or_else(|| serde_json::Number::from(0)),
                    )
                })
                .map_err(|_| "Cannot convert string to number".to_string()),
            _ => Err("Cannot convert to number".to_string()),
        }
    }

    fn sql_type(&self) -> &'static str {
        "REAL"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_number_validation() {
        let field_type = NumberFieldType;

        // Valid numbers
        assert!(field_type.validate("test", &json!(123)).is_ok());
        assert!(field_type.validate("test", &json!(123.45)).is_ok());
        assert!(field_type.validate("test", &json!(-42)).is_ok());

        // Invalid types
        assert!(field_type.validate("test", &json!("hello")).is_err());
        assert!(field_type.validate("test", &json!(true)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }

    #[test]
    fn test_number_conversion() {
        let field_type = NumberFieldType;

        // Number stays number
        assert_eq!(field_type.convert_value(&json!(123)).unwrap(), json!(123));

        // String to number
        assert_eq!(
            field_type.convert_value(&json!("123.45")).unwrap(),
            json!(123.45)
        );

        // Invalid string conversion
        assert!(field_type.convert_value(&json!("not-a-number")).is_err());
    }
}
