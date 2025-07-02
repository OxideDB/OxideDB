//! Auth Schema Validator Hook
//!
//! This hook validates auth collection schemas to ensure they have the required
//! default fields and proper configurations. It enforces the auth collection
//! requirements at the schema level.

use crate::{AppError, CollectionSchema, CollectionType, FieldDefinition, FieldType, BeforeEventContext};
use crate::field_types::{ValidationRules, SelectConfig};
use std::collections::HashMap;
use tracing::{debug, warn, info};

/// Configuration for auth schema validation
#[derive(Debug, Clone)]
pub struct AuthSchemaValidatorConfig {
    /// Whether to enforce required auth fields
    pub enforce_required_fields: bool,
    /// Whether to auto-add missing required fields
    pub auto_add_missing_fields: bool,
    /// Required field definitions for auth collections
    pub required_fields: HashMap<String, FieldDefinition>,
}

impl Default for AuthSchemaValidatorConfig {
    fn default() -> Self {
        let mut required_fields = HashMap::new();
        
        // Email field - identifier for authentication
        required_fields.insert("email".to_string(), FieldDefinition {
            field_type: FieldType::Email,
            required: true,
            unique: true,
            default: None,
            validation: Some(ValidationRules {
                regex: Some("^[\\w\\.-]+@[\\w\\.-]+\\.[a-zA-Z]{2,}$".to_string()),
                min: None,
                max: Some(254.0), // RFC 5321 maximum email length
                message: Some("Please enter a valid email address".to_string()),
                allow_empty: Some(false),
            }),
            index: true, // Index for fast lookups
        });

        // Password field - credential for authentication
        required_fields.insert("password".to_string(), FieldDefinition {
            field_type: FieldType::Password,
            required: true,
            unique: false,
            default: None,
            validation: Some(ValidationRules {
                regex: None,
                min: Some(8.0), // Minimum password length
                max: Some(128.0), // Maximum password length
                message: Some("Password must be at least 8 characters long".to_string()),
                allow_empty: Some(false),
            }),
            index: false,
        });

        // Role field - user role management
        required_fields.insert("role".to_string(), FieldDefinition {
            field_type: FieldType::Select(SelectConfig {
                options: vec!["user".to_string(), "admin".to_string()],
                multiple: false,
                allow_empty: false,
            }),
            required: true,
            unique: false,
            default: Some(serde_json::Value::String("user".to_string())),
            validation: None,
            index: true, // Index for role-based queries
        });

        // Email verification status
        required_fields.insert("email_verified".to_string(), FieldDefinition {
            field_type: FieldType::Boolean,
            required: true,
            unique: false,
            default: Some(serde_json::Value::Bool(false)),
            validation: None,
            index: true, // Index for verification status queries
        });

        Self {
            enforce_required_fields: true,
            auto_add_missing_fields: true,
            required_fields,
        }
    }
}

/// Auth schema validator hook for enforcing auth collection requirements
pub struct AuthSchemaValidatorHook {
    config: AuthSchemaValidatorConfig,
}

impl AuthSchemaValidatorHook {
    /// Create a new auth schema validator hook with default configuration
    pub fn new() -> Self {
        Self {
            config: AuthSchemaValidatorConfig::default(),
        }
    }

    /// Create a new auth schema validator hook with custom configuration
    pub fn with_config(config: AuthSchemaValidatorConfig) -> Self {
        Self { config }
    }

    /// Validate and potentially modify an auth collection schema
    pub fn validate_auth_schema(&self, schema: &mut CollectionSchema) -> Result<(), AppError> {
        if schema.collection_type != CollectionType::Auth {
            return Ok(()); // Only validate auth collections
        }

        debug!("Validating auth collection schema: {}", schema.name);

        if self.config.enforce_required_fields {
            self.check_required_fields(schema)?;
        }

        if self.config.auto_add_missing_fields {
            self.add_missing_fields(schema)?;
        }

        self.validate_field_types(schema)?;
        self.validate_field_constraints(schema)?;

        info!("✅ Auth collection schema validated successfully: {}", schema.name);
        Ok(())
    }

