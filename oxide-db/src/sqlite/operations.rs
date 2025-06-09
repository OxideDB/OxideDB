//! CRUD operations for SQLite database

use super::connection::SqliteDb;
use crate::{db::{Db, ListParams}, Record};
use oxide_core::{
    AppError,
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType,
    event::{RecordData, RecordId},
};
use tokio::task::spawn_blocking;
use tracing::debug;

#[async_trait::async_trait]
impl Db for SqliteDb {
    async fn initialize(&self) -> Result<(), AppError> {
        SqliteDb::initialize(self).await
    }

    async fn create_record(&self, collection: &str, data: RecordData) -> Result<Record, AppError> {
        // Create a mutable context for Before events (allows data transformation)
        let mut context = BeforeEventContext::new_create(collection.to_string(), data);
        
        // Dispatch BeforeRecordCreate event - handlers can modify the data
        self.event_bus
            .dispatch_before(BeforeEventType::RecordCreate, &mut context)
            .await?;
        
        // Extract the potentially modified data from the context
        let data = context.data;
        
        // Validate data against collection schema (after hook transformations)
        if let Ok(schema) = self.get_collection_schema(collection).await {
            if let Err(validation_error) = schema.validate_data(&data) {
                return Err(AppError::validation("data", &validation_error));
            }
        }

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
            .dispatch_after(AfterEventType::RecordCreated, &AfterEventContext::RecordCreated {
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
        // Create a mutable context for BeforeRecordRead event
        let mut context = BeforeEventContext {
            collection: collection.to_string(),
            data: serde_json::json!({"record_id": record_id}),
            metadata: serde_json::json!({}),
            record_id: Some(record_id.clone()),
            old_data: None,
        };

        // Dispatch BeforeRecordRead event
        self.event_bus
            .dispatch_before(BeforeEventType::RecordRead, &mut context)
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
            .dispatch_after(AfterEventType::RecordRead, &AfterEventContext::RecordRead {
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
        // First, get the existing record to include in events
        let old_record = <Self as Db>::read_record(self, collection, record_id).await?;

        // Create a mutable context for BeforeRecordUpdate event
        let mut context = BeforeEventContext::new_update(
            collection.to_string(),
            record_id.clone(),
            old_record.data.clone(),
            new_data,
        );

        // Dispatch BeforeRecordUpdate event
        self.event_bus
            .dispatch_before(BeforeEventType::RecordUpdate, &mut context)
            .await?;

        // Extract the potentially modified data from the context
        let new_data = context.data;

        // Validate data against collection schema
        if let Ok(schema) = self.get_collection_schema(collection).await {
            if let Err(validation_error) = schema.validate_data(&new_data) {
                return Err(AppError::validation("data", &validation_error));
            }
        }

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
            .dispatch_after(AfterEventType::RecordUpdated, &AfterEventContext::RecordUpdated {
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
        let record = <Self as Db>::read_record(self, collection, record_id).await?;

        // Create a mutable context for BeforeRecordDelete event
        let mut context = BeforeEventContext::new_delete(
            collection.to_string(),
            record_id.clone(),
            record.data.clone(),
        );

        // Dispatch BeforeRecordDelete event
        self.event_bus
            .dispatch_before(BeforeEventType::RecordDelete, &mut context)
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
            .dispatch_after(AfterEventType::RecordDeleted, &AfterEventContext::RecordDeleted {
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
        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let records = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut query = "SELECT id, collection, data, created_at, updated_at FROM records WHERE collection = ?1".to_string();
            let mut bind_params: Vec<String> = vec![collection_name];

            // Add sorting
            if let Some(sort_field) = &params.sort_field {
                let direction = if params.sort_ascending.unwrap_or(true) {
                    "ASC"
                } else {
                    "DESC"
                };
                query.push_str(&format!(" ORDER BY {} {}", sort_field, direction));
            } else {
                query.push_str(" ORDER BY created_at ASC");
            }

            // Add limit and offset
            if let Some(limit) = params.limit {
                query.push_str(" LIMIT ?");
                bind_params.push(limit.to_string());
            }

            if let Some(offset) = params.offset {
                query.push_str(" OFFSET ?");
                bind_params.push(offset.to_string());
            }

            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let records: Result<Vec<Record>, rusqlite::Error> = stmt
                .query_map(
                    rusqlite::params_from_iter(bind_params.iter()),
                    Self::row_to_record,
                )
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            records.map_err(|e| AppError::database(format!("Failed to query records: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Listed {} records from collection {}", records.len(), collection);
        Ok(records)
    }

    async fn create_collection(&self, collection: &str) -> Result<(), AppError> {
        SqliteDb::create_collection(self, collection).await
    }

    async fn create_collection_with_schema(&self, schema: oxide_core::CollectionSchema) -> Result<(), AppError> {
        SqliteDb::create_collection_with_schema(self, schema).await
    }

    async fn get_collection_schema(&self, collection: &str) -> Result<oxide_core::CollectionSchema, AppError> {
        SqliteDb::get_collection_schema(self, collection).await
    }

    async fn update_collection_schema(&self, collection: &str, schema: oxide_core::CollectionSchema) -> Result<(), AppError> {
        SqliteDb::update_collection_schema(self, collection, schema).await
    }

    async fn delete_collection(&self, collection: &str) -> Result<(), AppError> {
        SqliteDb::delete_collection(self, collection).await
    }

    async fn list_collections(&self) -> Result<Vec<String>, AppError> {
        SqliteDb::list_collections(self).await
    }

    async fn collection_exists(&self, collection: &str) -> Result<bool, AppError> {
        SqliteDb::collection_exists(self, collection).await
    }

    async fn count_records(&self, collection: &str) -> Result<usize, AppError> {
        SqliteDb::count_records(self, collection).await
    }

    async fn close(&self) -> Result<(), AppError> {
        SqliteDb::close(self).await
    }

    async fn health_check(&self) -> Result<(), AppError> {
        SqliteDb::health_check(self).await
    }
} 