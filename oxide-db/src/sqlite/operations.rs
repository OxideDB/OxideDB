//! CRUD operations for SQLite database

use super::connection::SqliteDb;
use crate::{db::{Db, ListParams, SchemaAdapter}, Record};
use oxide_core::{
    AppError, FieldType,
    BeforeEventContext, BeforeEventType, AfterEventType,
    event::types::{RecordData, RecordId},
};
use tokio::task::spawn_blocking;
use tracing::debug;
use serde_json::Value as JsonValue;

impl SqliteDb {
    /// Convert a record data JSON to SQL values for a specific schema
    fn record_data_to_sql_values(
        &self,
        data: &RecordData,
        schema: &oxide_core::CollectionSchema,
    ) -> Result<Vec<(String, SqlValue)>, AppError> {
        let mut sql_values = Vec::new();
        
        for (field_name, field_def) in &schema.fields {
            if let Some(value) = data.get(field_name) {
                let sql_value = match field_def.field_type.sql_type() {
                    "TEXT" => {
                        // For JSON or File fields, serialize the entire JSON value as a string
                        if matches!(field_def.field_type, FieldType::File(_))
                            || matches!(field_def.field_type, FieldType::Json)
                        {
                            SqlValue::Text(value.to_string())
                        } else {
                            SqlValue::Text(value.as_str().unwrap_or("").to_string())
                        }
                    }
                    "REAL" => SqlValue::Real(value.as_f64().unwrap_or(0.0)),
                    "INTEGER" => {
                        // Handle both boolean (stored as integer) and actual integers
                        if value.is_boolean() {
                            SqlValue::Integer(if value.as_bool().unwrap_or(false) { 1 } else { 0 })
                        } else {
                            SqlValue::Integer(value.as_i64().unwrap_or(0))
                        }
                    }
                    _ => SqlValue::Text(value.to_string()), // Fallback to text
                };
                sql_values.push((field_name.clone(), sql_value));
            } else if let Some(default) = &field_def.default {
                // Use default value if field is not provided
                let sql_value = match field_def.field_type.sql_type() {
                    "TEXT" => {
                        // For JSON or File fields, serialize the entire JSON value as a string
                        if matches!(field_def.field_type, FieldType::File(_))
                            || matches!(field_def.field_type, FieldType::Json)
                        {
                            SqlValue::Text(default.to_string())
                        } else {
                            SqlValue::Text(default.as_str().unwrap_or("").to_string())
                        }
                    }
                    "REAL" => SqlValue::Real(default.as_f64().unwrap_or(0.0)),
                    "INTEGER" => {
                        // Handle both boolean (stored as integer) and actual integers
                        if default.is_boolean() {
                            SqlValue::Integer(if default.as_bool().unwrap_or(false) { 1 } else { 0 })
                        } else {
                            SqlValue::Integer(default.as_i64().unwrap_or(0))
                        }
                    }
                    _ => SqlValue::Text(default.to_string()), // Fallback to text
                };
                sql_values.push((field_name.clone(), sql_value));
            }
        }
        
        Ok(sql_values)
    }