    /// Check that all required fields are present
    fn check_required_fields(&self, schema: &CollectionSchema) -> Result<(), AppError> {
        let missing_fields: Vec<String> = self.config.required_fields
            .keys()
            .filter(|field_name| !schema.fields.contains_key(*field_name))
            .cloned()
            .collect();

        if !missing_fields.is_empty() && !self.config.auto_add_missing_fields {
            return Err(AppError::validation(
                "schema",
                &format!(
                    "Auth collection '{}' is missing required fields: {}. Required fields for auth collections are: email, password, role, email_verified",
                    schema.name,
                    missing_fields.join(", ")
                )
            ));
        }

        Ok(())
    }

    /// Add missing required fields to the schema
    fn add_missing_fields(&self, schema: &mut CollectionSchema) -> Result<(), AppError> {
        let mut added_fields = Vec::new();

        for (field_name, field_def) in &self.config.required_fields {
            if !schema.fields.contains_key(field_name) {
                schema.fields.insert(field_name.clone(), field_def.clone());
                added_fields.push(field_name.clone());
            }
        }

        if !added_fields.is_empty() {
            info!("📝 Auto-added missing auth fields to collection '{}': {}", 
                  schema.name, added_fields.join(", "));
        }

        Ok(())
    }

    /// Validate that required fields have the correct types
    fn validate_field_types(&self, schema: &CollectionSchema) -> Result<(), AppError> {
        for (field_name, required_def) in &self.config.required_fields {
            if let Some(actual_def) = schema.fields.get(field_name) {
                // Check if field types are compatible
                if !self.are_field_types_compatible(&required_def.field_type, &actual_def.field_type) {
                    return Err(AppError::validation(
                        field_name,
                        &format!(
                            "Field '{}' in auth collection '{}' has incorrect type. Expected: {:?}, Found: {:?}",
                            field_name, schema.name, required_def.field_type, actual_def.field_type
                        )
                    ));
                }
            }
        }

        Ok(())
    }

    /// Validate field constraints (required, unique, etc.)
    fn validate_field_constraints(&self, schema: &CollectionSchema) -> Result<(), AppError> {
        for (field_name, required_def) in &self.config.required_fields {
            if let Some(actual_def) = schema.fields.get(field_name) {
                // Check required constraint
                if required_def.required && !actual_def.required {
                    return Err(AppError::validation(
                        field_name,
                        &format!("Field '{}' in auth collection '{}' must be required", field_name, schema.name)
                    ));
                }

                // Check unique constraint for identifier fields
                if field_name == "email" && required_def.unique && !actual_def.unique {
                    return Err(AppError::validation(
                        field_name,
                        &format!("Field '{}' in auth collection '{}' must be unique", field_name, schema.name)
                    ));
                }
            }
        }

        Ok(())
    }

    /// Check if two field types are compatible
    fn are_field_types_compatible(&self, required: &FieldType, actual: &FieldType) -> bool {
        match (required, actual) {
            // Exact matches
            (FieldType::Email, FieldType::Email) => true,
            (FieldType::Password, FieldType::Password) => true,
            (FieldType::Boolean, FieldType::Boolean) => true,
            
            // Text can be used for email (less strict)
            (FieldType::Email, FieldType::Text) => true,
            
            // Text can be used for password (less strict)
            (FieldType::Password, FieldType::Text) => true,
            
            // Select field compatibility
            (FieldType::Select(_), FieldType::Select(_)) => true,
            
            // Text can be used for select (less strict)
            (FieldType::Select(_), FieldType::Text) => true,
            
            _ => false,
        }
    }

    /// Get the current configuration
    pub fn config(&self) -> &AuthSchemaValidatorConfig {
        &self.config
    }

    /// Validate a field name for auth collections
    pub fn validate_field_name(&self, field_name: &str, schema: &CollectionSchema) -> Result<(), AppError> {
        if schema.collection_type != CollectionType::Auth {
            return Ok(());
        }

        // Reserved field names for auth collections
        let reserved_names = ["id", "created_at", "updated_at"];
        if reserved_names.contains(&field_name) {
            return Err(AppError::validation(
                field_name,
                &format!("Field name '{}' is reserved for auth collections", field_name)
            ));
        }

        // Check for naming conflicts with auth operations
        let auth_operation_names = ["login", "register", "logout", "validate", "refresh", "me"];
        if auth_operation_names.contains(&field_name) {
            warn!("Field name '{}' in auth collection '{}' conflicts with auth operation names", 
                  field_name, schema.name);
        }

        Ok(())
    }

