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

pub(crate) fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn create_field_index_sql(table_name: &str, field_name: &str) -> String {
    let index_name = format!("idx_{}_{}", table_name, field_name);
    format!(
        "CREATE INDEX IF NOT EXISTS {} ON {}({})",
        quote_identifier(&index_name),
        quote_identifier(table_name),
        quote_identifier(field_name)
    )
}

fn create_unique_field_index_sql(table_name: &str, field_name: &str) -> String {
    let index_name = format!("idx_{}_unique_{}", table_name, field_name);
    format!(
        "CREATE UNIQUE INDEX IF NOT EXISTS {} ON {}({})",
        quote_identifier(&index_name),
        quote_identifier(table_name),
        quote_identifier(field_name)
    )
}

fn create_schema_index_sql(
    table_name: &str,
    index_def: &oxide_core::collection::IndexDefinition,
) -> String {
    let index_type = if index_def.unique {
        "UNIQUE INDEX"
    } else {
        "INDEX"
    };
    let fields_str = index_def
        .fields
        .iter()
        .map(|field| quote_identifier(field))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "CREATE {} IF NOT EXISTS {} ON {}({})",
        index_type,
        quote_identifier(&index_def.name),
        quote_identifier(table_name),
        fields_str
    )
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
        let mut sql = format!(
            "CREATE TABLE IF NOT EXISTS {} (\n",
            quote_identifier(&table_name)
        );

        // Always include the primary key and metadata columns
        sql.push_str("    \"id\" TEXT PRIMARY KEY,\n");
        sql.push_str("    \"created_at\" INTEGER NOT NULL,\n");
        sql.push_str("    \"updated_at\" INTEGER NOT NULL");

        // Add schema-defined fields
        for (field_name, field_def) in &schema.fields {
            sql.push_str(",\n    ");
            sql.push_str(&format!(
                "{} {}",
                quote_identifier(field_name),
                self.field_type_to_sql(&field_def.field_type)
            ));

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
                            sql.push_str(&format!(
                                " DEFAULT '{}'",
                                default.to_string().replace('\'', "''")
                            ));
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
                        sql.push_str(&format!(
                            " DEFAULT '{}'",
                            default.to_string().replace('\'', "''")
                        ));
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

        // Always create indexes on metadata fields commonly used for sorting.
        index_statements.push(create_field_index_sql(&table_name, "created_at"));
        index_statements.push(create_field_index_sql(&table_name, "updated_at"));

        // Create indexes defined in schema
        for index_def in &schema.indexes {
            index_statements.push(create_schema_index_sql(&table_name, index_def));
        }

        // Create indexes for fields marked as unique or explicitly indexed.
        for (field_name, field_def) in &schema.fields {
            if field_def.unique {
                index_statements.push(create_unique_field_index_sql(&table_name, field_name));
            } else if field_def.index {
                index_statements.push(create_field_index_sql(&table_name, field_name));
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
    fn generate_migration_sql(
        &self,
        old_schema: &CollectionSchema,
        new_schema: &CollectionSchema,
    ) -> Vec<String> {
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
            let create_sql = self.generate_create_table_sql(new_schema).replace(
                &quote_identifier(&table_name),
                &quote_identifier(&temp_table_name),
            );
            migration_statements.push(create_sql);

            // 2. Copy intersecting fields from old table to new table
            let mut common_fields = vec!["id", "created_at", "updated_at"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>();
            for field_name in new_schema.fields.keys() {
                if old_schema.fields.contains_key(field_name) {
                    common_fields.push(field_name.clone());
                }
            }
            let fields_str = common_fields
                .iter()
                .map(|field| quote_identifier(field))
                .collect::<Vec<_>>()
                .join(", ");
            migration_statements.push(format!(
                "INSERT INTO {temp} ({fields}) SELECT {fields} FROM {orig}",
                temp = quote_identifier(&temp_table_name),
                fields = fields_str,
                orig = quote_identifier(&table_name)
            ));

            // 3. Drop old table
            migration_statements.push(format!(
                "DROP TABLE {orig}",
                orig = quote_identifier(&table_name)
            ));

            // 4. Rename new table
            migration_statements.push(format!(
                "ALTER TABLE {temp} RENAME TO {orig}",
                temp = quote_identifier(&temp_table_name),
                orig = quote_identifier(&table_name)
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
                let mut column_def = format!(
                    "{} {}",
                    quote_identifier(field_name),
                    self.field_type_to_sql(&field_def.field_type)
                );

                // Add default value if specified
                if let Some(default) = &field_def.default {
                    match field_def.field_type.sql_type() {
                        "TEXT" => {
                            if let Some(s) = default.as_str() {
                                column_def
                                    .push_str(&format!(" DEFAULT '{}'", s.replace('\'', "''")));
                            } else {
                                column_def.push_str(&format!(
                                    " DEFAULT '{}'",
                                    default.to_string().replace('\'', "''")
                                ));
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
                                    column_def
                                        .push_str(&format!(" DEFAULT {}", if b { 1 } else { 0 }));
                                }
                            } else if let Some(i) = default.as_i64() {
                                column_def.push_str(&format!(" DEFAULT {}", i));
                            }
                        }
                        _ => {
                            column_def.push_str(&format!(
                                " DEFAULT '{}'",
                                default.to_string().replace('\'', "''")
                            ));
                        }
                    }
                }

                migration_statements.push(format!(
                    "ALTER TABLE {} ADD COLUMN {}",
                    quote_identifier(&table_name),
                    column_def
                ));

                if field_def.unique {
                    migration_statements
                        .push(create_unique_field_index_sql(&table_name, field_name));
                } else if field_def.index {
                    migration_statements.push(create_field_index_sql(&table_name, field_name));
                }
            }
        }

        // Add indexes when existing fields become indexed.
        for (field_name, field_def) in &new_schema.fields {
            if let Some(old_field_def) = old_schema.fields.get(field_name) {
                if field_def.unique && !old_field_def.unique {
                    migration_statements
                        .push(create_unique_field_index_sql(&table_name, field_name));
                } else if field_def.index && !old_field_def.index && !field_def.unique {
                    migration_statements.push(create_field_index_sql(&table_name, field_name));
                }
            }
        }

        // Add newly declared schema indexes, including indexes on existing fields.
        for index_def in &new_schema.indexes {
            let existing_index = old_schema.indexes.iter().any(|old_index| {
                old_index.name == index_def.name
                    && old_index.fields == index_def.fields
                    && old_index.unique == index_def.unique
            });

            if !existing_index {
                migration_statements.push(create_schema_index_sql(&table_name, index_def));
            }
        }

        migration_statements
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::collection::IndexDefinition;
    use oxide_core::{CollectionType, FieldDefinition};

    #[test]
    fn test_table_name_generation() {
        let adapter = SqliteSchemaAdapter::new();
        assert_eq!(
            adapter.get_table_name("my_collection"),
            "collection_my_collection"
        );
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
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS \"collection_users\""));
        assert!(sql.contains("\"email\" TEXT NOT NULL UNIQUE"));
        assert!(sql.contains("\"verified\" INTEGER DEFAULT 0"));

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
        assert!(migration_sql.iter().any(|sql| sql
            .contains("ALTER TABLE \"collection_users\" ADD COLUMN \"age\" REAL DEFAULT 0")));
        assert!(migration_sql.iter().any(|sql| sql.contains(
            "ALTER TABLE \"collection_users\" ADD COLUMN \"verified\" INTEGER DEFAULT 0"
        )));
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
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS \"collection_users\""));
        assert!(sql.contains("\"password\" TEXT NOT NULL"));
        assert!(sql.contains("\"backup_password\" TEXT"));
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
        schema.add_field(
            "category".to_string(),
            FieldDefinition::new(FieldType::Text).indexed(),
        );

        // Add a custom index
        schema.add_index(IndexDefinition {
            name: "idx_posts_title_author".to_string(),
            fields: vec!["title".to_string(), "author".to_string()],
            unique: false,
        });

        let indexes = adapter.generate_index_sql(&schema);

        // Should have metadata indexes, unique field index, indexed field index, and custom index.
        assert!(indexes.len() >= 5);

        // Check for created_at index
        assert!(indexes
            .iter()
            .any(|sql| sql.contains("idx_collection_posts_created_at")));

        // Check for updated_at index
        assert!(indexes
            .iter()
            .any(|sql| sql.contains("idx_collection_posts_updated_at")));

        // Check for unique field index
        assert!(indexes
            .iter()
            .any(|sql| sql.contains("idx_collection_posts_unique_slug")));

        // Check for indexed field index
        assert!(indexes
            .iter()
            .any(|sql| sql.contains("idx_collection_posts_category")));

        // Check for custom index
        assert!(indexes
            .iter()
            .any(|sql| sql.contains("idx_posts_title_author")));
    }

    #[test]
    fn test_migration_adds_indexes_for_existing_fields() {
        let adapter = SqliteSchemaAdapter::new();

        let mut old_schema = CollectionSchema::new("posts".to_string(), CollectionType::Base);
        old_schema.add_field("title".to_string(), FieldDefinition::new(FieldType::Text));
        old_schema.add_field("author".to_string(), FieldDefinition::new(FieldType::Text));

        let mut new_schema = old_schema.clone();
        new_schema.fields.insert(
            "title".to_string(),
            FieldDefinition::new(FieldType::Text).indexed(),
        );
        new_schema.add_index(IndexDefinition {
            name: "idx_posts_author_title".to_string(),
            fields: vec!["author".to_string(), "title".to_string()],
            unique: false,
        });

        let migration_sql = adapter.generate_migration_sql(&old_schema, &new_schema);

        assert!(migration_sql
            .iter()
            .any(|sql| sql.contains("idx_collection_posts_title")));
        assert!(migration_sql
            .iter()
            .any(|sql| sql.contains("idx_posts_author_title")));
    }
}
