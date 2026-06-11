//! Demonstration of the new field validation system
//!
//! This example shows how to use:
//! - Regex validation rules
//! - Min/max constraints  
//! - Default values
//! - Required, unique, and indexed fields
//! - Custom error messages

use oxide_core::collection::{CollectionSchema, CollectionType, FieldDefinition};
use oxide_core::field_types::{FieldType, ValidationRules};
use serde_json::json;

fn main() {
    println!("=== OxideDB Field Validation System Demo ===\n");

    // Create a user collection with comprehensive validation
    let users_schema = create_users_schema();

    // Example 1: Valid user data
    println!("1. Testing valid user data:");
    let valid_user = json!({
        "username": "john_doe123",
        "email": "john@example.com",
        "age": 25,
        "bio": "Software developer"
    });

    let mut test_data = valid_user.clone();
    match users_schema.prepare_data(&mut test_data) {
        Ok(()) => {
            println!("✅ Valid user data accepted");
            println!(
                "   Data with defaults: {}",
                serde_json::to_string_pretty(&test_data).unwrap()
            );
        }
        Err(e) => println!("❌ Unexpected error: {}", e),
    }

    // Example 2: Invalid email format
    println!("\n2. Testing invalid email format:");
    let invalid_email = json!({
        "username": "jane_doe",
        "email": "invalid-email-format",
        "age": 30
    });

    match users_schema.validate_data(&invalid_email) {
        Ok(()) => println!("❌ Should have failed validation"),
        Err(e) => println!("✅ Correctly rejected: {}", e),
    }

    // Example 3: Username too short
    println!("\n3. Testing username too short:");
    let short_username = json!({
        "username": "jo",
        "email": "jo@example.com",
        "age": 20
    });

    match users_schema.validate_data(&short_username) {
        Ok(()) => println!("❌ Should have failed validation"),
        Err(e) => println!("✅ Correctly rejected: {}", e),
    }

    // Example 4: Invalid username characters
    println!("\n4. Testing invalid username characters:");
    let invalid_username = json!({
        "username": "john-doe!",
        "email": "john@example.com",
        "age": 25
    });

    match users_schema.validate_data(&invalid_username) {
        Ok(()) => println!("❌ Should have failed validation"),
        Err(e) => println!("✅ Correctly rejected: {}", e),
    }

    // Example 5: Age out of range
    println!("\n5. Testing age out of range:");
    let invalid_age = json!({
        "username": "older_user",
        "email": "older@example.com",
        "age": 150
    });

    match users_schema.validate_data(&invalid_age) {
        Ok(()) => println!("❌ Should have failed validation"),
        Err(e) => println!("✅ Correctly rejected: {}", e),
    }

    // Example 6: Missing required field
    println!("\n6. Testing missing required field:");
    let missing_required = json!({
        "email": "test@example.com",
        "age": 25
        // username is missing
    });

    match users_schema.validate_data(&missing_required) {
        Ok(()) => println!("❌ Should have failed validation"),
        Err(e) => println!("✅ Correctly rejected: {}", e),
    }

    // Example 7: Show default values application
    println!("\n7. Testing default values:");
    let partial_data = json!({
        "username": "new_user",
        "email": "new@example.com"
        // age and status will get defaults
    });

    let mut with_defaults = partial_data.clone();
    match users_schema.apply_defaults(&mut with_defaults) {
        Ok(()) => {
            println!("✅ Defaults applied successfully");
            println!(
                "   Original: {}",
                serde_json::to_string_pretty(&partial_data).unwrap()
            );
            println!(
                "   With defaults: {}",
                serde_json::to_string_pretty(&with_defaults).unwrap()
            );
        }
        Err(e) => println!("❌ Error applying defaults: {}", e),
    }

    // Example 8: Show indexed and unique fields
    println!("\n8. Schema metadata:");
    println!("   Indexed fields: {:?}", users_schema.get_indexed_fields());
    println!("   Unique fields: {:?}", users_schema.get_unique_fields());

    // Check which fields require hashing
    for field_name in ["username", "email", "password"] {
        if users_schema.field_requires_hashing(field_name) {
            println!("   Field '{}' requires hashing", field_name);
        }
    }
}

fn create_users_schema() -> CollectionSchema {
    let mut schema = CollectionSchema::new("users".to_string(), CollectionType::Base);

    // Username: alphanumeric + underscore, 3-20 chars, required, unique, indexed
    let username_validation = ValidationRules::new()
        .with_regex(r"^[a-zA-Z0-9_]+$".to_string())
        .with_min(3.0)
        .with_max(20.0)
        .with_message(
            "Username must be 3-20 characters and contain only letters, numbers, and underscores"
                .to_string(),
        );

    schema.add_field(
        "username".to_string(),
        FieldDefinition::new(FieldType::Text)
            .required()
            .unique()
            .indexed()
            .with_validation(username_validation),
    );

    // Email: valid email format, required, unique, indexed
    let email_validation = ValidationRules::new()
        .with_regex(r"^[^@]+@[^@]+\.[^@]+$".to_string())
        .with_message("Please enter a valid email address".to_string());

    schema.add_field(
        "email".to_string(),
        FieldDefinition::new(FieldType::Email)
            .required()
            .unique()
            .indexed()
            .with_validation(email_validation),
    );

    // Age: 13-120, optional with default of 18
    let age_validation = ValidationRules::new()
        .with_min(13.0)
        .with_max(120.0)
        .with_message("Age must be between 13 and 120".to_string());

    schema.add_field(
        "age".to_string(),
        FieldDefinition::new(FieldType::Number)
            .with_default(json!(18))
            .with_validation(age_validation),
    );

    // Bio: optional text, max 500 chars
    let bio_validation = ValidationRules::new()
        .with_max(500.0)
        .with_message("Bio must be less than 500 characters".to_string());

    schema.add_field(
        "bio".to_string(),
        FieldDefinition::new(FieldType::Text).with_validation(bio_validation),
    );

    // Status: enum-like field with default
    schema.add_field(
        "status".to_string(),
        FieldDefinition::new(FieldType::Text).with_default(json!("active")),
    );

    // Password: auto-hashed, required
    schema.add_field(
        "password".to_string(),
        FieldDefinition::new(FieldType::Password).required(),
    );

    schema
}
