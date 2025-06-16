//! Data Sanitizer Hook
//!
//! This hook sanitizes input data to prevent security issues and ensure
//! data consistency across the system.

use crate::{BeforeEventContext, AppError};
use regex::Regex;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, warn};

/// Configuration for data sanitization
#[derive(Debug, Clone)]
pub struct DataSanitizerConfig {
    /// Whether to strip HTML tags from string fields
    pub strip_html: bool,
    /// Whether to trim whitespace from string fields
    pub trim_whitespace: bool,
    /// Whether to normalize unicode characters
    pub normalize_unicode: bool,
    /// Fields to exclude from sanitization
    pub excluded_fields: HashSet<String>,
    /// Collections to skip sanitization for
    pub skip_collections: Vec<String>,
    /// Maximum string length (0 means no limit)
    pub max_string_length: usize,
    /// Whether to convert to lowercase for specific fields
    pub lowercase_fields: HashSet<String>,
}

impl Default for DataSanitizerConfig {
    fn default() -> Self {
        let mut excluded_fields = HashSet::new();
        excluded_fields.insert("password".to_string()); // Contains hash after processing, should not be sanitized
        excluded_fields.insert("content".to_string()); // Preserve rich content
        excluded_fields.insert("html".to_string());

        let mut lowercase_fields = HashSet::new();
        lowercase_fields.insert("email".to_string());
        lowercase_fields.insert("username".to_string());

        Self {
            strip_html: true,
            trim_whitespace: true,
            normalize_unicode: true,
            excluded_fields,
            skip_collections: vec!["_internal".to_string()],
            max_string_length: 10000, // 10KB limit
            lowercase_fields,
        }
    }
}

/// Data sanitizer hook for cleaning input data
pub struct DataSanitizerHook {
    config: DataSanitizerConfig,
    html_regex: Arc<Regex>,
    whitespace_regex: Arc<Regex>,
}

impl DataSanitizerHook {
    /// Create a new data sanitizer hook with default configuration
    pub fn new() -> Result<Self, AppError> {
        let config = DataSanitizerConfig::default();
        Self::with_config(config)
    }

    /// Create a new data sanitizer hook with custom configuration
    pub fn with_config(config: DataSanitizerConfig) -> Result<Self, AppError> {
        // Compile regex patterns
        let html_regex = Arc::new(
            Regex::new(r"<script[^>]*>.*?</script>|<[^>]*>")
                .map_err(|e| AppError::internal(format!("Failed to compile HTML regex: {}", e)))?
        );

        let whitespace_regex = Arc::new(
            Regex::new(r"\s+")
                .map_err(|e| AppError::internal(format!("Failed to compile whitespace regex: {}", e)))?
        );

        Ok(Self {
            config,
            html_regex,
            whitespace_regex,
        })
    }

    /// Handle Before record create events for data sanitization
    pub fn handle_before_record_create(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.should_skip_sanitization(&context.collection) {
            return Ok(());
        }

        debug!("Sanitizing data for create in collection: {}", context.collection);
        self.sanitize_data(context)?;
        Ok(())
    }

    /// Handle Before record update events for data sanitization
    pub fn handle_before_record_update(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.should_skip_sanitization(&context.collection) {
            return Ok(());
        }

        debug!("Sanitizing data for update in collection: {}", context.collection);
        self.sanitize_data(context)?;
        Ok(())
    }

    /// Check if sanitization should be skipped for this collection
    fn should_skip_sanitization(&self, collection: &str) -> bool {
        self.config.skip_collections.contains(&collection.to_string())
    }

    /// Sanitize all data in the context
    fn sanitize_data(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if let Some(data_obj) = context.data.as_object_mut() {
            for (field_name, value) in data_obj.iter_mut() {
                if !self.config.excluded_fields.contains(field_name) {
                    self.sanitize_value(field_name, value)?;
                }
            }
        }
        Ok(())
    }

    /// Sanitize a single value
    fn sanitize_value(&self, field_name: &str, value: &mut serde_json::Value) -> Result<(), AppError> {
        match value {
            serde_json::Value::String(s) => {
                let mut sanitized = s.clone();

                // Trim whitespace
                if self.config.trim_whitespace {
                    sanitized = sanitized.trim().to_string();
                }

                // Strip HTML tags
                if self.config.strip_html {
                    sanitized = self.html_regex.replace_all(&sanitized, "").to_string();
                }

                // Normalize unicode
                if self.config.normalize_unicode {
                    sanitized = self.normalize_unicode_string(&sanitized);
                }

                // Convert to lowercase for specific fields
                if self.config.lowercase_fields.contains(field_name) {
                    sanitized = sanitized.to_lowercase();
                }

                // Check string length
                if self.config.max_string_length > 0 && sanitized.len() > self.config.max_string_length {
                    warn!("String too long in field '{}': {} characters", field_name, sanitized.len());
                    return Err(AppError::validation(
                        field_name,
                        &format!("String too long: {} characters (max: {})", 
                                sanitized.len(), self.config.max_string_length)
                    ));
                }

                // Apply additional sanitization
                sanitized = self.sanitize_dangerous_content(&sanitized)?;

                *s = sanitized;
            }
            serde_json::Value::Object(obj) => {
                // Recursively sanitize nested objects
                for (nested_field, nested_value) in obj.iter_mut() {
                    self.sanitize_value(nested_field, nested_value)?;
                }
            }
            serde_json::Value::Array(arr) => {
                // Sanitize array elements
                for (index, item) in arr.iter_mut().enumerate() {
                    self.sanitize_value(&format!("{}[{}]", field_name, index), item)?;
                }
            }
            _ => {
                // Numbers, booleans, and null don't need sanitization
            }
        }
        Ok(())
    }

