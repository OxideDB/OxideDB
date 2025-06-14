//! SQLite-specific schema adapter
//!
//! This module provides the SQLite implementation of the SchemaAdapter trait,
//! handling database-specific schema operations like SQL generation and table management.

use crate::db::SchemaAdapter;
use oxide_core::{CollectionSchema, FieldType};

/// SQLite implementation of the SchemaAdapter trait
///
/// This adapter provides SQLite-specific logic for handling collection schemas,
/// including SQL DDL generation and field type mapping.
pub struct SqliteSchemaAdapter;

impl SqliteSchemaAdapter {
    /// Create a new SQLite schema adapter
    pub fn new() -> Self {
        Self
    }
}

impl Default for SqliteSchemaAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaAdapter for SqliteSchemaAdapter {
    /// Get the table name for a collection
    fn get_table_name(&self, collection_name: &str) -> String {
        format!("collection_{}", collection_name)
    }

    /// Generate SQL DDL for creating a collection's table
    fn generate_create_table_sql(&self, schema: &CollectionSchema) -> String {
        let table_name = self.get_table_name(&schema.name);
        let mut sql = format!("CREATE TABLE IF NOT EXISTS {} (\n", table_name);
        
        // Always include the primary key and metadata columns
        sql.push_str("    id TEXT PRIMARY KEY,\n");
        sql.push_str("    created_at INTEGER NOT NULL,\n");
        sql.push_str("    updated_at INTEGER NOT NULL");

        // Add schema-defined fields
        for (field_name, field_def) in &schema.fields {
            sql.push_str(",\n    ");
            sql.push_str(&format!("{} {}", field_name, self.field_type_to_sql(&field_def.field_type)));
            
            if field_def.required {
                sql.push_str(" NOT NULL");
            }
            
            if field_def.unique {
                sql.push_str(" UNIQUE");
            }

            if let Some(default) = &field_def.default {
                match field_def.field_type {
                    FieldType::Text | FieldType::Email | FieldType::Url => {
                        if let Some(s) = default.as_str() {
                            sql.push_str(&format!(" DEFAULT '{}'", s.replace('\'', "''")));
                        }
                    }
                    FieldType::Number => {
                        if let Some(n) = default.as_f64() {
                            sql.push_str(&format!(" DEFAULT {}", n));
                        }
                    }
                    FieldType::Boolean => {
                        if let Some(b) = default.as_bool() {
                            sql.push_str(&format!(" DEFAULT {}", if b { 1 } else { 0 }));
                        }
                    }
                    FieldType::Date => {
                        if let Some(d) = default.as_i64() {
                            sql.push_str(&format!(" DEFAULT {}", d));
                        }
                    }
                    FieldType::Json => {
                        sql.push_str(&format!(" DEFAULT '{}'", default.to_string().replace('\'', "''")));
                    }
                    FieldType::Password => {
                        // Note: Password fields should not have default values as they should be hashed
                        // But if a default is specified, treat it as text (it will be hashed by the hook)
                        if let Some(s) = default.as_str() {
                            sql.push_str(&format!(" DEFAULT '{}'", s.replace('\'', "''")));
                        }
                    }
                }
            }
        }

        sql.push_str("\n)");
        sql
    }

    /// Generate SQL statements for creating indexes
    fn generate_index_sql(&self, schema: &CollectionSchema) -> Vec<String> {
        let table_name = self.get_table_name(&schema.name);
        let mut index_statements = Vec::new();

        // Always create index on created_at for sorting
        index_statements.push(format!(
            "CREATE INDEX IF NOT EXISTS idx_{}_{} ON {}({})",
            table_name, "created_at", table_name, "created_at"
        ));

        // Create indexes defined in schema
        for index_def in &schema.indexes {
            let index_type = if index_def.unique { "UNIQUE INDEX" } else { "INDEX" };
            let fields_str = index_def.fields.join(", ");
            index_statements.push(format!(
                "CREATE {} IF NOT EXISTS {} ON {}({})",
                index_type, index_def.name, table_name, fields_str
            ));
        }

        // Create unique indexes for fields marked as unique
        for (field_name, field_def) in &schema.fields {
            if field_def.unique {
                index_statements.push(format!(
                    "CREATE UNIQUE INDEX IF NOT EXISTS idx_{}_unique_{} ON {}({})",
                    table_name, field_name, table_name, field_name
                ));
            }
        }

        index_statements
    }

