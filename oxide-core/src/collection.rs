//! Collection schema definitions
//!
//! This module defines the data structures for managing collection schemas
//! in OxideDB. Collections can be either 'base' (user-defined) or 'auth' 
//! (system authentication collections).

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;

/// The type of a collection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CollectionType {
    /// Base collections are user-defined collections for storing application data
    Base,
    /// Auth collections are system collections for authentication and user management  
    Auth,
}

impl std::fmt::Display for CollectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CollectionType::Base => write!(f, "base"),
            CollectionType::Auth => write!(f, "auth"),
        }
    }
}

/// Supported field types in collection schemas
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    /// Text/string field
    Text,
    /// Numeric field (integer or float)
    Number,
    /// Boolean field
    Boolean,
    /// Date/timestamp field
    Date,
    /// JSON object field
    Json,
    /// Email field (text with email validation)
    Email,
    /// URL field (text with URL validation)
    Url,
}

impl std::fmt::Display for FieldType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::Text => write!(f, "text"),
            FieldType::Number => write!(f, "number"),
            FieldType::Boolean => write!(f, "boolean"),
            FieldType::Date => write!(f, "date"),
            FieldType::Json => write!(f, "json"),
            FieldType::Email => write!(f, "email"),
            FieldType::Url => write!(f, "url"),
        }
    }
}

/// Field definition within a collection schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// The type of this field
    pub field_type: FieldType,
    /// Whether this field is required
    pub required: bool,
    /// Default value for this field (optional)
    pub default: Option<JsonValue>,
    /// Validation rules (optional)
    pub validation: Option<JsonValue>,
}

/// Complete schema definition for a collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionSchema {
    /// Collection identifier
    pub id: String,
    /// Collection name (must be unique)
    pub name: String,
    /// Type of collection
    pub collection_type: CollectionType,
    /// Field definitions for this collection
    pub fields: HashMap<String, FieldDefinition>,
    /// Timestamp when collection was created
    pub created_at: i64,
    /// Timestamp when collection was last updated
    pub updated_at: i64,
}

impl CollectionSchema {
    /// Create a new collection schema
    pub fn new(name: String, collection_type: CollectionType) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            collection_type,
            fields: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a field to the schema
    pub fn add_field(&mut self, name: String, definition: FieldDefinition) {
        self.fields.insert(name, definition);
        self.update_timestamp();
    }

    /// Remove a field from the schema
    pub fn remove_field(&mut self, name: &str) -> bool {
        let removed = self.fields.remove(name).is_some();
        if removed {
            self.update_timestamp();
        }
        removed
    }

    /// Update the updated_at timestamp
    fn update_timestamp(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
    }

    /// Validate data against this schema
    pub fn validate_data(&self, data: &JsonValue) -> Result<(), String> {
        let data_obj = match data.as_object() {
            Some(obj) => obj,
            None => return Err("Data must be a JSON object".to_string()),
        };

        // Check required fields
        for (field_name, field_def) in &self.fields {
            if field_def.required && !data_obj.contains_key(field_name) {
                return Err(format!("Required field '{}' is missing", field_name));
            }
        }

        // Validate field types
        for (field_name, value) in data_obj {
            if let Some(field_def) = self.fields.get(field_name) {
                self.validate_field_value(field_name, value, field_def)?;
            }
            // Note: We allow extra fields not defined in schema (for flexibility)
        }

        Ok(())
    }

    /// Validate a single field value
    fn validate_field_value(
        &self,
        field_name: &str,
        value: &JsonValue,
        field_def: &FieldDefinition,
    ) -> Result<(), String> {
        match field_def.field_type {
            FieldType::Text => {
                if !value.is_string() {
                    return Err(format!("Field '{}' must be a string", field_name));
                }
            }
            FieldType::Number => {
                if !value.is_number() {
                    return Err(format!("Field '{}' must be a number", field_name));
                }
            }
            FieldType::Boolean => {
                if !value.is_boolean() {
                    return Err(format!("Field '{}' must be a boolean", field_name));
                }
            }
            FieldType::Date => {
                if !value.is_string() {
                    return Err(format!("Field '{}' must be a date string", field_name));
                }
                // Additional date format validation could be added here
            }
            FieldType::Json => {
                // Any JSON value is valid for json type
            }
            FieldType::Email => {
                if let Some(email) = value.as_str() {
                    if !email.contains('@') {
                        return Err(format!("Field '{}' must be a valid email address", field_name));
                    }
                } else {
                    return Err(format!("Field '{}' must be a string", field_name));
                }
            }
            FieldType::Url => {
                if let Some(url) = value.as_str() {
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        return Err(format!("Field '{}' must be a valid URL", field_name));
                    }
                } else {
                    return Err(format!("Field '{}' must be a string", field_name));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_schema_creation() {
        let schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        assert_eq!(schema.name, "users");
        assert_eq!(schema.collection_type, CollectionType::Base);
        assert!(schema.fields.is_empty());
    }

    #[test]
    fn test_field_validation() {
        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        schema.add_field(
            "name".to_string(),
            FieldDefinition {
                field_type: FieldType::Text,
                required: true,
                default: None,
                validation: None,
            },
        );

        // Valid data
        let valid_data = serde_json::json!({"name": "John Doe"});
        assert!(schema.validate_data(&valid_data).is_ok());

        // Missing required field
        let invalid_data = serde_json::json!({});
        assert!(schema.validate_data(&invalid_data).is_err());

        // Wrong type
        let invalid_data = serde_json::json!({"name": 123});
        assert!(schema.validate_data(&invalid_data).is_err());
    }
} 