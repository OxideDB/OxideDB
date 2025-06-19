//! Legacy authentication support
//!
//! This module provides backward compatibility functions and default collection
//! schemas for existing authentication implementations.

use crate::collection::{CollectionSchema, CollectionType, FieldDefinition};
use crate::field_types::FieldType;
use std::collections::HashMap;

/// Create default auth collections schemas (legacy support)
/// 
/// This function creates the default "_users" and "_superusers" collections
/// that were previously hardcoded in the authentication system. This provides
/// backward compatibility for existing installations.
pub fn create_auth_collections() -> (CollectionSchema, CollectionSchema) {
    // Users collection schema
    let mut users_schema = CollectionSchema::new("_users".to_string(), CollectionType::Auth);
    let mut users_fields = HashMap::new();
    
    users_fields.insert("email".to_string(), FieldDefinition {
        field_type: FieldType::Email,
        required: true,
        unique: true, // Email must be unique
        default: None,
        validation: None,
    });
    
    users_fields.insert("password".to_string(), FieldDefinition {
        field_type: FieldType::Password,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    
    users_fields.insert("verified".to_string(), FieldDefinition {
        field_type: FieldType::Boolean,
        required: false,
        unique: false,
        default: Some(serde_json::json!(false)),
        validation: None,
    });
    
    users_schema.fields = users_fields;
    
    // Superusers collection schema
    let mut superusers_schema = CollectionSchema::new("_superusers".to_string(), CollectionType::Auth);
    let mut superusers_fields = HashMap::new();
    
    superusers_fields.insert("email".to_string(), FieldDefinition {
        field_type: FieldType::Email,
        required: true,
        unique: true, // Email must be unique
        default: None,
        validation: None,
    });
    
    superusers_fields.insert("password".to_string(), FieldDefinition {
        field_type: FieldType::Password,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    
    superusers_fields.insert("verified".to_string(), FieldDefinition {
        field_type: FieldType::Boolean,
        required: false,
        unique: false,
        default: Some(serde_json::json!(true)), // Superusers are verified by default
        validation: None,
    });
    
    superusers_schema.fields = superusers_fields;
    
    (users_schema, superusers_schema)
} 