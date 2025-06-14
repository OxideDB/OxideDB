//! Collection management operations for SQLite database

use crate::db::SchemaAdapter;
use super::connection::SqliteDb;
use oxide_core::{
    AppError,
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType,
    CollectionSchema, CollectionType,
};
use tokio::task::spawn_blocking;
use tracing::{info, debug};

impl SqliteDb {
    /// Initialize system collections like _collections
    pub(super) async fn initialize_system_collections(&self) -> Result<(), AppError> {
        // Create _collections collection to track collection metadata
        let collections_schema = CollectionSchema::new("_collections".to_string(), CollectionType::Base);
        
        // Only create if it doesn't exist
        if !self.collection_exists("_collections").await? {
            self.create_collection_with_schema(collections_schema).await?;
            info!("✅ Created _collections system collection");
        } else {
            debug!("_collections system collection already exists");
        }
        Ok(())
    }

    /// Create a collection with a default base schema (deprecated)
    #[deprecated(note = "Use create_collection_with_schema instead to enforce schema requirement")]
    pub async fn create_collection(&self, collection: &str) -> Result<(), AppError> {
        // Create a default base collection schema with no fields (discouraged)
        let schema = CollectionSchema::new(collection.to_string(), CollectionType::Base);
        self.create_collection_with_schema(schema).await
    }

    /// Create a collection with a specific schema
    pub async fn create_collection_with_schema(&self, schema: CollectionSchema) -> Result<(), AppError> {
        // Create a mutable context for BeforeCollectionCreate event
        let mut context = BeforeEventContext {
            collection: schema.name.clone(),
            data: serde_json::to_value(&schema).unwrap_or_default(),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };

        // Dispatch BeforeCollectionCreate event
        self.event_bus
            .dispatch_before(BeforeEventType::CollectionCreate, &mut context)
            .await?;

        let connection = self.connection.clone();
        let schema_name = schema.name.clone();
        let schema_name_for_events = schema.name.clone();
        let schema_json = serde_json::to_string(&schema)
            .map_err(|e| AppError::database(format!("Failed to serialize schema: {}", e)))?;

        // Generate SQL for the collection table and indexes using schema adapter
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let create_table_sql = schema_adapter.generate_create_table_sql(&schema);
        let index_sql_statements = schema_adapter.generate_index_sql(&schema);

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Start a transaction for atomicity
            let tx = conn.unchecked_transaction()
                .map_err(|e| AppError::database(format!("Failed to start transaction: {}", e)))?;

            // Insert collection metadata
            tx.execute(
                "INSERT INTO collections (id, name, type, schema, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                [
                    &schema.id,
                    &schema.name,
                    &schema.collection_type.to_string(),
                    &schema_json,
                    &schema.created_at.to_string(),
                    &schema.updated_at.to_string(),
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: rusqlite::ErrorCode::ConstraintViolation,
                        ..
                    },
                    _,
                ) => AppError::conflict(format!("Collection '{}' already exists", &schema_name)),
                _ => AppError::database(format!("Failed to create collection: {}", e)),
            })?;

            // Create the dedicated table for this collection
            tx.execute(&create_table_sql, [])
                .map_err(|e| AppError::database(format!("Failed to create collection table: {}", e)))?;

            // Create indexes for the collection table
            for index_sql in index_sql_statements {
                tx.execute(&index_sql, [])
                    .map_err(|e| AppError::database(format!("Failed to create index: {}", e)))?;
            }

            // Commit the transaction
            tx.commit()
                .map_err(|e| AppError::database(format!("Failed to commit transaction: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterCollectionCreate event
        self.event_bus
            .dispatch_after(AfterEventType::CollectionCreated, &AfterEventContext::CollectionCreated { 
                collection: schema_name_for_events.clone(),
            })
            .await?;

        info!("Created collection with dedicated table: {}", schema_name_for_events);
        Ok(())
    }

    /// Get the schema for a collection
    pub async fn get_collection_schema(&self, collection: &str) -> Result<CollectionSchema, AppError> {
        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let schema = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT schema FROM collections WHERE name = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let schema_json: String = stmt
                .query_row([&collection_name], |row| row.get(0))
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::not_found("collection", &collection_name)
                    }
                    _ => AppError::database(format!("Failed to query collection: {}", e)),
                })?;

            let schema: CollectionSchema = serde_json::from_str(&schema_json)
                .map_err(|e| AppError::database(format!("Failed to deserialize schema: {}", e)))?;

            Ok::<CollectionSchema, AppError>(schema)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(schema)
    }

    /// Update the schema for a collection
    pub async fn update_collection_schema(&self, collection: &str, mut schema: CollectionSchema) -> Result<(), AppError> {
        let connection = self.connection.clone();
        let collection_name = collection.to_string();

        // Update the updated_at timestamp
        schema.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let schema_json = serde_json::to_string(&schema)
            .map_err(|e| AppError::database(format!("Failed to serialize schema: {}", e)))?;

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let rows_affected = conn
                .execute(
                    "UPDATE collections SET schema = ?1, updated_at = ?2 WHERE name = ?3",
                    [&schema_json, &schema.updated_at.to_string(), &collection_name],
                )
                .map_err(|e| AppError::database(format!("Failed to update collection schema: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("collection", &collection_name));
            }

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        info!("Updated schema for collection: {}", collection);
        Ok(())
    }

    /// Delete a collection and all its records
    pub async fn delete_collection(&self, collection: &str) -> Result<(), AppError> {
        // Create a mutable context for BeforeCollectionDelete event
        let mut context = BeforeEventContext {
            collection: collection.to_string(),
            data: serde_json::json!({"collection": collection}),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };

        // Dispatch BeforeCollectionDelete event
        self.event_bus
            .dispatch_before(BeforeEventType::CollectionDelete, &mut context)
            .await?;

        // Get the schema to determine the table name
        let schema = self.get_collection_schema(collection).await?;
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&schema.name);

        let collection_name = collection.to_string();
        let collection_clone = collection_name.clone();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Start a transaction for atomicity
            let tx = conn.unchecked_transaction()
                .map_err(|e| AppError::database(format!("Failed to start transaction: {}", e)))?;

            // Drop the dedicated collection table
            tx.execute(&format!("DROP TABLE IF EXISTS {}", table_name), [])
                .map_err(|e| AppError::database(format!("Failed to drop collection table: {}", e)))?;

            // Delete the collection metadata entry
            let rows_affected = tx.execute(
                "DELETE FROM collections WHERE name = ?1",
                [&collection_name],
            )
            .map_err(|e| AppError::database(format!("Failed to delete collection: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("collection", &collection_name));
            }

            // Commit the transaction
            tx.commit()
                .map_err(|e| AppError::database(format!("Failed to commit transaction: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterCollectionDelete event
        self.event_bus
            .dispatch_after(AfterEventType::CollectionDeleted, &AfterEventContext::CollectionDeleted {
                collection: collection_clone.clone(),
            })
            .await?;

        info!("Deleted collection and its table: {}", collection_clone);
        Ok(())
    }

    /// List all collections
    pub async fn list_collections(&self) -> Result<Vec<CollectionSchema>, AppError> {
        let connection = self.connection.clone();

        let collections = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT schema FROM collections ORDER BY name")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let schemas: Result<Vec<CollectionSchema>, rusqlite::Error> = stmt
                .query_map([], |row| {
                    let schema_json: String = row.get(0)?;
                    let schema: CollectionSchema = serde_json::from_str(&schema_json)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
                    Ok(schema)
                })
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            schemas
                .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Listed {} collections", collections.len());
        Ok(collections)
    }
}
