//! Collection schema definitions
//!
//! This module defines the data structures for managing collection schemas
//! in OxideDB. Collections can be 'base' (user-defined lists), 'single'
//! (one-record content entries), or 'auth' (system authentication collections).

use crate::field_types::{FieldType, ValidationRules};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use ts_rs::TS;

/// The type of a collection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
#[serde(rename_all = "lowercase")]
pub enum CollectionType {
    /// Base collections are user-defined collections for storing application data
    Base,
    /// Single collections store exactly one record for static pages or site-wide content
    Single,
    /// Auth collections are system collections for authentication and user management  
    Auth,
}

impl std::fmt::Display for CollectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CollectionType::Base => write!(f, "base"),
            CollectionType::Single => write!(f, "single"),
            CollectionType::Auth => write!(f, "auth"),
        }
    }
}

/// Default schema version for new collections and for deserializing schemas
/// that were persisted before versioning was introduced.
fn default_version() -> u32 {
    1
}

fn is_valid_identifier(identifier: &str) -> bool {
    let mut chars = identifier.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphabetic() => {}
        _ => return false,
    }

    chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn validate_identifier(kind: &str, identifier: &str) -> Result<(), String> {
    if identifier.is_empty() {
        return Err(format!("{} identifier cannot be empty", kind));
    }

    if !is_valid_identifier(identifier) {
        return Err(format!(
            "{} identifier '{}' must start with a letter or underscore and contain only letters, numbers, and underscores",
            kind, identifier
        ));
    }

    Ok(())
}

/// Index definition for database optimization
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct IndexDefinition {
    /// Name of the index
    pub name: String,
    /// Fields to index (can be multiple for composite indexes)
    pub fields: Vec<String>,
    /// Whether this is a unique index
    pub unique: bool,
}

/// Field definition within a collection schema
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FieldDefinition {
    /// The type of this field
    pub field_type: FieldType,
    /// Whether this field is required
    pub required: bool,
    /// Whether this field must be unique
    pub unique: bool,
    /// Whether this field should be indexed for performance
    #[serde(default)]
    pub index: bool,
    /// Default value for this field (optional)
    #[ts(type = "any")]
    pub default: Option<JsonValue>,
    /// Validation rules for this field (regex, min/max, etc.)
    pub validation: Option<ValidationRules>,
}

impl FieldDefinition {
    /// Create a new field definition with basic settings
    pub fn new(field_type: FieldType) -> Self {
        Self {
            field_type,
            required: false,
            unique: false,
            index: false,
            default: None,
            validation: None,
        }
    }

    /// Set this field as required
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Set this field as unique
    pub fn unique(mut self) -> Self {
        self.unique = true;
        self
    }

    /// Set this field to be indexed
    pub fn indexed(mut self) -> Self {
        self.index = true;
        self
    }

    /// Set a default value for this field
    pub fn with_default(mut self, default: JsonValue) -> Self {
        self.default = Some(default);
        self
    }

    /// Set validation rules for this field
    pub fn with_validation(mut self, validation: ValidationRules) -> Self {
        self.validation = Some(validation);
        self
    }

    /// Validate a value against this field definition
    pub fn validate_value(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        // Skip validation if value is null and field is not required
        if value.is_null() && !self.required {
            return Ok(());
        }

        // Check if required field is missing/null
        if self.required && value.is_null() {
            return Err(format!(
                "Required field '{}' is missing or null",
                field_name
            ));
        }

        // Validate against field type and validation rules
        if let Some(validation_rules) = &self.validation {
            self.field_type
                .definition()
                .validate_with_rules(field_name, value, validation_rules)
        } else {
            self.field_type.validate(field_name, value)
        }
    }
}