    /// Convert SQL row to Record for a specific collection
    fn sql_row_to_record(
        row: &rusqlite::Row,
        collection: &str,
        schema: &oxide_core::CollectionSchema,
    ) -> Result<Record, rusqlite::Error> {
        let mut data = serde_json::Map::new();
        
        // Extract schema fields from the row
        for (field_name, field_def) in &schema.fields {
            let value = match field_def.field_type.sql_type() {
                "TEXT" => {
                    if let Ok(text) = row.get::<_, Option<String>>(field_name.as_str()) {
                        text.map(|s| {
                            // For JSON fields and File fields stored as TEXT, try to parse as JSON
                            if matches!(field_def.field_type, FieldType::Json) ||
                               matches!(field_def.field_type, FieldType::File(_)) {
                                serde_json::from_str(&s).unwrap_or(JsonValue::String(s))
                            } else {
                                JsonValue::String(s)
                            }
                        }).unwrap_or(JsonValue::Null)
                    } else {
                        JsonValue::Null
                    }
                }
                "REAL" => {
                    if let Ok(num) = row.get::<_, Option<f64>>(field_name.as_str()) {
                        num.map(|n| JsonValue::Number(serde_json::Number::from_f64(n).unwrap_or_else(|| serde_json::Number::from(0))))
                            .unwrap_or(JsonValue::Null)
                    } else {
                        JsonValue::Null
                    }
                }
                "INTEGER" => {
                    if let Ok(int_val) = row.get::<_, Option<i64>>(field_name.as_str()) {
                        int_val.map(|i| {
                            // For boolean fields stored as INTEGER, convert back to boolean
                            if matches!(field_def.field_type, FieldType::Boolean) {
                                JsonValue::Bool(i != 0)
                            } else {
                                JsonValue::Number(serde_json::Number::from(i))
                            }
                        }).unwrap_or(JsonValue::Null)
                    } else {
                        JsonValue::Null
                    }
                }
                _ => JsonValue::Null, // Fallback
            };
            data.insert(field_name.clone(), value);
        }

        Ok(Record {
            id: row.get("id")?,
            collection: collection.to_string(),
            data: JsonValue::Object(data),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    }
}

/// Enum for SQL value types
#[derive(Debug, Clone)]
enum SqlValue {
    Text(String),
    Integer(i64),
    Real(f64),
}

impl SqlValue {
    fn bind_to_statement(&self, stmt: &mut rusqlite::Statement, index: usize) -> Result<(), rusqlite::Error> {
        match self {
            SqlValue::Text(s) => stmt.raw_bind_parameter(index, s),
            SqlValue::Integer(i) => stmt.raw_bind_parameter(index, *i),
            SqlValue::Real(r) => stmt.raw_bind_parameter(index, *r),
        }
    }
}

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
        
        // Get collection schema and validate data
        let schema = self.get_collection_schema(collection).await?;
        if let Err(validation_error) = schema.validate_data(&data) {
            return Err(AppError::validation("data", &validation_error));
        }

        let record_id = Self::generate_record_id();
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&schema.name);
        let table_name_for_debug = table_name.clone(); // Clone for debug use
        let collection = collection.to_string();
        let connection = self.connection.clone();

        // Convert data to SQL values
        let sql_values = self.record_data_to_sql_values(&data, &schema)?;

        let record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            // Build dynamic INSERT statement
            let mut field_names = vec!["id".to_string(), "created_at".to_string(), "updated_at".to_string()];
            let mut placeholders = vec!["?1".to_string(), "?2".to_string(), "?3".to_string()];
            
            for (field_name, _) in &sql_values {
                field_names.push(field_name.clone());
                placeholders.push(format!("?{}", field_names.len()));
            }

