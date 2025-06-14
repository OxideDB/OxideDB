//! Schema Validator Hook
//!
//! This hook validates incoming data against the defined collection schema
//! to ensure data integrity and type safety.

use crate::{BeforeEventContext, AppError, CollectionSchema};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, warn};

/// Configuration for schema validation
#[derive(Debug, Clone)]
pub struct SchemaValidatorConfig {
    /// Whether to enforce strict validation (reject unknown fields)
    pub strict_mode: bool,
    /// Whether to auto-convert compatible types
    pub auto_type_conversion: bool,
    /// Collections to skip validation for
    pub skip_collections: Vec<String>,
    /// Whether to validate on updates (can be disabled for performance)
    pub validate_updates: bool,
}

impl Default for SchemaValidatorConfig {
    fn default() -> Self {
        Self {
            strict_mode: false,
            auto_type_conversion: true,
            skip_collections: vec!["_internal".to_string()],
            validate_updates: true,
        }
    }
}

/// Schema validator hook for enforcing data schemas
pub struct SchemaValidatorHook {
    config: SchemaValidatorConfig,
    schemas: Arc<RwLock<HashMap<String, CollectionSchema>>>,
}

impl SchemaValidatorHook {
    /// Create a new schema validator hook with default configuration
    pub fn new() -> Self {
        Self {
            config: SchemaValidatorConfig::default(),
            schemas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a new schema validator hook with custom configuration
    pub fn with_config(config: SchemaValidatorConfig) -> Self {
        Self {
            config,
            schemas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a schema for a collection
    pub fn register_schema(&self, collection: String, schema: CollectionSchema) -> Result<(), AppError> {
        let mut schemas = self.schemas.write().map_err(|_| {
            AppError::internal("Failed to acquire write lock for schemas")
        })?;
        
        schemas.insert(collection.clone(), schema);
        debug!("Registered schema for collection: {}", collection);
        Ok(())
    }

    /// Handle Before record create events for schema validation
    pub fn handle_before_record_create(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if self.should_skip_validation(&context.collection) {
            return Ok(());
        }

        debug!("Validating schema for create in collection: {}", context.collection);
        self.validate_against_schema(context)?;
        Ok(())
    }

    /// Handle Before record update events for schema validation
    pub fn handle_before_record_update(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        if !self.config.validate_updates || self.should_skip_validation(&context.collection) {
            return Ok(());
        }

        debug!("Validating schema for update in collection: {}", context.collection);
        self.validate_against_schema(context)?;
        Ok(())
    }

    /// Check if validation should be skipped for this collection
    fn should_skip_validation(&self, collection: &str) -> bool {
        self.config.skip_collections.contains(&collection.to_string())
    }

    /// Validate data against the registered schema
    fn validate_against_schema(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        let schemas = self.schemas.read().map_err(|_| {
            AppError::internal("Failed to acquire read lock for schemas")
        })?;

        if let Some(schema) = schemas.get(&context.collection) {
            // Validate the data against the schema
            if let Err(validation_error) = schema.validate_data(&context.data) {
                warn!("Schema validation failed for collection {}: {}", context.collection, validation_error);
                return Err(AppError::validation("schema", &validation_error));
            }

            // Apply type conversions if enabled
            if self.config.auto_type_conversion {
                self.apply_type_conversions(context, schema)?;
            }

            // Check for unknown fields in strict mode
            if self.config.strict_mode {
                self.check_unknown_fields(context, schema)?;
            }

            debug!("Schema validation passed for collection: {}", context.collection);
        } else {
            // No schema registered - decide based on strict mode
            if self.config.strict_mode {
                return Err(AppError::validation(
                    "schema", 
                    &format!("No schema defined for collection '{}' and strict mode is enabled", context.collection)
                ));
            } else {
                debug!("No schema found for collection '{}', skipping validation", context.collection);
            }
        }

        Ok(())
    }

    /// Apply automatic type conversions based on schema
    fn apply_type_conversions(&self, context: &mut BeforeEventContext, schema: &CollectionSchema) -> Result<(), AppError> {
        if let Some(data_obj) = context.data.as_object_mut() {
            for (field_name, field_def) in &schema.fields {
                if let Some(value) = data_obj.get_mut(field_name) {
                    // Try to convert the value to the expected type
                    match self.convert_value_type(value, &field_def.field_type) {
                        Ok(converted_value) => {
                            *value = converted_value;
                        }
                        Err(e) => {
                            warn!("Type conversion failed for field '{}': {}", field_name, e);
                            // Don't fail hard on type conversion errors unless in strict mode
                            if self.config.strict_mode {
                                return Err(AppError::validation(
                                    field_name,
                                    &format!("Type conversion failed: {}", e)
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Convert a value to the expected type
    fn convert_value_type(
        &self, 
        value: &serde_json::Value, 
        expected_type: &crate::FieldType
    ) -> Result<serde_json::Value, String> {
        use crate::FieldType;
        
        match expected_type {
            FieldType::Text => {
                match value {
                    serde_json::Value::String(_) => Ok(value.clone()),
                    serde_json::Value::Number(n) => Ok(serde_json::Value::String(n.to_string())),
                    serde_json::Value::Bool(b) => Ok(serde_json::Value::String(b.to_string())),
                    _ => Err("Cannot convert to text".to_string()),
                }
            }
            FieldType::Number => {
                match value {
                    serde_json::Value::Number(_) => Ok(value.clone()),
                    serde_json::Value::String(s) => {
                        s.parse::<f64>()
                            .map(|n| serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap()))
                            .map_err(|_| "Cannot convert string to number".to_string())
                    }
                    _ => Err("Cannot convert to number".to_string()),
                }
            }
            FieldType::Boolean => {
                match value {
                    serde_json::Value::Bool(_) => Ok(value.clone()),
                    serde_json::Value::String(s) => {
                        match s.to_lowercase().as_str() {
                            "true" | "1" | "yes" | "on" => Ok(serde_json::Value::Bool(true)),
                            "false" | "0" | "no" | "off" => Ok(serde_json::Value::Bool(false)),
                            _ => Err("Cannot convert string to boolean".to_string()),
                        }
                    }
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Ok(serde_json::Value::Bool(i != 0))
                        } else {
                            Err("Cannot convert number to boolean".to_string())
                        }
                    }
                    _ => Err("Cannot convert to boolean".to_string()),
                }
            }
            FieldType::Date => {
                match value {
                    serde_json::Value::String(s) => {
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
            FieldType::Email => {
                match value {
                    serde_json::Value::String(s) => {
                        // Basic email validation
                        if s.contains('@') && s.contains('.') {
                            Ok(value.clone())
                        } else {
                            Err("Invalid email format".to_string())
                        }
                    }
                    _ => Err("Email must be a string".to_string()),
                }
            }
            FieldType::Url => {
                match value {
                    serde_json::Value::String(s) => {
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
            FieldType::Json => {
                // JSON type accepts any valid JSON value
                Ok(value.clone())
            }
            FieldType::Password => {
                // Password fields should be strings
                match value {
                    serde_json::Value::String(_) => Ok(value.clone()),
                    _ => Err("Password must be a string".to_string()),
                }
            }
        }
    }

    /// Check for unknown fields in strict mode
    fn check_unknown_fields(&self, context: &BeforeEventContext, schema: &CollectionSchema) -> Result<(), AppError> {
        if let Some(data_obj) = context.data.as_object() {
            for field_name in data_obj.keys() {
                if !schema.fields.contains_key(field_name) {
                    return Err(AppError::validation(
                        field_name,
                        &format!("Unknown field '{}' not allowed in strict mode", field_name)
                    ));
                }
            }
        }
        Ok(())
    }

    /// Get all registered schemas
    pub fn get_schemas(&self) -> Result<HashMap<String, CollectionSchema>, AppError> {
        let schemas = self.schemas.read().map_err(|_| {
            AppError::internal("Failed to acquire read lock for schemas")
        })?;
        
        Ok(schemas.clone())
    }

    /// Get the current configuration
    pub fn config(&self) -> &SchemaValidatorConfig {
        &self.config
    }
}

impl Default for SchemaValidatorHook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CollectionType, FieldDefinition, FieldType};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_test_schema() -> CollectionSchema {
        let mut schema = CollectionSchema::new("test".to_string(), CollectionType::Base);
        let mut fields = HashMap::new();
        
        fields.insert("name".to_string(), FieldDefinition {
            field_type: FieldType::Text,
            required: true,
            default: None,
            validation: None,
            unique: false,
        });
        
        fields.insert("age".to_string(), FieldDefinition {
            field_type: FieldType::Number,
            required: false,
            default: Some(json!(0)),
            validation: None,
            unique: false,
        });
        
        schema.fields = fields;
        schema
    }

    #[test]
    fn test_schema_registration() {
        let hook = SchemaValidatorHook::new();
        let schema = create_test_schema();
        
        assert!(hook.register_schema("test".to_string(), schema).is_ok());
        
        let schemas = hook.get_schemas().unwrap();
        assert!(schemas.contains_key("test"));
    }

    #[test]
    fn test_schema_validation() {
        let hook = SchemaValidatorHook::new();
        let schema = create_test_schema();
        hook.register_schema("test".to_string(), schema).unwrap();

        let mut context = BeforeEventContext {
            collection: "test".to_string(),
            data: json!({
                "name": "John Doe",
                "age": 30
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        assert!(hook.handle_before_record_create(&mut context).is_ok());
    }

    #[test]
    fn test_type_conversion() {
        let mut config = SchemaValidatorConfig::default();
        config.auto_type_conversion = true;
        let hook = SchemaValidatorHook::with_config(config);
        
        let schema = create_test_schema();
        hook.register_schema("test".to_string(), schema).unwrap();

        let mut context = BeforeEventContext {
            collection: "test".to_string(),
            data: json!({
                "name": "John Doe",
                "age": "30"  // String that should be converted to number
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        assert!(hook.handle_before_record_create(&mut context).is_ok());
        
        // Check that age was converted to number
        assert!(context.data["age"].is_number());
    }

    #[test]
    fn test_strict_mode() {
        let mut config = SchemaValidatorConfig::default();
        config.strict_mode = true;
        let hook = SchemaValidatorHook::with_config(config);
        
        let schema = create_test_schema();
        hook.register_schema("test".to_string(), schema).unwrap();

        let mut context = BeforeEventContext {
            collection: "test".to_string(),
            data: json!({
                "name": "John Doe",
                "age": 30,
                "unknown_field": "should fail"
            }),
            metadata: json!({}),
            record_id: None,
            old_data: None,
        };

        // Should fail due to unknown field in strict mode
        assert!(hook.handle_before_record_create(&mut context).is_err());
    }
} 