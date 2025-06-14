//! Date field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// Date/timestamp field type
#[derive(Debug, Clone)]
pub struct DateFieldType;

impl FieldTypeDefinition for DateFieldType {
    fn type_name(&self) -> &'static str {
        "date"
    }
    
    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if !value.is_string() {
            return Err(format!("Field '{}' must be a date string", field_name));
        }
        
        // Additional date format validation could be added here
        // For now, we just ensure it's a string
        Ok(())
    }
    
    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        match value {
            JsonValue::String(s) => {
                // Try to parse as ISO 8601 datetime
                if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
                    Ok(value.clone())
                } else {
                    Err("Invalid datetime format".to_string())
                }
            }
            _ => Err("DateTime must be a string".to_string()),
        }
    }
    
    fn sql_type(&self) -> &'static str {
        "INTEGER" // Store as unix timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_date_validation() {
        let field_type = DateFieldType;
        
        // Valid date strings
        assert!(field_type.validate("test", &json!("2023-01-01T00:00:00Z")).is_ok());
        assert!(field_type.validate("test", &json!("2023-12-31")).is_ok());
        
        // Invalid types
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!(true)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }
    
    #[test]
    fn test_date_conversion() {
        let field_type = DateFieldType;
        
        // Valid ISO 8601 date
        let valid_date = "2023-01-01T00:00:00Z";
        assert_eq!(field_type.convert_value(&json!(valid_date)).unwrap(), json!(valid_date));
        
        // Invalid date format
        assert!(field_type.convert_value(&json!("not-a-date")).is_err());
        
        // Non-string input
        assert!(field_type.convert_value(&json!(123)).is_err());
    }
} 