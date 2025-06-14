//! Collection schema definitions
//!
//! This module defines the data structures for managing collection schemas
//! in OxideDB. Collections can be either 'base' (user-defined) or 'auth' 
//! (system authentication collections).

use crate::field_types::FieldType;
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

/// Index definition for database optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexDefinition {
    /// Name of the index
    pub name: String,
    /// Fields to index (can be multiple for composite indexes)
    pub fields: Vec<String>,
    /// Whether this is a unique index
    pub unique: bool,
}

/// Field definition within a collection schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDefinition {
    /// The type of this field
    pub field_type: FieldType,
    /// Whether this field is required
    pub required: bool,
    /// Whether this field must be unique
    pub unique: bool,
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
    /// Index definitions for performance optimization
    pub indexes: Vec<IndexDefinition>,
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
            indexes: Vec::new(),
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

    /// Add an index to the schema
    pub fn add_index(&mut self, index: IndexDefinition) {
        self.indexes.push(index);
        self.update_timestamp();
    }

    /// Remove an index from the schema
    pub fn remove_index(&mut self, index_name: &str) -> bool {
        let original_len = self.indexes.len();
        self.indexes.retain(|idx| idx.name != index_name);
        let removed = self.indexes.len() < original_len;
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
        // Use the new extensible field type validation
        field_def.field_type.validate(field_name, value)
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
        assert!(schema.indexes.is_empty());
    }

    #[test]
    fn test_field_validation() {
        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        schema.add_field(
            "name".to_string(),
            FieldDefinition {
                field_type: FieldType::Text,
                required: true,
                unique: false,
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

    #[test]
    fn test_password_field_type() {
        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        schema.add_field(
            "email".to_string(),
            FieldDefinition {
                field_type: FieldType::Email,
                required: true,
                unique: true,
                default: None,
                validation: None,
            },
        );
        schema.add_field(
            "password".to_string(),
            FieldDefinition {
                field_type: FieldType::Password,
                required: true,
                unique: false,
                default: None,
                validation: None,
            },
        );
        schema.add_field(
            "backup_password".to_string(),
            FieldDefinition {
                field_type: FieldType::Password,
                required: false,
                unique: false,
                default: None,
                validation: None,
            },
        );

        // Test password field validation
        let valid_data = serde_json::json!({
            "email": "test@example.com", 
            "password": "secret123",
            "backup_password": "backup456"
        });
        assert!(schema.validate_data(&valid_data).is_ok());

        // Test empty password validation fails
        let invalid_data = serde_json::json!({
            "email": "test@example.com", 
            "password": "",
            "backup_password": "backup456"
        });
        assert!(schema.validate_data(&invalid_data).is_err());

        // Test missing required password field
        let missing_password = serde_json::json!({
            "email": "test@example.com"
        });
        assert!(schema.validate_data(&missing_password).is_err());

        // Test password field type requirements
        assert!(FieldType::Password.requires_hashing());
    }
} 