            let insert_sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                table_name,
                field_names.join(", "),
                placeholders.join(", ")
            );

            let mut stmt = conn.prepare(&insert_sql)
                .map_err(|e| AppError::database(format!("Failed to prepare insert statement: {}", e)))?;

            // Bind base values
            stmt.raw_bind_parameter(1, &record_id)
                .map_err(|e| AppError::database(format!("Failed to bind record_id: {}", e)))?;
            stmt.raw_bind_parameter(2, now)
                .map_err(|e| AppError::database(format!("Failed to bind created_at: {}", e)))?;
            stmt.raw_bind_parameter(3, now)
                .map_err(|e| AppError::database(format!("Failed to bind updated_at: {}", e)))?;

            // Bind field values
            for (index, (_, sql_value)) in sql_values.iter().enumerate() {
                sql_value.bind_to_statement(&mut stmt, index + 4)
                    .map_err(|e| AppError::database(format!("Failed to bind field value: {}", e)))?;
            }

            stmt.raw_execute()
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
        let request_context = oxide_core::event::RequestContext::authenticated("system".to_string()); // Default system context
        let after_context = oxide_core::event::AfterEventContext::record_created(
            record.collection.clone(),
            record.id.clone(),
            record.data.clone(),
            request_context,
        );
        self.event_bus
            .dispatch_after(AfterEventType::RecordCreated, &after_context)
            .await?;

        debug!(
            "Created record {} in collection table {}",
            record.id, table_name_for_debug
        );
        Ok(record)
    }

    async fn read_record(
        &self,
        collection: &str,
        record_id: &RecordId,
    ) -> Result<Record, AppError> {
        // Create a mutable context for BeforeRecordRead event using the factory method
        let mut context = BeforeEventContext::new_read(
            collection.to_string(),
            record_id.clone(),
            serde_json::json!({"record_id": record_id}),
        );

        // Dispatch BeforeRecordRead event
        self.event_bus
            .dispatch_before(BeforeEventType::RecordRead, &mut context)
            .await?;

        // Get collection schema
        let schema = self.get_collection_schema(collection).await?;
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&schema.name);
        let table_name_for_debug = table_name.clone(); // Clone for debug use

        let collection = collection.to_string(); // Clone collection for the closure
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        let record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let select_sql = format!("SELECT * FROM {} WHERE id = ?1", table_name);
            let mut stmt = conn
                .prepare(&select_sql)
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let record = stmt
                .query_row([&record_id], |row| Self::sql_row_to_record(row, &collection, &schema))
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
        let request_context = oxide_core::event::RequestContext::authenticated("system".to_string());
        // Note: There's no direct factory method for RecordRead, so we'll use the enum variant with all fields
        let after_context = oxide_core::event::AfterEventContext::RecordRead {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            collection: record.collection.clone(),
            record_id: record.id.clone(),
            data: record.data.clone(),
            request_context,
        };
        self.event_bus
            .dispatch_after(AfterEventType::RecordRead, &after_context)
            .await?;

        debug!(
            "Read record {} from collection table {}",
            record.id, table_name_for_debug
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

        // Get collection schema and validate data
        let schema = self.get_collection_schema(collection).await?;
        if let Err(validation_error) = schema.validate_data(&new_data) {
            return Err(AppError::validation("data", &validation_error));
        }

        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&schema.name);
        let table_name_for_debug = table_name.clone(); // Clone for debug use
        let collection = collection.to_string();
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        // Convert data to SQL values
        let sql_values = self.record_data_to_sql_values(&new_data, &schema)?;

        let updated_record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            // Build dynamic UPDATE statement
            let mut set_clauses = vec!["updated_at = ?1".to_string()];
            for (index, (field_name, _)) in sql_values.iter().enumerate() {
                set_clauses.push(format!("{} = ?{}", field_name, index + 2));
            }

            let update_sql = format!(
                "UPDATE {} SET {} WHERE id = ?{}",
                table_name,
                set_clauses.join(", "),
                sql_values.len() + 2
            );

            let mut stmt = conn.prepare(&update_sql)
                .map_err(|e| AppError::database(format!("Failed to prepare update statement: {}", e)))?;

            // Bind updated_at
            stmt.raw_bind_parameter(1, now)
                .map_err(|e| AppError::database(format!("Failed to bind updated_at: {}", e)))?;

            // Bind field values
            for (index, (_, sql_value)) in sql_values.iter().enumerate() {
                sql_value.bind_to_statement(&mut stmt, index + 2)
                    .map_err(|e| AppError::database(format!("Failed to bind field value: {}", e)))?;
            }

            // Bind record_id for WHERE clause
            stmt.raw_bind_parameter(sql_values.len() + 2, &record_id)
                .map_err(|e| AppError::database(format!("Failed to bind record_id: {}", e)))?;

            let rows_affected = stmt.raw_execute()
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
        let request_context = oxide_core::event::RequestContext::authenticated("system".to_string());
        let after_context = oxide_core::event::AfterEventContext::record_updated(
            updated_record.collection.clone(),
            updated_record.id.clone(),
            old_record.data,
            updated_record.data.clone(),
            request_context,
        );
        self.event_bus
            .dispatch_after(AfterEventType::RecordUpdated, &after_context)
            .await?;

        debug!(
            "Updated record {} in collection table {}",
            updated_record.id, table_name_for_debug
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

        // Get collection schema
        let schema = self.get_collection_schema(collection).await?;
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&schema.name);
        let table_name_for_debug = table_name.clone(); // Clone for debug use

        let record_id = record_id.clone();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let delete_sql = format!("DELETE FROM {} WHERE id = ?1", table_name);
            let rows_affected = conn
                .execute(&delete_sql, [&record_id])
                .map_err(|e| AppError::database(format!("Failed to delete record: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("record", &record_id));
            }

            Ok(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordDelete event
        let request_context = oxide_core::event::RequestContext::authenticated("system".to_string());
        let after_context = oxide_core::event::AfterEventContext::record_deleted(
            record.collection.clone(),
            record.id.clone(),
            record.data.clone(),
            request_context,
        );
        self.event_bus
            .dispatch_after(AfterEventType::RecordDeleted, &after_context)
            .await?;

        debug!(
            "Deleted record {} from collection table {}",
            record.id, table_name_for_debug
        );
        Ok(record)
    }

    async fn list_records(
        &self,
        collection: &str,
        params: ListParams,
    ) -> Result<Vec<Record>, AppError> {
        // Get collection schema
        let schema = self.get_collection_schema(collection).await?;
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&schema.name);
        let table_name_for_debug = table_name.clone(); // Clone for debug use

        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let records = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut query = format!("SELECT * FROM {}", table_name);
            let mut bind_params: Vec<String> = vec![];

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
                    |row| Self::sql_row_to_record(row, &collection_name, &schema),
                )
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            records.map_err(|e| AppError::database(format!("Failed to query records: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Listed {} records from collection table {}", records.len(), table_name_for_debug);
        Ok(records)
    }

    async fn create_collection(&self, schema: oxide_core::CollectionSchema) -> Result<(), AppError> {
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

    async fn list_collections(&self) -> Result<Vec<oxide_core::CollectionSchema>, AppError> {
        SqliteDb::list_collections(self).await
    }

    async fn collection_exists(&self, collection: &str) -> Result<bool, AppError> {
        SqliteDb::collection_exists(self, collection).await
    }

    async fn count_records(&self, collection: &str) -> Result<usize, AppError> {
        SqliteDb::count_records(self, collection).await
    }

    async fn get_collection_size_kb(&self, collection: &str) -> Result<f64, AppError> {
        SqliteDb::get_collection_size_kb(self, collection).await
    }

    async fn close(&self) -> Result<(), AppError> {
        SqliteDb::close(self).await
    }

    async fn health_check(&self) -> Result<(), AppError> {
        SqliteDb::health_check(self).await
    }

    // Permission storage methods

    /// Store permissions for a collection
    async fn store_permissions(&self, permissions: &oxide_core::CollectionPermissions) -> Result<(), AppError> {
        use oxide_core::auth::PermissionService;
        <Self as PermissionService>::store_permissions(self, permissions).await
    }

    /// Get permissions for a collection
    async fn get_permissions(&self, collection: &str) -> Result<Option<oxide_core::CollectionPermissions>, AppError> {
        use oxide_core::auth::PermissionService;
        <Self as PermissionService>::get_permissions(self, collection).await
    }

    /// Delete permissions for a collection (revert to defaults)
    async fn delete_permissions(&self, collection: &str) -> Result<(), AppError> {
        use oxide_core::auth::PermissionService;
        <Self as PermissionService>::delete_permissions(self, collection).await
    }

    /// List all collections that have custom permissions
    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError> {
        use oxide_core::auth::PermissionService;
        <Self as PermissionService>::list_collections_with_permissions(self).await
    }

    async fn authenticate_user(&self, auth_request: crate::db::AuthRequest, auth_config: &oxide_core::auth::AuthCollectionConfig) -> Result<crate::db::AuthResponse, AppError> {
        SqliteDb::authenticate_user(self, auth_request, auth_config).await
    }

    async fn register_user(&self, register_request: crate::db::RegisterRequest, auth_config: &oxide_core::auth::AuthCollectionConfig) -> Result<String, AppError> {
        SqliteDb::register_user(self, register_request, auth_config).await
    }

    async fn find_user_by_identifier(&self, collection: &str, identifier_field: &str, identifier_value: &str) -> Result<crate::Record, AppError> {
        SqliteDb::find_user_by_identifier(self, collection, identifier_field, identifier_value).await
    }

    async fn list_auth_collections(&self) -> Result<Vec<oxide_core::CollectionSchema>, AppError> {
        SqliteDb::list_auth_collections(self).await
    }

    async fn populate_relationships(&self, collection: &str, records: &mut [Record]) -> Result<(), AppError> {
        SqliteDb::populate_relationships(self, collection, records).await
    }

    async fn populate_specific_relationships(&self, collection: &str, records: &mut [Record], field_names: &[String]) -> Result<(), AppError> {
        SqliteDb::populate_specific_relationships(self, collection, records, field_names).await
    }

    async fn get_related_records(
        &self, 
        target_collection: &str, 
        record_ids: &[String],
        display_field: Option<&str>
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, AppError> {
        SqliteDb::get_related_records(self, target_collection, record_ids, display_field).await
    }
} 