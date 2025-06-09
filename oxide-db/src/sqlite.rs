//! SQLite implementation of the database interface

use crate::{
    db::{Db, ListParams},
    Record,
};
use oxide_core::{
    event::{Event, EventBus, RecordData, RecordId},
    AppError,
    CollectionSchema, CollectionType, FieldDefinition, FieldType,
};
use rusqlite::{Connection, Row};
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;
use tracing::{debug, info};
use uuid::Uuid;

/// SQLite implementation of the Db trait
///
/// This implementation uses SQLite as the underlying database and integrates
/// with the EventBus to dispatch events for all operations. The connection
/// is wrapped in Arc<Mutex<>> to allow safe concurrent access.
pub struct SqliteDb {
    connection: Arc<Mutex<Connection>>,
    event_bus: Arc<dyn EventBus>,
}

impl SqliteDb {
    /// Create a new SqliteDb instance
    ///
    /// # Arguments
    /// * `database_path` - Path to the SQLite database file (use ":memory:" for in-memory)
    /// * `event_bus` - The event bus for dispatching events
    pub fn new(database_path: &str, event_bus: Arc<dyn EventBus>) -> Result<Self, AppError> {
        let connection = Connection::open(database_path)
            .map_err(|e| AppError::database(format!("Failed to open SQLite database: {}", e)))?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            event_bus,
        })
    }

    /// Generate a new UUID for a record
    fn generate_record_id() -> RecordId {
        Uuid::new_v4().to_string()
    }

    /// Convert a SQLite row to a Record
    fn row_to_record(row: &Row) -> Result<Record, rusqlite::Error> {
        let data_str: String = row.get("data")?;
        let data = serde_json::from_str(&data_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(Record {
            id: row.get("id")?,
            collection: row.get("collection")?,
            data,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
    /// Initialize system collections like _collections
    async fn initialize_system_collections(&self) -> Result<(), AppError> {
        // Check if _collections system collection already exists
        if self.collection_exists("_collections").await? {
            debug!("System collection _collections already exists");
            return Ok(());
        }

        // Create the _collections system collection schema
        let mut collections_schema = CollectionSchema::new("_collections".to_string(), CollectionType::Base);
        
        // Add fields for collection metadata
        collections_schema.add_field(
            "name".to_string(),
            FieldDefinition {
                field_type: FieldType::Text,
                required: true,
                default: None,
                validation: None,
            },
        );
        
        collections_schema.add_field(
            "type".to_string(),
            FieldDefinition {
                field_type: FieldType::Text,
                required: true,
                default: Some(serde_json::Value::String("base".to_string())),
                validation: None,
            },
        );
        
        collections_schema.add_field(
            "schema".to_string(),
            FieldDefinition {
                field_type: FieldType::Json,
                required: true,
                default: None,
                validation: None,
            },
        );

        // Create the system collection
        self.create_collection_with_schema(collections_schema).await?;
        
        info!("✅ Initialized _collections system collection");
        Ok(())
    }
}

#[async_trait::async_trait]
impl Db for SqliteDb {
    async fn initialize(&self) -> Result<(), AppError> {
        info!("Initializing SQLite database");

        let connection = self.connection.clone();
        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Create the records table
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS records (
                    id TEXT PRIMARY KEY,
                    collection TEXT NOT NULL,
                    data TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create records table: {}", e)))?;

            // Create indexes for performance
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_records_collection ON records(collection)",
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create collection index: {}", e)))?;

            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_records_created_at ON records(created_at)",
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create created_at index: {}", e)))?;

            // Create the collections table to track collections with schema support
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS collections (
                    id TEXT PRIMARY KEY,
                    name TEXT UNIQUE NOT NULL,
                    type TEXT NOT NULL CHECK (type IN ('base', 'auth')),
                    schema TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to create collections table: {}", e))
            })?;

            info!("SQLite database initialized successfully");
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Initialize system collections
        self.initialize_system_collections().await?;

        // Dispatch system startup event
        self.event_bus.dispatch(Event::OnSystemStartup).await?;

        Ok(())
    }

    async fn create_record(&self, collection: &str, data: RecordData) -> Result<Record, AppError> {
        // Validate data against collection schema
        if let Ok(schema) = self.get_collection_schema(collection).await {
            if let Err(validation_error) = schema.validate_data(&data) {
                return Err(AppError::validation("data", &validation_error));
            }
        }

        // Dispatch BeforeRecordCreate event
        self.event_bus
            .dispatch(Event::BeforeRecordCreate {
                collection: collection.to_string(),
                data: data.clone(),
            })
            .await?;

        let record_id = Self::generate_record_id();
        let collection = collection.to_string();
        let connection = self.connection.clone();

        let record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let data_str = serde_json::to_string(&data)
                .map_err(|e| AppError::database(format!("Failed to serialize data: {}", e)))?;

            conn.execute(
                "INSERT INTO records (id, collection, data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                [&record_id, &collection, &data_str, &now.to_string(), &now.to_string()],
            )
            .map_err(|e| AppError::database(format!("Failed to insert record: {}", e)))?;

            Ok::<Record, AppError>(Record {
                id: record_id,
                collection,
                data,
                created_at: now,
                updated_at: now,
            })
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordCreate event
        self.event_bus
            .dispatch(Event::AfterRecordCreate {
                collection: record.collection.clone(),
                record_id: record.id.clone(),
                data: record.data.clone(),
            })
            .await?;

        debug!(
            "Created record {} in collection {}",
            record.id, record.collection
        );
        Ok(record)
    }

    async fn read_record(
        &self,
        collection: &str,
        record_id: &RecordId,
    ) -> Result<Record, AppError> {
        // Dispatch BeforeRecordRead event
        self.event_bus
            .dispatch(Event::BeforeRecordRead {
                collection: collection.to_string(),
                record_id: record_id.clone(),
            })
            .await?;

        let collection = collection.to_string();
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        let record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT id, collection, data, created_at, updated_at FROM records WHERE id = ?1 AND collection = ?2")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let record = stmt
                .query_row([&record_id, &collection], Self::row_to_record)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::not_found("record", &record_id)
                    }
                    _ => AppError::database(format!("Failed to query record: {}", e)),
                })?;

            Ok::<Record, AppError>(record)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordRead event
        self.event_bus
            .dispatch(Event::AfterRecordRead {
                collection: record.collection.clone(),
                record_id: record.id.clone(),
                data: record.data.clone(),
            })
            .await?;

        debug!(
            "Read record {} from collection {}",
            record.id, record.collection
        );
        Ok(record)
    }

    async fn update_record(
        &self,
        collection: &str,
        record_id: &RecordId,
        new_data: RecordData,
    ) -> Result<Record, AppError> {
        // Validate data against collection schema
        if let Ok(schema) = self.get_collection_schema(collection).await {
            if let Err(validation_error) = schema.validate_data(&new_data) {
                return Err(AppError::validation("data", &validation_error));
            }
        }

        // First, get the existing record to include in events
        let old_record = self.read_record(collection, record_id).await?;

        // Dispatch BeforeRecordUpdate event
        self.event_bus
            .dispatch(Event::BeforeRecordUpdate {
                collection: collection.to_string(),
                record_id: record_id.clone(),
                old_data: old_record.data.clone(),
                new_data: new_data.clone(),
            })
            .await?;

        let collection = collection.to_string();
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        let updated_record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let data_str = serde_json::to_string(&new_data)
                .map_err(|e| AppError::database(format!("Failed to serialize data: {}", e)))?;

            let rows_affected = conn
                .execute(
                    "UPDATE records SET data = ?1, updated_at = ?2 WHERE id = ?3 AND collection = ?4",
                    [&data_str, &now.to_string(), &record_id, &collection],
                )
                .map_err(|e| AppError::database(format!("Failed to update record: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("record", &record_id));
            }

            Ok(Record {
                id: record_id,
                collection,
                data: new_data,
                created_at: old_record.created_at,
                updated_at: now,
            })
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordUpdate event
        self.event_bus
            .dispatch(Event::AfterRecordUpdate {
                collection: updated_record.collection.clone(),
                record_id: updated_record.id.clone(),
                old_data: old_record.data,
                new_data: updated_record.data.clone(),
            })
            .await?;

        debug!(
            "Updated record {} in collection {}",
            updated_record.id, updated_record.collection
        );
        Ok(updated_record)
    }

    async fn delete_record(
        &self,
        collection: &str,
        record_id: &RecordId,
    ) -> Result<Record, AppError> {
        // First, get the existing record to include in events
        let record = self.read_record(collection, record_id).await?;

        // Dispatch BeforeRecordDelete event
        self.event_bus
            .dispatch(Event::BeforeRecordDelete {
                collection: collection.to_string(),
                record_id: record_id.clone(),
                data: record.data.clone(),
            })
            .await?;

        let collection = collection.to_string();
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let rows_affected = conn
                .execute(
                    "DELETE FROM records WHERE id = ?1 AND collection = ?2",
                    [&record_id, &collection],
                )
                .map_err(|e| AppError::database(format!("Failed to delete record: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("record", &record_id));
            }

            Ok(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordDelete event
        self.event_bus
            .dispatch(Event::AfterRecordDelete {
                collection: record.collection.clone(),
                record_id: record.id.clone(),
                data: record.data.clone(),
            })
            .await?;

        debug!(
            "Deleted record {} from collection {}",
            record.id, record.collection
        );
        Ok(record)
    }

    async fn list_records(
        &self,
        collection: &str,
        params: ListParams,
    ) -> Result<Vec<Record>, AppError> {
        let collection = collection.to_string();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Build the query with optional sorting, limit, and offset
            let mut query = "SELECT id, collection, data, created_at, updated_at FROM records WHERE collection = ?1".to_string();

            if let Some(sort_field) = &params.sort_field {
                let direction = if params.sort_ascending.unwrap_or(true) { "ASC" } else { "DESC" };
                query.push_str(&format!(" ORDER BY {} {}", sort_field, direction));
            } else {
                query.push_str(" ORDER BY created_at DESC");
            }

            if let Some(limit) = params.limit {
                query.push_str(&format!(" LIMIT {}", limit));
            }

            if let Some(offset) = params.offset {
                query.push_str(&format!(" OFFSET {}", offset));
            }

            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let records: Result<Vec<Record>, rusqlite::Error> = stmt
                .query_map([&collection], Self::row_to_record)
                .map_err(|e| AppError::database(format!("Failed to query records: {}", e)))?
                .collect();

            let records = records
                .map_err(|e| AppError::database(format!("Failed to query records: {}", e)))?;

            debug!("Listed {} records from collection {}", records.len(), collection);
            Ok(records)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    async fn create_collection(&self, collection: &str) -> Result<(), AppError> {
        // Create a default base collection schema
        let schema = CollectionSchema::new(collection.to_string(), CollectionType::Base);
        self.create_collection_with_schema(schema).await
    }

    async fn create_collection_with_schema(&self, schema: CollectionSchema) -> Result<(), AppError> {
        // Dispatch BeforeCollectionCreate event
        self.event_bus
            .dispatch(Event::BeforeCollectionCreate {
                collection: schema.name.clone(),
            })
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
            .dispatch(Event::AfterCollectionCreate { 
                collection: schema_name_for_events.clone(),
            })
            .await?;

        info!("Created collection with schema: {}", schema_name_for_events);
        Ok(())
    }

    async fn get_collection_schema(&self, collection: &str) -> Result<CollectionSchema, AppError> {
        let collection = collection.to_string();
        let collection_for_debug = collection.clone();
        let connection = self.connection.clone();

        let schema = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT schema FROM collections WHERE name = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let schema_json: String = stmt
                .query_row([&collection], |row| row.get(0))
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::not_found("collection", &collection)
                    }
                    _ => AppError::database(format!("Failed to query collection schema: {}", e)),
                })?;

            let schema: CollectionSchema = serde_json::from_str(&schema_json)
                .map_err(|e| AppError::database(format!("Failed to deserialize schema: {}", e)))?;

            Ok::<CollectionSchema, AppError>(schema)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Retrieved schema for collection: {}", collection_for_debug);
        Ok(schema)
    }

    async fn update_collection_schema(&self, collection: &str, mut schema: CollectionSchema) -> Result<(), AppError> {
        // Update the schema timestamp
        schema.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let connection = self.connection.clone();
        let collection = collection.to_string();
        let collection_for_info = collection.clone();
        let schema_json = serde_json::to_string(&schema)
            .map_err(|e| AppError::database(format!("Failed to serialize schema: {}", e)))?;

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let rows_affected = conn
                .execute(
                    "UPDATE collections SET schema = ?1, updated_at = ?2 WHERE name = ?3",
                    [&schema_json, &schema.updated_at.to_string(), &collection],
                )
                .map_err(|e| AppError::database(format!("Failed to update collection schema: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("collection", &collection));
            }

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        info!("Updated schema for collection: {}", collection_for_info);
        Ok(())
    }

    async fn delete_collection(&self, collection: &str) -> Result<(), AppError> {
        // Dispatch BeforeCollectionDelete event
        self.event_bus
            .dispatch(Event::BeforeCollectionDelete {
                collection: collection.to_string(),
            })
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
            .dispatch(Event::AfterCollectionDelete {
                collection: collection_clone.clone(),
            })
            .await?;

        info!("Deleted collection: {}", collection_clone);
        Ok(())
    }

    async fn list_collections(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT name FROM collections ORDER BY name")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let collections: Result<Vec<String>, rusqlite::Error> = stmt
                .query_map([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))?
                .collect();

            let collections = collections
                .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))?;

            debug!("Listed {} collections", collections.len());
            Ok(collections)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    async fn collection_exists(&self, collection: &str) -> Result<bool, AppError> {
        let collection = collection.to_string();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM collections WHERE name = ?1",
                    [&collection],
                    |row| row.get(0),
                )
                .map_err(|e| {
                    AppError::database(format!("Failed to check collection existence: {}", e))
                })?;

            Ok(count > 0)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    async fn count_records(&self, collection: &str) -> Result<usize, AppError> {
        let collection = collection.to_string();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM records WHERE collection = ?1",
                    [&collection],
                    |row| row.get(0),
                )
                .map_err(|e| AppError::database(format!("Failed to count records: {}", e)))?;

            Ok(count as usize)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    async fn close(&self) -> Result<(), AppError> {
        info!("Closing SQLite database connection");

        // Dispatch system shutdown event
        self.event_bus.dispatch(Event::OnSystemShutdown).await?;

        // SQLite connections are automatically closed when dropped
        // No explicit close needed for rusqlite::Connection
        Ok(())
    }

    async fn health_check(&self) -> Result<(), AppError> {
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Simple query to verify the connection is working
            conn.query_row("SELECT 1", [], |_| Ok(()))
                .map_err(|e| AppError::database(format!("Health check failed: {}", e)))?;

            Ok(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }
}
