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
                match field_def.field_type.sql_type() {
                    "TEXT" => {
                        if let Some(s) = default.as_str() {
                            sql.push_str(&format!(" DEFAULT '{}'", s.replace('\'', "''")));
                        } else {
                            // For JSON fields, serialize the default value
                            sql.push_str(&format!(" DEFAULT '{}'", default.to_string().replace('\'', "''")));
                        }
                    }
                    "REAL" => {
                        if let Some(n) = default.as_f64() {
                            sql.push_str(&format!(" DEFAULT {}", n));
                        }
                    }
                    "INTEGER" => {
                        if default.is_boolean() {
                            if let Some(b) = default.as_bool() {
                                sql.push_str(&format!(" DEFAULT {}", if b { 1 } else { 0 }));
                            }
                        } else if let Some(i) = default.as_i64() {
                            sql.push_str(&format!(" DEFAULT {}", i));
                        }
                    }
                    _ => {
                        // Fallback to text representation
                        sql.push_str(&format!(" DEFAULT '{}'", default.to_string().replace('\'', "''")));
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
        // Use the new extensible field type system
        field_type.sql_type()
    }
    
    /// Generate SQL statements to migrate a table from old schema to new schema
    fn generate_migration_sql(&self, old_schema: &CollectionSchema, new_schema: &CollectionSchema) -> Vec<String> {
        let table_name = self.get_table_name(&new_schema.name);
        let mut migration_statements = Vec::new();
        
        // Detect breaking changes: removed fields or type changes
        let mut has_breaking_change = false;

        for (old_field_name, old_field_def) in &old_schema.fields {
            match new_schema.fields.get(old_field_name) {
                None => {
                    // Field removed => breaking change
                    has_breaking_change = true;
                    break;
                }
                Some(new_def) => {
                    if old_field_def.field_type != new_def.field_type {
                        has_breaking_change = true;
                        break;
                    }
                }
            }
        }

        if has_breaking_change {
            // Perform full table rebuild strategy
            let temp_table_name = format!("{}_new_v{}", table_name, new_schema.version);

            // 1. Create new table with desired schema
            let create_sql = self
                .generate_create_table_sql(new_schema)
                .replace(&table_name, &temp_table_name);
            migration_statements.push(create_sql);

            // 2. Copy intersecting fields from old table to new table
            let mut common_fields = vec!["id", "created_at", "updated_at"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
            for (field_name, _) in &new_schema.fields {
                if old_schema.fields.contains_key(field_name) {
                    common_fields.push(field_name.clone());
                }
            }
            let fields_str = common_fields.join(", ");
            migration_statements.push(format!(
                "INSERT INTO {temp} ({fields}) SELECT {fields} FROM {orig}",
                temp = temp_table_name,
                fields = fields_str,
                orig = table_name
            ));

            // 3. Drop old table
            migration_statements.push(format!("DROP TABLE {orig}", orig = table_name));

            // 4. Rename new table
            migration_statements.push(format!(
                "ALTER TABLE {temp} RENAME TO {orig}",
                temp = temp_table_name,
                orig = table_name
            ));

            // 5. Recreate indexes for the new schema
            migration_statements.extend(self.generate_index_sql(new_schema));

            return migration_statements;
        }

        // Non-breaking changes: handle additive modifications in-place

        // Find new fields that need to be added
        for (field_name, field_def) in &new_schema.fields {
            if !old_schema.fields.contains_key(field_name) {
                // This is a new field, generate ALTER TABLE ADD COLUMN statement
                let mut column_def = format!("{} {}", field_name, self.field_type_to_sql(&field_def.field_type));

                // Handle unique constraints via separate indexes (SQLite limitation)
                if field_def.unique {
                    migration_statements.push(format!(
                        "CREATE UNIQUE INDEX IF NOT EXISTS idx_{}_unique_{} ON {}({})",
                        table_name, field_name, table_name, field_name
                    ));
                }

                // Add default value if specified
                if let Some(default) = &field_def.default {
                    match field_def.field_type.sql_type() {
                        "TEXT" => {
                            if let Some(s) = default.as_str() {
                                column_def.push_str(&format!(" DEFAULT '{}'", s.replace('\'', "''")));
                            } else {
                                column_def.push_str(&format!(" DEFAULT '{}'", default.to_string().replace('\'', "''")));
                            }
                        }
                        "REAL" => {
                            if let Some(n) = default.as_f64() {
                                column_def.push_str(&format!(" DEFAULT {}", n));
                            }
                        }
                        "INTEGER" => {
                            if default.is_boolean() {
                                if let Some(b) = default.as_bool() {
                                    column_def.push_str(&format!(" DEFAULT {}", if b { 1 } else { 0 }));
                                }
                            } else if let Some(i) = default.as_i64() {
                                column_def.push_str(&format!(" DEFAULT {}", i));
                            }
                        }
                        _ => {
                            column_def.push_str(&format!(" DEFAULT '{}'", default.to_string().replace('\'', "''")));
                        }
                    }
                }

                migration_statements.push(format!("ALTER TABLE {} ADD COLUMN {}", table_name, column_def));
            }
        }

        // Add indexes for new fields or explicitly defined indexes
        for index_def in &new_schema.indexes {
            let has_new_fields = index_def
                .fields
                .iter()
                .any(|field| !old_schema.fields.contains_key(field));
            if has_new_fields {
                let index_type = if index_def.unique { "UNIQUE INDEX" } else { "INDEX" };
                let fields_str = index_def.fields.join(", ");
                migration_statements.push(format!(
                    "CREATE {} IF NOT EXISTS {} ON {}({})",
                    index_type, index_def.name, table_name, fields_str
                ));
            }
        }

        migration_statements
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
                index: false,
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
                index: false,
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
    fn test_migration_sql_generation() {
        let adapter = SqliteSchemaAdapter::new();
        
        // Create original schema
        let mut old_schema = CollectionSchema::new("users".to_string(), CollectionType::Base);
        old_schema.add_field(
            "email".to_string(),
            FieldDefinition {
                field_type: FieldType::Email,
                required: true,
                unique: true,
                default: None,
                validation: None,
                index: false,
            },
        );
        
        // Create new schema with additional field
        let mut new_schema = old_schema.clone();
        new_schema.add_field(
            "age".to_string(),
            FieldDefinition {
                field_type: FieldType::Number,
                required: false,
                unique: false,
                default: Some(serde_json::json!(0)),
                validation: None,
                index: false,
            },
        );
        new_schema.add_field(
            "verified".to_string(),
            FieldDefinition {
                field_type: FieldType::Boolean,
                required: false,
                unique: false,
                default: Some(serde_json::json!(false)),
                validation: None,
                index: false,
            },
        );
        
        let migration_sql = adapter.generate_migration_sql(&old_schema, &new_schema);
        
        // Should generate ALTER TABLE statements for new fields
        assert!(migration_sql.len() >= 2); // At least two new fields
        assert!(migration_sql.iter().any(|sql| sql.contains("ALTER TABLE collection_users ADD COLUMN age REAL DEFAULT 0")));
        assert!(migration_sql.iter().any(|sql| sql.contains("ALTER TABLE collection_users ADD COLUMN verified INTEGER DEFAULT 0")));
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
                index: false,
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
                index: false,
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
                index: false,
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
                index: false,
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