    /// Normalize unicode characters
    fn normalize_unicode_string(&self, s: &str) -> String {
        use unicode_normalization::UnicodeNormalization;
        s.nfc().collect()
    }

    /// Sanitize potentially dangerous content
    fn sanitize_dangerous_content(&self, s: &str) -> Result<String, AppError> {
        let mut sanitized = s.to_string();

        // Remove null bytes
        sanitized = sanitized.replace('\0', "");

        // Remove control characters (except common whitespace)
        sanitized = sanitized
            .chars()
            .filter(|&c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
            .collect();

        // Normalize excessive whitespace
        sanitized = self.whitespace_regex.replace_all(&sanitized, " ").to_string();

        // Check for suspicious patterns
        if self.contains_suspicious_patterns(&sanitized) {
            warn!("Suspicious content detected and sanitized");
            // For now, just log - in a real system you might want to reject or further sanitize
        }

        Ok(sanitized)
    }

    /// Check for suspicious patterns that might indicate attacks
    fn contains_suspicious_patterns(&self, s: &str) -> bool {
        let suspicious_patterns = [
            "javascript:",
            "data:",
            "vbscript:",
            "onload=",
            "onerror=",
            "onclick=",
            "<script",
            "</script>",
            "eval(",
            "expression(",
        ];

        let s_lower = s.to_lowercase();
        suspicious_patterns.iter().any(|pattern| s_lower.contains(pattern))
    }

    /// Add a field to the exclusion list
    pub fn exclude_field(&mut self, field_name: String) {
        self.config.excluded_fields.insert(field_name);
    }

    /// Remove a field from the exclusion list
    pub fn include_field(&mut self, field_name: &str) {
        self.config.excluded_fields.remove(field_name);
    }

    /// Add a field to the lowercase conversion list
    pub fn add_lowercase_field(&mut self, field_name: String) {
        self.config.lowercase_fields.insert(field_name);
    }

    /// Get sanitization statistics
    pub fn get_stats(&self) -> DataSanitizerStats {
        DataSanitizerStats {
            excluded_fields_count: self.config.excluded_fields.len(),
            lowercase_fields_count: self.config.lowercase_fields.len(),
            skip_collections_count: self.config.skip_collections.len(),
            max_string_length: self.config.max_string_length,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &DataSanitizerConfig {
        &self.config
    }
}

/// Statistics about the data sanitizer
#[derive(Debug, Clone)]
pub struct DataSanitizerStats {
    pub excluded_fields_count: usize,
    pub lowercase_fields_count: usize,
    pub skip_collections_count: usize,
    pub max_string_length: usize,
}

impl Default for DataSanitizerHook {
    fn default() -> Self {
        Self::new().expect("Failed to create default DataSanitizerHook")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_html_stripping() {
        let hook = DataSanitizerHook::new().unwrap();

        let mut context = BeforeEventContext {
            collection: "posts".to_string(),
            data: json!({
                "title": "<script>alert('xss')</script>Hello World",
                "content": "This should <b>not</b> be stripped"
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Title should be stripped (not excluded)
        assert_eq!(context.data["title"], json!("Hello World"));
        
        // Content should be preserved (excluded by default)
        assert_eq!(context.data["content"], json!("This should <b>not</b> be stripped"));
    }

    #[test]
    fn test_whitespace_trimming() {
        let hook = DataSanitizerHook::new().unwrap();

        let mut context = BeforeEventContext {
            collection: "users".to_string(),
            data: json!({
                "name": "  John Doe  ",
                "email": "  JOHN@EXAMPLE.COM  "
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Name should be trimmed
        assert_eq!(context.data["name"], json!("John Doe"));
        
        // Email should be trimmed and lowercased
        assert_eq!(context.data["email"], json!("john@example.com"));
    }

    #[test]
    fn test_string_length_validation() {
        let mut config = DataSanitizerConfig::default();
        config.max_string_length = 10;
        let hook = DataSanitizerHook::with_config(config).unwrap();

        let mut context = BeforeEventContext {
            collection: "posts".to_string(),
            data: json!({
                "title": "This is a very long title that exceeds the limit"
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        // Should fail due to length limit
        assert!(hook.handle_before_record_create(&mut context).is_err());
    }

    #[test]
    fn test_nested_object_sanitization() {
        let hook = DataSanitizerHook::new().unwrap();

        let mut context = BeforeEventContext {
            collection: "complex".to_string(),
            data: json!({
                "user": {
                    "name": "  John  ",
                    "profile": {
                        "bio": "<script>alert('xss')</script>Developer"
                    }
                }
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Nested fields should be sanitized
        assert_eq!(context.data["user"]["name"], json!("John"));
        assert_eq!(context.data["user"]["profile"]["bio"], json!("Developer"));
    }

    #[test]
    fn test_suspicious_content_detection() {
        let hook = DataSanitizerHook::new().unwrap();
        
        assert!(hook.contains_suspicious_patterns("javascript:alert('xss')"));
        assert!(hook.contains_suspicious_patterns("<script>alert(1)</script>"));
        assert!(!hook.contains_suspicious_patterns("normal content"));
    }
}