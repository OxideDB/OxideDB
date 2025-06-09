//! Activity Logger Hook
//!
//! This hook logs all database operations for auditing and debugging purposes.
//! It provides comprehensive logging of Before and After events with configurable
//! log levels and filtering.

use crate::{BeforeEventContext, AfterEventContext, AppError};
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use tracing::{info, debug, warn, error};

/// Configuration for activity logging
#[derive(Debug, Clone)]
pub struct ActivityLoggerConfig {
    /// Collections to log (empty means log all)
    pub collections_to_log: Vec<String>,
    /// Collections to exclude from logging
    pub collections_to_exclude: Vec<String>,
    /// Whether to log data content (can be disabled for privacy)
    pub log_data_content: bool,
    /// Whether to log sensitive fields (passwords, tokens, etc.)
    pub log_sensitive_fields: bool,
    /// Fields considered sensitive
    pub sensitive_fields: HashSet<String>,
    /// Whether to log metadata
    pub log_metadata: bool,
    /// Maximum data length to log (truncate if longer)
    pub max_data_length: usize,
}

impl Default for ActivityLoggerConfig {
    fn default() -> Self {
        let mut sensitive_fields = HashSet::new();
        sensitive_fields.insert("password".to_string());
        sensitive_fields.insert("passwordHash".to_string());
        sensitive_fields.insert("token".to_string());
        sensitive_fields.insert("apiKey".to_string());
        sensitive_fields.insert("secret".to_string());

        Self {
            collections_to_log: vec![],
            collections_to_exclude: vec![],
            log_data_content: true,
            log_sensitive_fields: false,
            sensitive_fields,
            log_metadata: true,
            max_data_length: 1000,
        }
    }
}

/// Activity logger hook for comprehensive operation logging
pub struct ActivityLoggerHook {
    config: ActivityLoggerConfig,
}

impl ActivityLoggerHook {
    /// Create a new activity logger hook with default configuration
    pub fn new() -> Self {
        Self {
            config: ActivityLoggerConfig::default(),
        }
    }

    /// Create a new activity logger hook with custom configuration
    pub fn with_config(config: ActivityLoggerConfig) -> Self {
        Self { config }
    }

    /// Handle Before events (all types)
    pub fn handle_before_event(&self, event_type: &str, context: &BeforeEventContext) -> Result<(), AppError> {
        if !self.should_log_collection(&context.collection) {
            return Ok(());
        }

        let sanitized_data = if self.config.log_data_content {
            Some(self.sanitize_data(&context.data))
        } else {
            None
        };

        let metadata = if self.config.log_metadata {
            Some(&context.metadata)
        } else {
            None
        };

        info!(
            "🎣 [BEFORE] {} in collection '{}' | Record ID: {:?} | Data: {} | Metadata: {}",
            event_type,
            context.collection,
            context.record_id,
            sanitized_data.map_or_else(|| "[HIDDEN]".to_string(), |d| self.truncate_json(&d)),
            metadata.map_or_else(|| "[HIDDEN]".to_string(), |m| self.truncate_json(m))
        );

        Ok(())
    }

    /// Handle After events (all types)
    pub fn handle_after_event(&self, event_type: &str, context: &AfterEventContext) -> Result<(), AppError> {
        match context {
            AfterEventContext::RecordCreated { collection, record_id, data } => {
                if self.should_log_collection(collection) {
                    let sanitized_data = if self.config.log_data_content {
                        Some(self.sanitize_data(data))
                    } else {
                        None
                    };

                    info!(
                        "✅ [AFTER] {} | Collection: '{}' | Record ID: {} | Data: {}",
                        event_type,
                        collection,
                        record_id,
                        sanitized_data.map_or_else(|| "[HIDDEN]".to_string(), |d| self.truncate_json(&d))
                    );
                }
            }
            AfterEventContext::RecordUpdated { collection, record_id, old_data, new_data } => {
                if self.should_log_collection(collection) {
                    let old_sanitized = if self.config.log_data_content {
                        Some(self.sanitize_data(old_data))
                    } else {
                        None
                    };
                    let new_sanitized = if self.config.log_data_content {
                        Some(self.sanitize_data(new_data))
                    } else {
                        None
                    };

                    info!(
                        "🔄 [AFTER] {} | Collection: '{}' | Record ID: {} | Old: {} | New: {}",
                        event_type,
                        collection,
                        record_id,
                        old_sanitized.map_or_else(|| "[HIDDEN]".to_string(), |d| self.truncate_json(&d)),
                        new_sanitized.map_or_else(|| "[HIDDEN]".to_string(), |d| self.truncate_json(&d))
                    );
                }
            }
            AfterEventContext::RecordDeleted { collection, record_id, data } => {
                if self.should_log_collection(collection) {
                    info!(
                        "🗑️ [AFTER] {} | Collection: '{}' | Record ID: {} | Deleted data logged separately",
                        event_type,
                        collection,
                        record_id
                    );
                }
            }
            AfterEventContext::UserRegistered { user_id, email, .. } => {
                info!(
                    "👤 [AFTER] {} | User ID: {} | Email: {}",
                    event_type,
                    user_id,
                    email
                );
            }
            AfterEventContext::UserAuthenticated { user_id, email } => {
                info!(
                    "🔐 [AFTER] {} | User ID: {} | Email: {}",
                    event_type,
                    user_id,
                    email
                );
            }
            AfterEventContext::CollectionCreated { collection } => {
                info!(
                    "📁 [AFTER] {} | Collection: '{}'",
                    event_type,
                    collection
                );
            }
            AfterEventContext::CollectionDeleted { collection } => {
                info!(
                    "🗂️ [AFTER] {} | Collection: '{}'",
                    event_type,
                    collection
                );
            }
            AfterEventContext::ErrorOccurred { error_type, message, context: error_context } => {
                error!(
                    "❌ [AFTER] {} | Error Type: {} | Message: {} | Context: {}",
                    event_type,
                    error_type,
                    message,
                    self.truncate_json(error_context)
                );
            }
            _ => {
                debug!(
                    "📋 [AFTER] {} | Generic event logged",
                    event_type
                );
            }
        }

        Ok(())
    }