    /// Handle BeforeRecordCreate events for auth schema validation
    pub fn handle_before_record_create(&self, context: &mut crate::BeforeEventContext) -> Result<(), AppError> {
        // This hook validates during schema creation, not record creation
        // For now, we just skip record-level validation as the main validation
        // happens when the schema is created/updated
        debug!("Auth schema validator hook called for record creation in collection: {}", context.collection);
        Ok(())
    }
}

impl Default for AuthSchemaValidatorHook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CollectionType;

    fn create_minimal_auth_schema() -> CollectionSchema {
        CollectionSchema::new("users".to_string(), CollectionType::Auth)
    }

    fn create_complete_auth_schema() -> CollectionSchema {
        let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Auth);
        
        // Add all required fields manually
        schema.add_field("email".to_string(), FieldDefinition {
            field_type: FieldType::Email,
            required: true,
            unique: true,
            default: None,
            validation: None,
            index: false,
        });
        
        schema.add_field("password".to_string(), FieldDefinition {
            field_type: FieldType::Password,
            required: true,
            unique: false,
            default: None,
            validation: None,
            index: false,
        });
        
        schema.add_field("role".to_string(), FieldDefinition {
            field_type: FieldType::Text, // Compatible with select
            required: true,
            unique: false,
            default: Some(serde_json::Value::String("user".to_string())),
            validation: None,
            index: false,
        });
        
        schema.add_field("email_verified".to_string(), FieldDefinition {
            field_type: FieldType::Boolean,
            required: true,
            unique: false,
            default: Some(serde_json::Value::Bool(false)),
            validation: None,
            index: false,
        });
        
        schema
    }

    #[test]
    fn test_auto_add_missing_fields() {
        let hook = AuthSchemaValidatorHook::new();
        let mut schema = create_minimal_auth_schema();
        
        // Initially no fields
        assert_eq!(schema.fields.len(), 0);
        
        // Hook should add required fields
        assert!(hook.validate_auth_schema(&mut schema).is_ok());
        
        // Should now have all required fields
        assert!(schema.fields.contains_key("email"));
        assert!(schema.fields.contains_key("password"));
        assert!(schema.fields.contains_key("role"));
        assert!(schema.fields.contains_key("email_verified"));
    }

    #[test]
    fn test_validate_complete_schema() {
        let hook = AuthSchemaValidatorHook::new();
        let mut schema = create_complete_auth_schema();
        
        // Should validate successfully
        assert!(hook.validate_auth_schema(&mut schema).is_ok());
    }

    #[test]
    fn test_skip_base_collections() {
        let hook = AuthSchemaValidatorHook::new();
        let mut schema = CollectionSchema::new("posts".to_string(), CollectionType::Base);
        
        // Should skip validation for base collections
        assert!(hook.validate_auth_schema(&mut schema).is_ok());
        assert_eq!(schema.fields.len(), 0); // No fields added
    }

    #[test]
    fn test_field_type_compatibility() {
        let hook = AuthSchemaValidatorHook::new();
        
        // Test compatible types
        assert!(hook.are_field_types_compatible(&FieldType::Email, &FieldType::Email));
        assert!(hook.are_field_types_compatible(&FieldType::Email, &FieldType::Text));
        assert!(hook.are_field_types_compatible(&FieldType::Password, &FieldType::Text));
        
        // Test incompatible types
        assert!(!hook.are_field_types_compatible(&FieldType::Email, &FieldType::Boolean));
        assert!(!hook.are_field_types_compatible(&FieldType::Password, &FieldType::Number));
    }

    #[test]
    fn test_field_name_validation() {
        let hook = AuthSchemaValidatorHook::new();
        let schema = create_minimal_auth_schema();
        
        // Test reserved names
        assert!(hook.validate_field_name("id", &schema).is_err());
        assert!(hook.validate_field_name("created_at", &schema).is_err());
        
        // Test valid names
        assert!(hook.validate_field_name("first_name", &schema).is_ok());
        assert!(hook.validate_field_name("custom_field", &schema).is_ok());
    }
} 