    /// Convert field type to SQLite column type
    fn field_type_to_sql(&self, field_type: &FieldType) -> &'static str {
        match field_type {
            FieldType::Text | FieldType::Email | FieldType::Url => "TEXT",
            FieldType::Number => "REAL",
            FieldType::Boolean => "INTEGER", // SQLite doesn't have native boolean
            FieldType::Date => "INTEGER",    // Store as unix timestamp
            FieldType::Json => "TEXT",       // Store as JSON string
            FieldType::Password => "TEXT",   // Store as hashed string
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::{CollectionType, FieldDefinition};
    use oxide_core::collection::IndexDefinition;

    #[test]
    fn test_table_name_generation() {
        let adapter = SqliteSchemaAdapter::new();
        assert_eq!(adapter.get_table_name("my_collection"), "collection_my_collection");
    }

    #[test]
    fn test_field_type_sql_conversion() {
        let adapter = SqliteSchemaAdapter::new();
        assert_eq!(adapter.field_type_to_sql(&FieldType::Text), "TEXT");
        assert_eq!(adapter.field_type_to_sql(&FieldType::Number), "REAL");
        assert_eq!(adapter.field_type_to_sql(&FieldType::Boolean), "INTEGER");
        assert_eq!(adapter.field_type_to_sql(&FieldType::Date), "INTEGER");
        assert_eq!(adapter.field_type_to_sql(&FieldType::Json), "TEXT");
        assert_eq!(adapter.field_type_to_sql(&FieldType::Email), "TEXT");
        assert_eq!(adapter.field_type_to_sql(&FieldType::Url), "TEXT");
        assert_eq!(adapter.field_type_to_sql(&FieldType::Password), "TEXT");
    }

    #[test]
    fn test_sql_generation() {
        let adapter = SqliteSchemaAdapter::new();
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
            "verified".to_string(),
            FieldDefinition {
                field_type: FieldType::Boolean,
                required: false,
                unique: false,
                default: Some(serde_json::json!(false)),
                validation: None,
            },
        );

        let sql = adapter.generate_create_table_sql(&schema);
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS collection_users"));
        assert!(sql.contains("email TEXT NOT NULL UNIQUE"));
        assert!(sql.contains("verified INTEGER DEFAULT 0"));

        let indexes = adapter.generate_index_sql(&schema);
        assert!(!indexes.is_empty());
    }

    #[test]
    fn test_password_field_sql_generation() {
        let adapter = SqliteSchemaAdapter::new();
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

        // Test SQL generation includes password fields
        let sql = adapter.generate_create_table_sql(&schema);
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS collection_users"));
        assert!(sql.contains("password TEXT NOT NULL"));
        assert!(sql.contains("backup_password TEXT"));
    }

    #[test]
    fn test_index_generation() {
        let adapter = SqliteSchemaAdapter::new();
        let mut schema = CollectionSchema::new("posts".to_string(), CollectionType::Base);
        
        // Add a field with unique constraint
        schema.add_field(
            "slug".to_string(),
            FieldDefinition {
                field_type: FieldType::Text,
                required: true,
                unique: true,
                default: None,
                validation: None,
            },
        );

        // Add a custom index
        schema.add_index(IndexDefinition {
            name: "idx_posts_title_author".to_string(),
            fields: vec!["title".to_string(), "author".to_string()],
            unique: false,
        });

        let indexes = adapter.generate_index_sql(&schema);
        
        // Should have created_at index, unique field index, and custom index
        assert!(indexes.len() >= 3);
        
        // Check for created_at index
        assert!(indexes.iter().any(|sql| sql.contains("idx_collection_posts_created_at")));
        
        // Check for unique field index
        assert!(indexes.iter().any(|sql| sql.contains("idx_collection_posts_unique_slug")));
        
        // Check for custom index
        assert!(indexes.iter().any(|sql| sql.contains("idx_posts_title_author")));
    }
} 