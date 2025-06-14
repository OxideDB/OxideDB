//! URL field type definition

use super::FieldTypeDefinition;
use serde_json::Value as JsonValue;

/// URL field type (text with URL validation)
#[derive(Debug, Clone)]
pub struct UrlFieldType;

impl FieldTypeDefinition for UrlFieldType {
    fn type_name(&self) -> &'static str {
        "url"
    }
    
    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if let Some(url) = value.as_str() {
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(format!("Field '{}' must be a valid URL", field_name));
            }
        } else {
            return Err(format!("Field '{}' must be a string", field_name));
        }
        Ok(())
    }
    
    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        match value {
            JsonValue::String(s) => {
                // Basic URL validation
                if s.starts_with("http://") || s.starts_with("https://") {
                    Ok(value.clone())
                } else {
                    Err("Invalid URL format".to_string())
                }
            }
            _ => Err("URL must be a string".to_string()),
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
    fn test_url_validation() {
        let field_type = UrlFieldType;
        
        // Valid URLs
        assert!(field_type.validate("test", &json!("https://example.com")).is_ok());
        assert!(field_type.validate("test", &json!("http://test.org")).is_ok());
        assert!(field_type.validate("test", &json!("https://subdomain.example.com/path")).is_ok());
        
        // Invalid URLs
        assert!(field_type.validate("test", &json!("not-a-url")).is_err());
        assert!(field_type.validate("test", &json!("ftp://example.com")).is_err());
        assert!(field_type.validate("test", &json!("example.com")).is_err());
        
        // Invalid types
        assert!(field_type.validate("test", &json!(123)).is_err());
        assert!(field_type.validate("test", &json!(true)).is_err());
        assert!(field_type.validate("test", &json!({})).is_err());
    }
    
    #[test]
    fn test_url_conversion() {
        let field_type = UrlFieldType;
        
        // Valid URLs
        assert_eq!(field_type.convert_value(&json!("https://example.com")).unwrap(), json!("https://example.com"));
        assert_eq!(field_type.convert_value(&json!("http://test.org")).unwrap(), json!("http://test.org"));
        
        // Invalid URL format
        assert!(field_type.convert_value(&json!("not-a-url")).is_err());
        assert!(field_type.convert_value(&json!("ftp://example.com")).is_err());
        
        // Non-string input
        assert!(field_type.convert_value(&json!(123)).is_err());
    }
} 