/// Complete schema definition for a collection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CollectionSchema {
    /// Collection identifier
    pub id: String,
    /// Collection name (must be unique)
    pub name: String,
    /// Type of collection
    pub collection_type: CollectionType,
    /// Schema version (increment on breaking changes)
    ///
    /// Versioning allows the database layer to perform automated migrations
    /// when a collection definition evolves in an incompatible way. The
    /// default version is `1`. The value **MUST** be incremented whenever a
    /// change would require a table rebuild (e.g. field removal or type
    /// change). Adding new optional fields does **not** require a version
    /// bump.
    #[serde(default = "default_version")]
    pub version: u32,
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
            version: 1,
            fields: HashMap::new(),
            indexes: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Increment the schema version and update the `updated_at` timestamp.
    ///
    /// This helper should be called by tooling that performs breaking schema
    /// changes.  The database layer relies on the version number to decide
    /// whether a lightweight in-place migration is sufficient or a full table
    /// rebuild is required.
    pub fn bump_version(&mut self) {
        self.version += 1;
        self.update_timestamp();
    }

    /// Add a field to the schema
    pub fn add_field(&mut self, name: String, definition: FieldDefinition) {
        self.fields.insert(name, definition);
        self.update_timestamp();
    }

    /// Validate collection, field, and index identifiers before they are used
    /// by storage adapters.
    pub fn validate_identifiers(&self) -> Result<(), String> {
        validate_identifier("Collection", &self.name)?;

        for field_name in self.fields.keys() {
            validate_identifier("Field", field_name)?;

            if matches!(field_name.as_str(), "id" | "created_at" | "updated_at") {
                return Err(format!(
                    "Field name '{}' is reserved for record metadata",
                    field_name
                ));
            }
        }

        for index in &self.indexes {
            validate_identifier("Index", &index.name)?;

            if index.fields.is_empty() {
                return Err(format!(
                    "Index '{}' must include at least one field",
                    index.name
                ));
            }

            for field_name in &index.fields {
                validate_identifier("Index field", field_name)?;
                if !self.fields.contains_key(field_name) {
                    return Err(format!(
                        "Index '{}' references unknown field '{}'",
                        index.name, field_name
                    ));
                }
            }
        }

        Ok(())
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

        // Validate field types and validation rules
        for (field_name, value) in data_obj {
            if let Some(field_def) = self.fields.get(field_name) {
                field_def.validate_value(field_name, value)?;
            } else {
                return Err(format!(
                    "Unknown field '{}' is not defined in schema '{}'",
                    field_name, self.name
                ));
            }
        }

        Ok(())
    }

    /// Apply default values to data where fields are missing
    pub fn apply_defaults(&self, data: &mut JsonValue) -> Result<(), String> {
        let data_obj = match data.as_object_mut() {
            Some(obj) => obj,
            None => return Err("Data must be a JSON object".to_string()),
        };

        for (field_name, field_def) in &self.fields {
            // Apply default value if field is missing and has a default
            if !data_obj.contains_key(field_name) {
                if let Some(default_value) = &field_def.default {
                    data_obj.insert(field_name.clone(), default_value.clone());
                }
            }
        }

        Ok(())
    }

    /// Get all fields that should be indexed
    pub fn get_indexed_fields(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter_map(|(name, def)| if def.index { Some(name.clone()) } else { None })
            .collect()
    }

    /// Get all unique fields (for constraint creation)
    pub fn get_unique_fields(&self) -> Vec<String> {
        self.fields
            .iter()
            .filter_map(|(name, def)| if def.unique { Some(name.clone()) } else { None })
            .collect()
    }

    /// Check if a field should be hashed
    pub fn field_requires_hashing(&self, field_name: &str) -> bool {
        self.fields
            .get(field_name)
            .map(|def| def.field_type.requires_hashing())
            .unwrap_or(false)
    }

    /// Validate data and apply defaults in one step
    pub fn prepare_data(&self, data: &mut JsonValue) -> Result<(), String> {
        // First apply defaults
        self.apply_defaults(data)?;
        // Then validate the complete data
        self.validate_data(data)
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
        assert_eq!(schema.version, 1);
        assert!(schema.fields.is_empty());
        assert!(schema.indexes.is_empty());
    }

    #[test]
    fn test_single_collection_type_display() {
        assert_eq!(CollectionType::Single.to_string(), "single");
    }

    #[test]
    fn test_field_validation() {
        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        schema.add_field(
            "name".to_string(),
            FieldDefinition::new(FieldType::Text).required(),
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
    fn test_unknown_fields_are_rejected() {
        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        schema.add_field("name".to_string(), FieldDefinition::new(FieldType::Text));

        let invalid_data = serde_json::json!({
            "name": "John Doe",
            "unexpected": "this would not be persisted"
        });

        assert!(schema.validate_data(&invalid_data).is_err());
    }

    #[test]
    fn test_identifier_validation() {
        let mut schema = CollectionSchema::new("_users".to_string(), CollectionType::Auth);
        schema.add_field(
            "email_address".to_string(),
            FieldDefinition::new(FieldType::Email),
        );
        schema.add_index(IndexDefinition {
            name: "idx_users_email".to_string(),
            fields: vec!["email_address".to_string()],
            unique: true,
        });

        assert!(schema.validate_identifiers().is_ok());

        let mut invalid_schema =
            CollectionSchema::new("users; DROP TABLE users".to_string(), CollectionType::Base);
        invalid_schema.add_field("name".to_string(), FieldDefinition::new(FieldType::Text));
        assert!(invalid_schema.validate_identifiers().is_err());

        let mut reserved_field =
            CollectionSchema::new("profiles".to_string(), CollectionType::Base);
        reserved_field.add_field("id".to_string(), FieldDefinition::new(FieldType::Text));
        assert!(reserved_field.validate_identifiers().is_err());
    }

    #[test]
    fn test_password_field_type() {
        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        schema.add_field(
            "email".to_string(),
            FieldDefinition::new(FieldType::Email).required().unique(),
        );
        schema.add_field(
            "password".to_string(),
            FieldDefinition::new(FieldType::Password).required(),
        );
        schema.add_field(
            "backup_password".to_string(),
            FieldDefinition::new(FieldType::Password),
        );

        // Check password hashing requirements
        assert!(schema.field_requires_hashing("password"));
        assert!(schema.field_requires_hashing("backup_password"));
        assert!(!schema.field_requires_hashing("email"));
    }

    #[test]
    fn test_field_definition_builder() {
        // Test fluent builder pattern
        let field = FieldDefinition::new(FieldType::Text)
            .required()
            .unique()
            .indexed()
            .with_default(serde_json::json!("default_value"));

        assert!(field.required);
        assert!(field.unique);
        assert!(field.index);
        assert_eq!(field.default, Some(serde_json::json!("default_value")));
    }

    #[test]
    fn test_validation_rules() {
        use crate::field_types::ValidationRules;

        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);

        // Add field with regex validation
        let email_validation = ValidationRules::new()
            .with_regex(r"^[^@]+@[^@]+\.[^@]+$".to_string())
            .with_message("Please enter a valid email address".to_string());

        schema.add_field(
            "email".to_string(),
            FieldDefinition::new(FieldType::Text)
                .required()
                .with_validation(email_validation),
        );

        // Add field with length constraints
        let username_validation = ValidationRules::new()
            .with_min(3.0)
            .with_max(20.0)
            .with_regex(r"^[a-zA-Z0-9_]+$".to_string());

        schema.add_field(
            "username".to_string(),
            FieldDefinition::new(FieldType::Text)
                .required()
                .unique()
                .with_validation(username_validation),
        );

        // Valid data
        let valid_data = serde_json::json!({
            "email": "user@example.com",
            "username": "john_doe123"
        });
        assert!(schema.validate_data(&valid_data).is_ok());

        // Invalid email format
        let invalid_email = serde_json::json!({
            "email": "invalid-email",
            "username": "john_doe123"
        });
        assert!(schema.validate_data(&invalid_email).is_err());

        // Username too short
        let short_username = serde_json::json!({
            "email": "user@example.com",
            "username": "jo"
        });
        assert!(schema.validate_data(&short_username).is_err());

        // Username with invalid characters
        let invalid_username = serde_json::json!({
            "email": "user@example.com",
            "username": "john-doe!"
        });
        assert!(schema.validate_data(&invalid_username).is_err());
    }

    #[test]
    fn test_default_values() {
        let mut schema = CollectionSchema::new("posts".to_string(), CollectionType::Base);

        schema.add_field(
            "title".to_string(),
            FieldDefinition::new(FieldType::Text).required(),
        );

        schema.add_field(
            "status".to_string(),
            FieldDefinition::new(FieldType::Text).with_default(serde_json::json!("draft")),
        );

        schema.add_field(
            "views".to_string(),
            FieldDefinition::new(FieldType::Number).with_default(serde_json::json!(0)),
        );

        // Test applying defaults
        let mut data = serde_json::json!({"title": "My Post"});
        assert!(schema.apply_defaults(&mut data).is_ok());

        let expected = serde_json::json!({
            "title": "My Post",
            "status": "draft",
            "views": 0
        });
        assert_eq!(data, expected);

        // Test prepare_data (apply defaults + validate)
        let mut incomplete_data = serde_json::json!({"title": "Another Post"});
        assert!(schema.prepare_data(&mut incomplete_data).is_ok());
        assert_eq!(incomplete_data["status"], serde_json::json!("draft"));
        assert_eq!(incomplete_data["views"], serde_json::json!(0));
    }

    #[test]
    fn test_indexed_and_unique_fields() {
        let mut schema = CollectionSchema::new("products".to_string(), CollectionType::Base);

        schema.add_field(
            "name".to_string(),
            FieldDefinition::new(FieldType::Text).required().indexed(),
        );

        schema.add_field(
            "sku".to_string(),
            FieldDefinition::new(FieldType::Text).required().unique(),
        );

        schema.add_field(
            "category".to_string(),
            FieldDefinition::new(FieldType::Text).indexed(),
        );

        // Test getting indexed fields
        let indexed_fields = schema.get_indexed_fields();
        assert!(indexed_fields.contains(&"name".to_string()));
        assert!(indexed_fields.contains(&"category".to_string()));
        assert!(!indexed_fields.contains(&"sku".to_string()));

        // Test getting unique fields
        let unique_fields = schema.get_unique_fields();
        assert!(unique_fields.contains(&"sku".to_string()));
        assert!(!unique_fields.contains(&"name".to_string()));
    }

    #[test]
    fn test_number_validation_rules() {
        use crate::field_types::ValidationRules;

        let mut schema = CollectionSchema::new("products".to_string(), CollectionType::Base);

        let price_validation = ValidationRules::new().with_min(0.0).with_max(10000.0);

        schema.add_field(
            "price".to_string(),
            FieldDefinition::new(FieldType::Number)
                .required()
                .with_validation(price_validation),
        );

        // Valid price
        let valid_data = serde_json::json!({"price": 99.99});
        assert!(schema.validate_data(&valid_data).is_ok());

        // Price too low
        let invalid_low = serde_json::json!({"price": -10.0});
        assert!(schema.validate_data(&invalid_low).is_err());

        // Price too high
        let invalid_high = serde_json::json!({"price": 15000.0});
        assert!(schema.validate_data(&invalid_high).is_err());
    }
}