    /// Check if we should log operations for this collection
    fn should_log_collection(&self, collection: &str) -> bool {
        // If exclusion list is not empty and collection is in it, don't log
        if !self.config.collections_to_exclude.is_empty() 
            && self.config.collections_to_exclude.contains(&collection.to_string()) {
            return false;
        }

        // If inclusion list is not empty, only log if collection is in it
        if !self.config.collections_to_log.is_empty() {
            return self.config.collections_to_log.contains(&collection.to_string());
        }

        // If both lists are empty, log everything
        true
    }

    /// Sanitize data by removing or masking sensitive fields
    fn sanitize_data(&self, data: &JsonValue) -> JsonValue {
        if !self.config.log_sensitive_fields {
            self.mask_sensitive_fields(data.clone())
        } else {
            data.clone()
        }
    }

    /// Mask sensitive fields in JSON data
    fn mask_sensitive_fields(&self, mut data: JsonValue) -> JsonValue {
        if let Some(obj) = data.as_object_mut() {
            for (key, value) in obj.iter_mut() {
                if self.config.sensitive_fields.contains(key) {
                    *value = JsonValue::String("[MASKED]".to_string());
                } else if value.is_object() {
                    *value = self.mask_sensitive_fields(value.clone());
                }
            }
        }
        data
    }

    /// Truncate JSON string representation if too long
    fn truncate_json(&self, data: &JsonValue) -> String {
        let json_str = serde_json::to_string(data).unwrap_or_else(|_| "[INVALID JSON]".to_string());
        
        if json_str.len() > self.config.max_data_length {
            format!("{}... [TRUNCATED]", &json_str[..self.config.max_data_length])
        } else {
            json_str
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &ActivityLoggerConfig {
        &self.config
    }

    /// Update configuration
    pub fn set_config(&mut self, config: ActivityLoggerConfig) {
        self.config = config;
    }

    /// Add a collection to the logging inclusion list
    pub fn add_collection_to_log(&mut self, collection: String) {
        if !self.config.collections_to_log.contains(&collection) {
            self.config.collections_to_log.push(collection);
        }
    }

    /// Add a collection to the logging exclusion list
    pub fn add_collection_to_exclude(&mut self, collection: String) {
        if !self.config.collections_to_exclude.contains(&collection) {
            self.config.collections_to_exclude.push(collection);
        }
    }
}

impl Default for ActivityLoggerHook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_activity_logger_config() {
        let config = ActivityLoggerConfig::default();
        let hook = ActivityLoggerHook::with_config(config);

        assert!(hook.config.log_data_content);
        assert!(!hook.config.log_sensitive_fields);
        assert!(hook.config.sensitive_fields.contains("password"));
    }

    #[test]
    fn test_collection_filtering() {
        let mut config = ActivityLoggerConfig::default();
        config.collections_to_log = vec!["users".to_string()];
        let hook = ActivityLoggerHook::with_config(config);

        assert!(hook.should_log_collection("users"));
        assert!(!hook.should_log_collection("posts"));
    }

    #[test]
    fn test_sensitive_field_masking() {
        let hook = ActivityLoggerHook::new();
        let data = json!({
            "email": "test@example.com",
            "password": "secret123",
            "name": "John Doe"
        });

        let sanitized = hook.sanitize_data(&data);
        
        assert_eq!(sanitized["email"], json!("test@example.com"));
        assert_eq!(sanitized["password"], json!("[MASKED]"));
        assert_eq!(sanitized["name"], json!("John Doe"));
    }

    #[test]
    fn test_data_truncation() {
        let mut config = ActivityLoggerConfig::default();
        config.max_data_length = 10;
        let hook = ActivityLoggerHook::with_config(config);

        let large_data = json!({"key": "very_long_value_that_exceeds_limit"});
        let truncated = hook.truncate_json(&large_data);
        
        assert!(truncated.contains("[TRUNCATED]"));
        assert!(truncated.len() > 10); // Should include the truncation marker
    }
} 