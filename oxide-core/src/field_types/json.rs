//! JSON field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// JSON object field type
#[derive(Debug, Clone)]
pub struct JsonFieldType;

impl FieldTypeDefinition for JsonFieldType {
    fn type_name(&self) -> &'static str {
        "json"
    }
    
    fn validate(&self, _field_name: &str, _value: &JsonValue) -> Result<(), String> {
        // Any JSON value is valid for json type
        Ok(())
    }
    
    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        // JSON type accepts any valid JSON value
        Ok(value.clone())
    }
    
    fn sql_type(&self) -> &'static str {
        "TEXT" // Store as JSON string
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_validation() {
        let field_type = JsonFieldType;
        
        // All JSON values are valid
        assert!(field_type.validate("test", &json!("string")).is_ok());
        assert!(field_type.validate("test", &json!(123)).is_ok());
        assert!(field_type.validate("test", &json!(true)).is_ok());
        assert!(field_type.validate("test", &json!({})).is_ok());
        assert!(field_type.validate("test", &json!([])).is_ok());
        assert!(field_type.validate("test", &json!(null)).is_ok());
    }
    
    #[test]
    fn test_json_conversion() {
        let field_type = JsonFieldType;
        
        // All values pass through unchanged
        assert_eq!(field_type.convert_value(&json!("hello")).unwrap(), json!("hello"));
        assert_eq!(field_type.convert_value(&json!(123)).unwrap(), json!(123));
        assert_eq!(field_type.convert_value(&json!(true)).unwrap(), json!(true));
        assert_eq!(field_type.convert_value(&json!({})).unwrap(), json!({}));
        assert_eq!(field_type.convert_value(&json!([])).unwrap(), json!([]));
        assert_eq!(field_type.convert_value(&json!(null)).unwrap(), json!(null));
    }
} 