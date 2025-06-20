//! Relationship Field Type Demo
//!
//! This example demonstrates the new relationship field type in OxideDB,
//! which allows creating relationships between collections.

use oxide_core::{
    CollectionSchema, CollectionType, FieldDefinition, FieldType,
    field_types::{RelationshipConfig, RelationshipFieldType}
};
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 OxideDB Relationship Field Type Demo");
    println!("=======================================\n");

    // Create a users collection
    let mut users_schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
    users_schema.add_field("name".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    users_schema.add_field("email".to_string(), FieldDefinition {
        field_type: FieldType::Email,
        required: true,
        unique: true,
        default: None,
        validation: None,
    });

    println!("📊 Created 'users' collection schema");
    println!("Fields: name (text), email (email)\n");

    // Create a posts collection with a relationship to users
    let mut posts_schema = CollectionSchema::new("posts".to_string(), CollectionType::Base);
    posts_schema.add_field("title".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    posts_schema.add_field("content".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });

    // Add a single relationship field to reference the author (user)
    let author_relationship = RelationshipConfig {
        target_collection: "users".to_string(),
        multiple: false,
        cascade_delete: false,
        display_field: Some("name".to_string()),
    };
    posts_schema.add_field("author_id".to_string(), FieldDefinition {
        field_type: FieldType::Relationship(author_relationship),
        required: true,
        unique: false,
        default: None,
        validation: None,
    });

    println!("📝 Created 'posts' collection schema");
    println!("Fields: title (text), content (text), author_id (relationship -> users)\n");

    // Create a tags collection
    let mut tags_schema = CollectionSchema::new("tags".to_string(), CollectionType::Base);
    tags_schema.add_field("name".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: true,
        default: None,
        validation: None,
    });
    tags_schema.add_field("color".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: false,
        unique: false,
        default: Some(json!("#blue")),
        validation: None,
    });

    println!("🏷️  Created 'tags' collection schema");
    println!("Fields: name (text), color (text)\n");

    // Add multiple relationship field to posts for tags
    let tags_relationship = RelationshipConfig {
        target_collection: "tags".to_string(),
        multiple: true,
        cascade_delete: false,
        display_field: Some("name".to_string()),
    };
    posts_schema.add_field("tag_ids".to_string(), FieldDefinition {
        field_type: FieldType::Relationship(tags_relationship),
        required: false,
        unique: false,
        default: None,
        validation: None,
    });

    println!("🔗 Added multiple relationship to 'posts' collection");
    println!("Field: tag_ids (relationship -> tags, multiple=true)\n");

    // Demonstrate relationship field validation
    println!("✅ Testing Relationship Field Validation");
    println!("==========================================");

    // Get the relationship fields
    let author_field = posts_schema.fields.get("author_id").unwrap();
    let tags_field = posts_schema.fields.get("tag_ids").unwrap();

    // Test single relationship validation
    println!("\n📌 Single Relationship (author_id):");
    let valid_author_id = json!("user-12345678");
    let invalid_author_id = json!(["multiple", "ids"]);
    
    match author_field.field_type.validate("author_id", &valid_author_id) {
        Ok(_) => println!("  ✅ Valid single ID: {}", valid_author_id),
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    match author_field.field_type.validate("author_id", &invalid_author_id) {
        Ok(_) => println!("  ✅ Valid: {}", invalid_author_id),
        Err(e) => println!("  ❌ Expected error for array in single field: {}", e),
    }

    // Test multiple relationship validation
    println!("\n📌 Multiple Relationship (tag_ids):");
    let valid_tag_ids = json!(["tag-12345678", "tag-87654321", "tag-11111111"]);
    let invalid_tag_ids = json!("single-id-string");
    
    match tags_field.field_type.validate("tag_ids", &valid_tag_ids) {
        Ok(_) => println!("  ✅ Valid multiple IDs: {}", valid_tag_ids),
        Err(e) => println!("  ❌ Error: {}", e),
    }
    
    match tags_field.field_type.validate("tag_ids", &invalid_tag_ids) {
        Ok(_) => println!("  ✅ Valid: {}", invalid_tag_ids),
        Err(e) => println!("  ❌ Expected error for string in multiple field: {}", e),
    }

    // Show field type information
    println!("\n📋 Field Type Information");
    println!("==========================");
    
    if let FieldType::Relationship(config) = &author_field.field_type {
        println!("Author field:");
        println!("  Target collection: {}", config.target_collection);
        println!("  Multiple: {}", config.multiple);
        println!("  Cascade delete: {}", config.cascade_delete);
        println!("  Display field: {:?}", config.display_field);
        println!("  SQL type: {}", author_field.field_type.sql_type());
    }

    if let FieldType::Relationship(config) = &tags_field.field_type {
        println!("\nTags field:");
        println!("  Target collection: {}", config.target_collection);
        println!("  Multiple: {}", config.multiple);
        println!("  Cascade delete: {}", config.cascade_delete);
        println!("  Display field: {:?}", config.display_field);
        println!("  SQL type: {}", tags_field.field_type.sql_type());
    }

    // Show example record data
    println!("\n📄 Example Record Data");
    println!("=======================");
    
    let example_post = json!({
        "title": "Getting Started with OxideDB Relationships",
        "content": "This post explains how to use the new relationship field type...",
        "author_id": "user-12345678",
        "tag_ids": ["tag-12345678", "tag-87654321"]
    });
    
    println!("Post record:");
    println!("{}", serde_json::to_string_pretty(&example_post)?);

    println!("\n🎯 When populated with relationship data:");
    let populated_example = json!({
        "title": "Getting Started with OxideDB Relationships",
        "content": "This post explains how to use the new relationship field type...",
        "author_id": "user-12345678",
        "author_id_populated": {
            "id": "user-12345678",
            "display": "John Doe"
        },
        "tag_ids": ["tag-12345678", "tag-87654321"],
        "tag_ids_populated": [
            {
                "id": "tag-12345678",
                "display": "Tutorial"
            },
            {
                "id": "tag-87654321", 
                "display": "Database"
            }
        ]
    });
    
    println!("{}", serde_json::to_string_pretty(&populated_example)?);

    println!("\n🚀 Usage in API:");
    println!("================");
    println!("GET /collections/posts/records?populate_relationships=true");
    println!("  ↳ Returns records with relationship data populated");
    println!("\nGET /collections/posts/records");
    println!("  ↳ Returns records with just the relationship IDs");

    println!("\n✨ Summary");
    println!("==========");
    println!("🔗 Relationship fields support both single and multiple references");
    println!("📊 Relationships can be configured with cascade delete and display fields");
    println!("🔍 Validation ensures IDs are in the correct format");
    println!("💾 Storage uses TEXT columns to store IDs as JSON");
    println!("🚀 API supports optional relationship population for performance");

    Ok(())
} 