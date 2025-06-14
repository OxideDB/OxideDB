//! Text field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Text/string field type
#[derive(Debug, Clone)]
pub struct TextFieldType;

impl FieldTypeDefinition for TextFieldType {
    fn type_name(&self) -> &'static str {
        "text"
    }
    
    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if !value.is_string() {
            return Err(format!("Field '{}' must be a string", field_name));
        }
        Ok(())
    }
    
    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        match value {
            JsonValue::String(_) => Ok(value.clone()),
            JsonValue::Number(n) => Ok(JsonValue::String(n.to_string())),
            JsonValue::Bool(b) => Ok(JsonValue::String(b.to_string())),
            _ => Err("Cannot convert to text".to_string()),
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
    fn test_text_validation() {
        let field_type = TextFieldType;
        
        // Valid text
        assert!(field_type.validate("test", &json!("hello")).is_ok());
        
        // Invalid types
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!(true)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }
    
    #[test]
    fn test_text_conversion() {
        let field_type = TextFieldType;
        
        // String stays string
        assert_eq!(field_type.convert_value(&json!("hello")).unwrap(), json!("hello"));
        
        // Number to string
        assert_eq!(field_type.convert_value(&json!(123)).unwrap(), json!("123"));
        
        // Bool to string
        assert_eq!(field_type.convert_value(&json!(true)).unwrap(), json!("true"));
    }
} 