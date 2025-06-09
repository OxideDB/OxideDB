//! Collection management operations for SQLite database

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

    /// Create a collection with a default base schema
    pub async fn create_collection(&self, collection: &str) -> Result<(), AppError> {
        // Create a default base collection schema
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

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            conn.execute(
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
                ) => AppError::conflict(&format!("Collection '{}' already exists", &schema_name)),
                _ => AppError::database(format!("Failed to create collection: {}", e)),
            })?;

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

        info!("Created collection with schema: {}", schema_name_for_events);
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

        let collection_name = collection.to_string();
        let collection_clone = collection_name.clone();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Delete all records in the collection first
            conn.execute(
                "DELETE FROM records WHERE collection = ?1",
                [&collection_name],
            )
            .map_err(|e| AppError::database(format!("Failed to delete records: {}", e)))?;

            // Delete the collection entry
            conn.execute(
                "DELETE FROM collections WHERE name = ?1",
                [&collection_name],
            )
            .map_err(|e| AppError::database(format!("Failed to delete collection: {}", e)))?;

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

        info!("Deleted collection: {}", collection_clone);
        Ok(())
    }

    /// List all collections
    pub async fn list_collections(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connection.clone();

        let collections = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT name FROM collections ORDER BY name")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let collection_names: Result<Vec<String>, rusqlite::Error> = stmt
                .query_map([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            collection_names
                .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Listed {} collections", collections.len());
        Ok(collections)
    }
}
