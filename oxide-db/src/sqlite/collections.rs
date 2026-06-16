//! Collection management operations for SQLite database

use super::{
    connection::SqliteDb, hooks::ensure_before_handlers_succeeded, schema_adapter::quote_identifier,
};
use crate::db::SchemaAdapter;
use oxide_core::{
    AfterEventContext, AfterEventType, AppError, BeforeEventContext, BeforeEventType,
    CollectionSchema, CollectionType,
};
use tokio::task::spawn_blocking;
use tracing::{debug, info};
use uuid::Uuid;

impl SqliteDb {
    /// Initialize system collections like _collections
    pub(super) async fn initialize_system_collections(&self) -> Result<(), AppError> {
        // Create _collections collection to track collection metadata
        let collections_schema =
            CollectionSchema::new("_collections".to_string(), CollectionType::Base);

        // Only create if it doesn't exist
        if !self.collection_exists("_collections").await? {
            self.create_collection_with_schema(collections_schema)
                .await?;
            info!("✅ Created _collections system collection");
        } else {
            debug!("_collections system collection already exists");
        }

        // Create _plugins collection to store plugin configurations
        use oxide_core::plugin_config::create_plugins_collection_schema;
        let plugins_schema = create_plugins_collection_schema();

        if !self.collection_exists("_plugins").await? {
            self.create_collection_with_schema(plugins_schema).await?;
            info!("✅ Created _plugins system collection");
        } else {
            debug!("_plugins system collection already exists");
        }

        // Create _site_settings collection to store site-wide configuration
        if !self.collection_exists("_site_settings").await? {
            self.create_site_settings_collection().await?;
        } else {
            debug!("_site_settings system collection already exists");
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
    pub async fn create_collection_with_schema(
        &self,
        schema: CollectionSchema,
    ) -> Result<(), AppError> {
        schema
            .validate_identifiers()
            .map_err(|e| AppError::validation("schema", &e))?;

        // Create a mutable context for BeforeCollectionCreate event
        let mut context = BeforeEventContext::new_create(
            schema.name.clone(),
            serde_json::to_value(&schema).unwrap_or_default(),
        );

        // Dispatch BeforeCollectionCreate event
        let before_results = self
            .event_bus
            .dispatch_before(BeforeEventType::CollectionCreate, &mut context)
            .await?;
        ensure_before_handlers_succeeded(&before_results)?;

        let connection = self.connection.clone();
        let schema_name = schema.name.clone();
        let schema_name_for_events = schema.name.clone();
        let schema_for_events = schema.clone(); // Clone schema for after event dispatch
        let schema_for_cache = schema.clone();
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

        self.schema_cache
            .write()
            .await
            .insert(schema_name_for_events.clone(), schema_for_cache);

        // Dispatch AfterCollectionCreate event
        let request_context = oxide_core::event::context::RequestContext::anonymous();
        self.event_bus
            .dispatch_after(
                AfterEventType::CollectionCreated,
                &AfterEventContext::CollectionCreated {
                    event_id: Uuid::new_v4().to_string(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    collection: schema_name_for_events.clone(),
                    schema: serde_json::to_value(&schema_for_events).unwrap_or_default(),
                    request_context,
                },
            )
            .await?;

        info!(
            "Created collection with dedicated table: {}",
            schema_name_for_events
        );
        Ok(())
    }

    /// Get the schema for a collection
    pub async fn get_collection_schema(
        &self,
        collection: &str,
    ) -> Result<CollectionSchema, AppError> {
        if let Some(schema) = self.schema_cache.read().await.get(collection).cloned() {
            return Ok(schema);
        }

        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let schema = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare_cached("SELECT schema FROM collections WHERE name = ?1")
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

        self.schema_cache
            .write()
            .await
            .insert(collection.to_string(), schema.clone());

        Ok(schema)
    }

    /// Update the schema for a collection
    pub async fn update_collection_schema(
        &self,
        collection: &str,
        mut schema: CollectionSchema,
    ) -> Result<(), AppError> {
        schema
            .validate_identifiers()
            .map_err(|e| AppError::validation("schema", &e))?;

        // Get the current schema to compare for migration
        let old_schema = self.get_collection_schema(collection).await?;

        // Ensure the new schema version is greater than the existing version
        if schema.version <= old_schema.version {
            return Err(AppError::validation(
                "schema.version".to_string(),
                format!(
                    "New schema version {} must be greater than current version {}",
                    schema.version, old_schema.version
                ),
            ));
        }

        // Create a mutable context for BeforeCollectionUpdate event
        let mut context = BeforeEventContext::new_collection_update(
            collection.to_string(),
            serde_json::to_value(&old_schema).unwrap_or_default(),
            serde_json::to_value(&schema).unwrap_or_default(),
        );

        // Dispatch BeforeCollectionUpdate event
        let before_results = self
            .event_bus
            .dispatch_before(BeforeEventType::CollectionUpdate, &mut context)
            .await?;
        ensure_before_handlers_succeeded(&before_results)?;

        let connection = self.connection.clone();
        let collection_name = collection.to_string();

        // Update the updated_at timestamp
        schema.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let schema_json = serde_json::to_string(&schema)
            .map_err(|e| AppError::database(format!("Failed to serialize schema: {}", e)))?;
        let schema_type = schema.collection_type.to_string();

        // Generate migration SQL using schema adapter
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let migration_statements = schema_adapter.generate_migration_sql(&old_schema, &schema);

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Start a transaction for atomicity
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| AppError::database(format!("Failed to start transaction: {}", e)))?;

            // Execute migration statements to update the table structure
            for migration_sql in migration_statements {
                tx.execute(&migration_sql, []).map_err(|e| {
                    AppError::database(format!(
                        "Failed to execute migration SQL '{}': {}",
                        migration_sql, e
                    ))
                })?;
            }

            // Update the schema metadata
            let rows_affected = tx
                .execute(
                    "UPDATE collections SET type = ?1, schema = ?2, updated_at = ?3 WHERE name = ?4",
                    [
                        &schema_type,
                        &schema_json,
                        &schema.updated_at.to_string(),
                        &collection_name,
                    ],
                )
                .map_err(|e| {
                    AppError::database(format!("Failed to update collection schema: {}", e))
                })?;

            if rows_affected == 0 {
                return Err(AppError::not_found("collection", &collection_name));
            }

            // Commit the transaction
            tx.commit().map_err(|e| {
                AppError::database(format!("Failed to commit schema update transaction: {}", e))
            })?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        self.schema_cache
            .write()
            .await
            .insert(collection.to_string(), schema.clone());

        // Dispatch AfterCollectionUpdated event
        let request_context = oxide_core::event::context::RequestContext::anonymous();
        let after_context = oxide_core::event::AfterEventContext::collection_updated(
            collection.to_string(),
            serde_json::to_value(&old_schema).unwrap_or_default(),
            serde_json::to_value(&schema).unwrap_or_default(),
            request_context,
        );
        self.event_bus
            .dispatch_after(AfterEventType::CollectionUpdated, &after_context)
            .await?;

        info!(
            "Updated schema and migrated table for collection: {}",
            collection
        );
        Ok(())
    }

    /// Delete a collection and all its records
    pub async fn delete_collection(&self, collection: &str) -> Result<(), AppError> {
        // Create a mutable context for BeforeCollectionDelete event
        let mut context = BeforeEventContext::new_delete(
            collection.to_string(),
            "".to_string(), // No specific record ID for collection operations
            serde_json::json!({"collection": collection}),
        );

        // Dispatch BeforeCollectionDelete event
        let before_results = self
            .event_bus
            .dispatch_before(BeforeEventType::CollectionDelete, &mut context)
            .await?;
        ensure_before_handlers_succeeded(&before_results)?;

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
            let tx = conn
                .unchecked_transaction()
                .map_err(|e| AppError::database(format!("Failed to start transaction: {}", e)))?;

            // Drop the dedicated collection table
            tx.execute(
                &format!("DROP TABLE IF EXISTS {}", quote_identifier(&table_name)),
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to drop collection table: {}", e)))?;

            // Delete the collection metadata entry
            let rows_affected = tx
                .execute(
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

        self.schema_cache.write().await.remove(&collection_clone);

        // Dispatch AfterCollectionDelete event
        let request_context = oxide_core::event::context::RequestContext::anonymous();
        self.event_bus
            .dispatch_after(
                AfterEventType::CollectionDeleted,
                &AfterEventContext::CollectionDeleted {
                    event_id: Uuid::new_v4().to_string(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                    collection: collection_clone.clone(),
                    request_context,
                },
            )
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
                .prepare_cached("SELECT schema FROM collections ORDER BY name")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let schemas: Result<Vec<CollectionSchema>, rusqlite::Error> = stmt
                .query_map([], |row| {
                    let schema_json: String = row.get(0)?;
                    let schema: CollectionSchema =
                        serde_json::from_str(&schema_json).map_err(|e| {
                            rusqlite::Error::FromSqlConversionFailure(
                                0,
                                rusqlite::types::Type::Text,
                                Box::new(e),
                            )
                        })?;
                    Ok(schema)
                })
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            schemas.map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        let mut schema_cache = self.schema_cache.write().await;
        schema_cache.clear();
        for schema in &collections {
            schema_cache.insert(schema.name.clone(), schema.clone());
        }

        debug!("Listed {} collections", collections.len());
        Ok(collections)
    }
}
