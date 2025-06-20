//! Relationship field type for referencing other collections
//!
//! This field type allows creating relationships between collections.
//! It supports both single (one-to-one, many-to-one) and multiple (one-to-many, many-to-many) relationships.

use super::FieldTypeDefinition;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;

/// Configuration for relationship fields
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct RelationshipConfig {
    /// The name of the target collection
    pub target_collection: String,
    /// Whether this is a multiple relationship (array of IDs) or single (single ID)
    pub multiple: bool,
    /// Whether to cascade delete (delete related records when this record is deleted)
    pub cascade_delete: bool,
    /// Field name in the target collection to display when populated (optional)
    pub display_field: Option<String>,
}

/// Relationship field type implementation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipFieldType {
    /// Configuration for this relationship
    pub config: RelationshipConfig,
}

impl RelationshipFieldType {
    /// Create a new relationship field type
    pub fn new(config: RelationshipConfig) -> Self {
        Self { config }
    }

    /// Create a single relationship field
    pub fn single(target_collection: String) -> Self {
        Self::new(RelationshipConfig {
            target_collection,
            multiple: false,
            cascade_delete: false,
            display_field: None,
        })
    }

    /// Create a multiple relationship field
    pub fn multiple(target_collection: String) -> Self {
        Self::new(RelationshipConfig {
            target_collection,
            multiple: true,
            cascade_delete: false,
            display_field: None,
        })
    }

    /// Set cascade delete behavior
    pub fn with_cascade_delete(mut self, cascade: bool) -> Self {
        self.config.cascade_delete = cascade;
        self
    }

    /// Set display field for populated relationships
    pub fn with_display_field(mut self, field: Option<String>) -> Self {
        self.config.display_field = field;
        self
    }

    /// Validate a single record ID
    fn validate_single_id(&self, value: &JsonValue) -> Result<(), String> {
        match value {
            JsonValue::String(id) => {
                if id.trim().is_empty() {
                    return Err("Record ID cannot be empty".to_string());
                }
                // Basic UUID format validation (optional - could be more strict)
                if id.len() < 8 {
                    return Err("Invalid record ID format".to_string());
                }
                Ok(())
            }
            JsonValue::Null => Ok(()), // Allow null for optional relationships
            _ => Err("Single relationship must be a string ID or null".to_string()),
        }
    }

    /// Validate multiple record IDs
    fn validate_multiple_ids(&self, value: &JsonValue) -> Result<(), String> {
        match value {
            JsonValue::Array(ids) => {
                for (index, id) in ids.iter().enumerate() {
                    if let JsonValue::String(id_str) = id {
                        if id_str.trim().is_empty() {
                            return Err(format!("Record ID at index {} cannot be empty", index));
                        }
                        if id_str.len() < 8 {
                            return Err(format!("Invalid record ID format at index {}", index));
                        }
                    } else {
                        return Err(format!("Record ID at index {} must be a string", index));
                    }
                }
                Ok(())
            }
            JsonValue::Null => Ok(()), // Allow null for optional relationships
            _ => Err("Multiple relationship must be an array of string IDs or null".to_string()),
        }
    }
}

impl FieldTypeDefinition for RelationshipFieldType {
    fn type_name(&self) -> &'static str {
        "relationship"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        if self.config.multiple {
            self.validate_multiple_ids(value).map_err(|e| {
                format!("Field '{}': {}", field_name, e)
            })
        } else {
            self.validate_single_id(value).map_err(|e| {
                format!("Field '{}': {}", field_name, e)
            })
        }
    }

    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        // For relationship fields, we generally don't need to convert the value
        // as it should already be in the correct format (string ID or array of string IDs)
        self.validate("conversion", value)?;
        Ok(value.clone())
    }

    fn sql_type(&self) -> &'static str {
        if self.config.multiple {
            "TEXT" // Store as JSON array of IDs
        } else {
            "TEXT" // Store as single ID string
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_single_relationship_validation() {
        let field = RelationshipFieldType::single("users".to_string());
        
        // Valid single ID
        assert!(field.validate("test", &json!("user-123")).is_ok());
        
        // Null is allowed for optional relationships
        assert!(field.validate("test", &JsonValue::Null).is_ok());
        
        // Invalid types
        assert!(field.validate("test", &json!(123)).is_err());
        assert!(field.validate("test", &json!(["id1", "id2"])).is_err());
        
        // Empty string
        assert!(field.validate("test", &json!("")).is_err());
        
        // Too short ID
        assert!(field.validate("test", &json!("123")).is_err());
    }

    #[test]
    fn test_multiple_relationship_validation() {
        let field = RelationshipFieldType::multiple("users".to_string());
        
        // Valid multiple IDs
        assert!(field.validate("test", &json!(["user-123", "user-456"])).is_ok());
        
        // Empty array is valid
        assert!(field.validate("test", &json!([])).is_ok());
        
        // Null is allowed for optional relationships
        assert!(field.validate("test", &JsonValue::Null).is_ok());
        
        // Invalid types
        assert!(field.validate("test", &json!("single-id")).is_err());
        assert!(field.validate("test", &json!(123)).is_err());
        
        // Array with invalid ID
        assert!(field.validate("test", &json!(["user-123", ""])).is_err());
        assert!(field.validate("test", &json!(["user-123", 456])).is_err());
    }

    #[test]
    fn test_relationship_config_creation() {
        let single_field = RelationshipFieldType::single("posts".to_string());
        assert!(!single_field.config.multiple);
        assert_eq!(single_field.config.target_collection, "posts");
        
        let multiple_field = RelationshipFieldType::multiple("tags".to_string())
            .with_cascade_delete(true)
            .with_display_field(Some("name".to_string()));
        
        assert!(multiple_field.config.multiple);
        assert!(multiple_field.config.cascade_delete);
        assert_eq!(multiple_field.config.display_field, Some("name".to_string()));
    }

    #[test]
    fn test_field_type_name() {
        let field = RelationshipFieldType::single("users".to_string());
        assert_eq!(field.type_name(), "relationship");
    }

    #[test]
    fn test_sql_type() {
        let single_field = RelationshipFieldType::single("users".to_string());
        assert_eq!(single_field.sql_type(), "TEXT");
        
        let multiple_field = RelationshipFieldType::multiple("users".to_string());
        assert_eq!(multiple_field.sql_type(), "TEXT");
    }
} 