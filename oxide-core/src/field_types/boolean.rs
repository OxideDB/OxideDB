//! Boolean field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Boolean field type
#[derive(Debug, Clone)]
pub struct BooleanFieldType;

impl FieldTypeDefinition for BooleanFieldType {
    fn type_name(&self) -> &'static str {
        "boolean"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if !value.is_boolean() {
            return Err(format!("Field '{}' must be a boolean", field_name));
        }
        Ok(())
    }

    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        match value {
            JsonValue::Bool(_) => Ok(value.clone()),
            JsonValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "1" | "yes" | "on" => Ok(JsonValue::Bool(true)),
                "false" | "0" | "no" | "off" => Ok(JsonValue::Bool(false)),
                _ => Err("Cannot convert string to boolean".to_string()),
            },
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(JsonValue::Bool(i != 0))
                } else {
                    Err("Cannot convert number to boolean".to_string())
                }
            }
            _ => Err("Cannot convert to boolean".to_string()),
        }
    }

    fn sql_type(&self) -> &'static str {
        "INTEGER" // SQLite doesn't have native boolean
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_boolean_validation() {
        let field_type = BooleanFieldType;

        // Valid booleans
        assert!(field_type.validate("test", &json!(true)).is_ok());
        assert!(field_type.validate("test", &json!(false)).is_ok());

        // Invalid types
        assert!(field_type.validate("test", &json!("hello")).is_err());
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }

    #[test]
    fn test_boolean_conversion() {
        let field_type = BooleanFieldType;

        // Boolean stays boolean
        assert_eq!(field_type.convert_value(&json!(true)).unwrap(), json!(true));
        assert_eq!(
            field_type.convert_value(&json!(false)).unwrap(),
            json!(false)
        );

        // String to boolean
        assert_eq!(
            field_type.convert_value(&json!("true")).unwrap(),
            json!(true)
        );
        assert_eq!(
            field_type.convert_value(&json!("false")).unwrap(),
            json!(false)
        );
        assert_eq!(field_type.convert_value(&json!("1")).unwrap(), json!(true));
        assert_eq!(field_type.convert_value(&json!("0")).unwrap(), json!(false));
        assert_eq!(
            field_type.convert_value(&json!("yes")).unwrap(),
            json!(true)
        );
        assert_eq!(
            field_type.convert_value(&json!("no")).unwrap(),
            json!(false)
        );

        // Number to boolean
        assert_eq!(field_type.convert_value(&json!(1)).unwrap(), json!(true));
        assert_eq!(field_type.convert_value(&json!(0)).unwrap(), json!(false));
        assert_eq!(field_type.convert_value(&json!(-1)).unwrap(), json!(true));

        // Invalid string conversion
        assert!(field_type.convert_value(&json!("maybe")).is_err());
    }
}
