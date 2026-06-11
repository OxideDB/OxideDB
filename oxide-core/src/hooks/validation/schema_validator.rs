//! Schema Validator Hook
//!
//! This hook validates incoming data against the defined collection schema
//! to ensure data integrity and type safety.

use crate::{AppError, BeforeEventContext, CollectionSchema};
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
    pub fn register_schema(
        &self,
        collection: String,
        schema: CollectionSchema,
    ) -> Result<(), AppError> {
        let mut schemas = self
            .schemas
            .write()
            .map_err(|_| AppError::internal("Failed to acquire write lock for schemas"))?;

        schemas.insert(collection.clone(), schema);
        debug!("Registered schema for collection: {}", collection);
        Ok(())
    }

    /// Handle Before record create events for schema validation
    pub fn handle_before_record_create(
        &self,
        context: &mut BeforeEventContext,
    ) -> Result<(), AppError> {
        if self.should_skip_validation(&context.collection) {
            return Ok(());
        }

        debug!(
            "Validating schema for create in collection: {}",
            context.collection
        );
        self.validate_against_schema(context)?;
        Ok(())
    }

    /// Handle Before record update events for schema validation
    pub fn handle_before_record_update(
        &self,
        context: &mut BeforeEventContext,
    ) -> Result<(), AppError> {
        if !self.config.validate_updates || self.should_skip_validation(&context.collection) {
            return Ok(());
        }

        debug!(
            "Validating schema for update in collection: {}",
            context.collection
        );
        self.validate_against_schema(context)?;
        Ok(())
    }

    /// Check if validation should be skipped for this collection
    fn should_skip_validation(&self, collection: &str) -> bool {
        self.config
            .skip_collections
            .contains(&collection.to_string())
    }

    /// Validate data against the registered schema
    fn validate_against_schema(&self, context: &mut BeforeEventContext) -> Result<(), AppError> {
        let schemas = self
            .schemas
            .read()
            .map_err(|_| AppError::internal("Failed to acquire read lock for schemas"))?;

        if let Some(schema) = schemas.get(&context.collection) {
            // Apply type conversions first if enabled
            if self.config.auto_type_conversion {
                self.apply_type_conversions(context, schema)?;
            }

            // Then validate the data against the schema
            if let Err(validation_error) = schema.validate_data(&context.data) {
                warn!(
                    "Schema validation failed for collection {}: {}",
                    context.collection, validation_error
                );
                return Err(AppError::validation("schema", &validation_error));
            }

            // Check for unknown fields in strict mode
            if self.config.strict_mode {
                self.check_unknown_fields(context, schema)?;
            }

            debug!(
                "Schema validation passed for collection: {}",
                context.collection
            );
        } else {
            // No schema registered - decide based on strict mode
            if self.config.strict_mode {
                return Err(AppError::validation(
                    "schema",
                    &format!(
                        "No schema defined for collection '{}' and strict mode is enabled",
                        context.collection
                    ),
                ));
            } else {
                debug!(
                    "No schema found for collection '{}', skipping validation",
                    context.collection
                );
            }
        }

        Ok(())
    }

    /// Apply automatic type conversions based on schema
    fn apply_type_conversions(
        &self,
        context: &mut BeforeEventContext,
        schema: &CollectionSchema,
    ) -> Result<(), AppError> {
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
                                    &format!("Type conversion failed: {}", e),
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
        expected_type: &crate::FieldType,
    ) -> Result<serde_json::Value, String> {
        // Use the new extensible field type conversion
        expected_type.convert_value(value)
    }

    /// Check for unknown fields in strict mode
    fn check_unknown_fields(
        &self,
        context: &BeforeEventContext,
        schema: &CollectionSchema,
    ) -> Result<(), AppError> {
        if let Some(data_obj) = context.data.as_object() {
            for field_name in data_obj.keys() {
                if !schema.fields.contains_key(field_name) {
                    return Err(AppError::validation(
                        field_name,
                        &format!("Unknown field '{}' not allowed in strict mode", field_name),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Get all registered schemas
    pub fn get_schemas(&self) -> Result<HashMap<String, CollectionSchema>, AppError> {
        let schemas = self
            .schemas
            .read()
            .map_err(|_| AppError::internal("Failed to acquire read lock for schemas"))?;

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

        fields.insert(
            "name".to_string(),
            FieldDefinition {
                field_type: FieldType::Text,
                required: true,
                default: None,
                validation: None,
                unique: false,
                index: false,
            },
        );

        fields.insert(
            "age".to_string(),
            FieldDefinition {
                field_type: FieldType::Number,
                required: false,
                default: Some(json!(0)),
                validation: None,
                unique: false,
                index: false,
            },
        );

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

        let mut context = BeforeEventContext::new_create(
            "test".to_string(),
            json!({
                "name": "John Doe",
                "age": 30
            }),
        );

        assert!(hook.handle_before_record_create(&mut context).is_ok());
    }

    #[test]
    fn test_type_conversion() {
        let config = SchemaValidatorConfig {
            auto_type_conversion: true,
            ..Default::default()
        };
        let hook = SchemaValidatorHook::with_config(config);

        let schema = create_test_schema();
        hook.register_schema("test".to_string(), schema).unwrap();

        let mut context = BeforeEventContext::new_create(
            "test".to_string(),
            json!({
                "name": "John Doe",
                "age": "30"  // String that should be converted to number
            }),
        );

        assert!(hook.handle_before_record_create(&mut context).is_ok());

        // Check that age was converted to number
        assert!(context.data["age"].is_number());
    }

    #[test]
    fn test_strict_mode() {
        let config = SchemaValidatorConfig {
            strict_mode: true,
            ..Default::default()
        };
        let hook = SchemaValidatorHook::with_config(config);

        let schema = create_test_schema();
        hook.register_schema("test".to_string(), schema).unwrap();

        let mut context = BeforeEventContext::new_create(
            "test".to_string(),
            json!({
                "name": "John Doe",
                "age": 30,
                "unknown_field": "should fail"
            }),
        );

        // Should fail due to unknown field in strict mode
        assert!(hook.handle_before_record_create(&mut context).is_err());
    }
}
