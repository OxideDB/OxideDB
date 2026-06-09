//! CRUD operations for SQLite database

use super::connection::SqliteDb;
use crate::{
    db::{Db, FilterOp, ListParams, SchemaAdapter},
    Record,
};
use oxide_core::{
    event::types::{RecordData, RecordId},
    AfterEventType, AppError, BeforeEventContext, BeforeEventType, CollectionSchema, FieldType,
};
use rusqlite::{
    params_from_iter,
    types::{ToSqlOutput, Value as RusqliteValue},
    ToSql,
};
use serde_json::Value as JsonValue;
use tokio::task::spawn_blocking;
use tracing::debug;

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
                            SqlValue::Integer(if value.as_bool().unwrap_or(false) {
                                1
                            } else {
                                0
                            })
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
                            SqlValue::Integer(if default.as_bool().unwrap_or(false) {
                                1
                            } else {
                                0
                            })
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
                            if matches!(field_def.field_type, FieldType::Json)
                                || matches!(field_def.field_type, FieldType::File(_))
                            {
                                serde_json::from_str(&s).unwrap_or(JsonValue::String(s))
                            } else {
                                JsonValue::String(s)
                            }
                        })
                        .unwrap_or(JsonValue::Null)
                    } else {
                        JsonValue::Null
                    }
                }
                "REAL" => {
                    if let Ok(num) = row.get::<_, Option<f64>>(field_name.as_str()) {
                        num.map(|n| {
                            JsonValue::Number(
                                serde_json::Number::from_f64(n)
                                    .unwrap_or_else(|| serde_json::Number::from(0)),
                            )
                        })
                        .unwrap_or(JsonValue::Null)
                    } else {
                        JsonValue::Null
                    }
                }
                "INTEGER" => {
                    if let Ok(int_val) = row.get::<_, Option<i64>>(field_name.as_str()) {
                        int_val
                            .map(|i| {
                                // For boolean fields stored as INTEGER, convert back to boolean
                                if matches!(field_def.field_type, FieldType::Boolean) {
                                    JsonValue::Bool(i != 0)
                                } else {
                                    JsonValue::Number(serde_json::Number::from(i))
                                }
                            })
                            .unwrap_or(JsonValue::Null)
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
    Null,
    Text(String),
    Integer(i64),
    Real(f64),
}

impl SqlValue {
    fn bind_to_statement(
        &self,
        stmt: &mut rusqlite::Statement,
        index: usize,
    ) -> Result<(), rusqlite::Error> {
        match self {
            SqlValue::Null => stmt.raw_bind_parameter(index, rusqlite::types::Null),
            SqlValue::Text(s) => stmt.raw_bind_parameter(index, s),
            SqlValue::Integer(i) => stmt.raw_bind_parameter(index, *i),
            SqlValue::Real(r) => stmt.raw_bind_parameter(index, *r),
        }
    }
}

impl ToSql for SqlValue {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            SqlValue::Null => ToSqlOutput::Owned(RusqliteValue::Null),
            SqlValue::Text(value) => ToSqlOutput::Owned(RusqliteValue::Text(value.clone())),
            SqlValue::Integer(value) => ToSqlOutput::Owned(RusqliteValue::Integer(*value)),
            SqlValue::Real(value) => ToSqlOutput::Owned(RusqliteValue::Real(*value)),
        })
    }
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn resolve_record_column(
    schema: &CollectionSchema,
    requested_field: &str,
) -> Result<String, AppError> {
    match requested_field {
        "id" | "created_at" | "updated_at" => Ok(requested_field.to_string()),
        field if schema.fields.contains_key(field) => Ok(field.to_string()),
        field => Err(AppError::validation(
            "field".to_string(),
            format!("Unknown record field '{}'", field),
        )),
    }
}

fn required_filter_value(params: &ListParams, filter_op: FilterOp) -> Result<&str, AppError> {
    params.filter_value.as_deref().ok_or_else(|| {
        AppError::validation(
            "filter_value".to_string(),
            format!("filter_value is required for {:?} filters", filter_op),
        )
    })
}

fn filter_value_to_sql(raw: &str) -> SqlValue {
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Null) => SqlValue::Null,
        Ok(serde_json::Value::Bool(value)) => SqlValue::Integer(i64::from(value)),
        Ok(serde_json::Value::Number(value)) => {
            if let Some(value) = value.as_i64() {
                SqlValue::Integer(value)
            } else if let Some(value) = value.as_u64() {
                if value <= i64::MAX as u64 {
                    SqlValue::Integer(value as i64)
                } else {
                    SqlValue::Real(value as f64)
                }
            } else {
                SqlValue::Real(value.as_f64().unwrap_or_default())
            }
        }
        Ok(serde_json::Value::String(value)) => SqlValue::Text(value),
        Ok(_) | Err(_) => SqlValue::Text(raw.to_string()),
    }
}

fn escape_like(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for character in raw.chars() {
        match character {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

fn is_searchable_field(field_type: &FieldType) -> bool {
    matches!(
        field_type,
        FieldType::Text
            | FieldType::Email
            | FieldType::Url
            | FieldType::Phone
            | FieldType::Password
            | FieldType::Json
            | FieldType::Relationship(_)
            | FieldType::File(_)
            | FieldType::Select(_)
    )
}

fn append_filter_clauses(
    query: &mut String,
    schema: &CollectionSchema,
    params: &ListParams,
    bind_params: &mut Vec<SqlValue>,
) -> Result<(), AppError> {
    let mut clauses = Vec::new();

    if let Some(search) = params
        .search
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let mut search_clauses = vec![format!("{} LIKE ? ESCAPE '\\'", quote_identifier("id"))];
        bind_params.push(SqlValue::Text(format!("%{}%", escape_like(search))));

        for (field_name, field_def) in &schema.fields {
            if is_searchable_field(&field_def.field_type) {
                search_clauses.push(format!(
                    "CAST({} AS TEXT) LIKE ? ESCAPE '\\'",
                    quote_identifier(field_name)
                ));
                bind_params.push(SqlValue::Text(format!("%{}%", escape_like(search))));
            }
        }

        clauses.push(format!("({})", search_clauses.join(" OR ")));
    }

    match params
        .filter_field
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(filter_field) => {
            let column = quote_identifier(&resolve_record_column(schema, filter_field)?);
            let filter_op = params.filter_op.unwrap_or_default();

            let clause = match filter_op {
                FilterOp::Exists => format!("{} IS NOT NULL", column),
                FilterOp::NotExists => format!("{} IS NULL", column),
                FilterOp::Contains => {
                    let value = required_filter_value(params, filter_op)?;
                    bind_params.push(SqlValue::Text(format!("%{}%", escape_like(value))));
                    format!("CAST({} AS TEXT) LIKE ? ESCAPE '\\'", column)
                }
                FilterOp::Eq | FilterOp::Ne => {
                    let value = filter_value_to_sql(required_filter_value(params, filter_op)?);
                    match (filter_op, value) {
                        (FilterOp::Eq, SqlValue::Null) => format!("{} IS NULL", column),
                        (FilterOp::Ne, SqlValue::Null) => format!("{} IS NOT NULL", column),
                        (FilterOp::Eq, value) => {
                            bind_params.push(value);
                            format!("{} = ?", column)
                        }
                        (FilterOp::Ne, value) => {
                            bind_params.push(value);
                            format!("{} <> ?", column)
                        }
                        _ => unreachable!(),
                    }
                }
                FilterOp::Gt | FilterOp::Gte | FilterOp::Lt | FilterOp::Lte => {
                    let value = filter_value_to_sql(required_filter_value(params, filter_op)?);
                    if matches!(value, SqlValue::Null) {
                        return Err(AppError::validation(
                            "filter_value".to_string(),
                            "null cannot be used with comparison filters".to_string(),
                        ));
                    }

                    bind_params.push(value);
                    let operator = match filter_op {
                        FilterOp::Gt => ">",
                        FilterOp::Gte => ">=",
                        FilterOp::Lt => "<",
                        FilterOp::Lte => "<=",
                        _ => unreachable!(),
                    };
                    format!("{} {} ?", column, operator)
                }
            };

            clauses.push(clause);
        }
        None if params.filter_op.is_some() || params.filter_value.is_some() => {
            return Err(AppError::validation(
                "filter_field".to_string(),
                "filter_field is required when filter_op or filter_value is provided".to_string(),
            ));
        }
        None => {}
    }

    if !clauses.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&clauses.join(" AND "));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::{CollectionType, FieldDefinition};

    fn test_schema() -> CollectionSchema {
        let mut schema = CollectionSchema::new("articles".to_string(), CollectionType::Base);
        schema.add_field("title".to_string(), FieldDefinition::new(FieldType::Text));
        schema.add_field("views".to_string(), FieldDefinition::new(FieldType::Number));
        schema.add_field(
            "published".to_string(),
            FieldDefinition::new(FieldType::Boolean),
        );
        schema
    }

    #[test]
    fn appends_search_and_filter_clauses_with_bound_values() {
        let mut query = "SELECT * FROM collection_articles".to_string();
        let mut bind_params = Vec::new();
        let params = ListParams {
            search: Some("rust_%".to_string()),
            filter_field: Some("views".to_string()),
            filter_op: Some(FilterOp::Gte),
            filter_value: Some("10".to_string()),
            ..Default::default()
        };

        append_filter_clauses(&mut query, &test_schema(), &params, &mut bind_params).unwrap();

        assert!(query.contains("\"id\" LIKE ? ESCAPE '\\'"));
        assert!(query.contains("\"views\" >= ?"));
        assert_eq!(bind_params.len(), 3);
        assert!(matches!(bind_params.last(), Some(SqlValue::Integer(10))));
    }

    #[test]
    fn rejects_unknown_filter_fields() {
        let mut query = "SELECT * FROM collection_articles".to_string();
        let mut bind_params = Vec::new();
        let params = ListParams {
            filter_field: Some("title; DROP TABLE articles".to_string()),
            filter_value: Some("x".to_string()),
            ..Default::default()
        };

        assert!(
            append_filter_clauses(&mut query, &test_schema(), &params, &mut bind_params).is_err()
        );
    }

    #[test]
    fn supports_null_equality_without_binding_null_comparison() {
        let mut query = "SELECT * FROM collection_articles".to_string();
        let mut bind_params = Vec::new();
        let params = ListParams {
            filter_field: Some("published".to_string()),
            filter_value: Some("null".to_string()),
            ..Default::default()
        };

        append_filter_clauses(&mut query, &test_schema(), &params, &mut bind_params).unwrap();

        assert!(query.contains("\"published\" IS NULL"));
        assert!(bind_params.is_empty());
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
            let mut field_names = vec![
                "id".to_string(),
                "created_at".to_string(),
                "updated_at".to_string(),
            ];
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

            let mut stmt = conn.prepare(&insert_sql).map_err(|e| {
                AppError::database(format!("Failed to prepare insert statement: {}", e))
            })?;

            // Bind base values
            stmt.raw_bind_parameter(1, &record_id)
                .map_err(|e| AppError::database(format!("Failed to bind record_id: {}", e)))?;
            stmt.raw_bind_parameter(2, now)
                .map_err(|e| AppError::database(format!("Failed to bind created_at: {}", e)))?;
            stmt.raw_bind_parameter(3, now)
                .map_err(|e| AppError::database(format!("Failed to bind updated_at: {}", e)))?;

            // Bind field values
            for (index, (_, sql_value)) in sql_values.iter().enumerate() {
                sql_value
                    .bind_to_statement(&mut stmt, index + 4)
                    .map_err(|e| {
                        AppError::database(format!("Failed to bind field value: {}", e))
                    })?;
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
        let request_context =
            oxide_core::event::RequestContext::authenticated("system".to_string()); // Default system context
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
                .query_row([&record_id], |row| {
                    Self::sql_row_to_record(row, &collection, &schema)
                })
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
        let request_context =
            oxide_core::event::RequestContext::authenticated("system".to_string());
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

            let mut stmt = conn.prepare(&update_sql).map_err(|e| {
                AppError::database(format!("Failed to prepare update statement: {}", e))
            })?;

            // Bind updated_at
            stmt.raw_bind_parameter(1, now)
                .map_err(|e| AppError::database(format!("Failed to bind updated_at: {}", e)))?;

            // Bind field values
            for (index, (_, sql_value)) in sql_values.iter().enumerate() {
                sql_value
                    .bind_to_statement(&mut stmt, index + 2)
                    .map_err(|e| {
                        AppError::database(format!("Failed to bind field value: {}", e))
                    })?;
            }

            // Bind record_id for WHERE clause
            stmt.raw_bind_parameter(sql_values.len() + 2, &record_id)
                .map_err(|e| AppError::database(format!("Failed to bind record_id: {}", e)))?;

            let rows_affected = stmt
                .raw_execute()
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
        let request_context =
            oxide_core::event::RequestContext::authenticated("system".to_string());
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
        let request_context =
            oxide_core::event::RequestContext::authenticated("system".to_string());
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

            let mut query = format!("SELECT * FROM {}", quote_identifier(&table_name));
            let mut bind_params: Vec<SqlValue> = vec![];

            append_filter_clauses(&mut query, &schema, &params, &mut bind_params)?;

            // Add sorting
            if let Some(sort_field) = &params.sort_field {
                let sort_column = quote_identifier(&resolve_record_column(&schema, sort_field)?);
                let direction = if params.sort_ascending.unwrap_or(true) {
                    "ASC"
                } else {
                    "DESC"
                };
                query.push_str(&format!(" ORDER BY {} {}", sort_column, direction));
            } else {
                query.push_str(&format!(" ORDER BY {} ASC", quote_identifier("created_at")));
            }

            // Add limit and offset
            if let Some(limit) = params.limit {
                query.push_str(" LIMIT ?");
                bind_params.push(SqlValue::Integer(limit as i64));
            }

            if let Some(offset) = params.offset {
                query.push_str(" OFFSET ?");
                bind_params.push(SqlValue::Integer(offset as i64));
            }

            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let records: Result<Vec<Record>, rusqlite::Error> = stmt
                .query_map(params_from_iter(bind_params.iter()), |row| {
                    Self::sql_row_to_record(row, &collection_name, &schema)
                })
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            records.map_err(|e| AppError::database(format!("Failed to query records: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!(
            "Listed {} records from collection table {}",
            records.len(),
            table_name_for_debug
        );
        Ok(records)
    }

    async fn create_collection(
        &self,
        schema: oxide_core::CollectionSchema,
    ) -> Result<(), AppError> {
        SqliteDb::create_collection_with_schema(self, schema).await
    }

    async fn get_collection_schema(
        &self,
        collection: &str,
    ) -> Result<oxide_core::CollectionSchema, AppError> {
        SqliteDb::get_collection_schema(self, collection).await
    }

    async fn update_collection_schema(
        &self,
        collection: &str,
        schema: oxide_core::CollectionSchema,
    ) -> Result<(), AppError> {
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

    async fn count_records_with_params(
        &self,
        collection: &str,
        params: ListParams,
    ) -> Result<usize, AppError> {
        // Get collection schema
        let schema = self.get_collection_schema(collection).await?;
        let schema_adapter = super::schema_adapter::SqliteSchemaAdapter::new();
        let table_name = schema_adapter.get_table_name(&schema.name);
        let table_name_for_debug = table_name.clone();
        let connection = self.connection.clone();

        let count = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut query = format!("SELECT COUNT(*) FROM {}", quote_identifier(&table_name));
            let mut bind_params = Vec::new();
            append_filter_clauses(&mut query, &schema, &params, &mut bind_params)?;

            let mut stmt = conn.prepare(&query).map_err(|e| {
                AppError::database(format!("Failed to prepare filtered count statement: {}", e))
            })?;

            let count: i64 = stmt
                .query_row(params_from_iter(bind_params.iter()), |row| row.get(0))
                .map_err(|e| {
                    AppError::database(format!("Failed to count filtered records: {}", e))
                })?;

            Ok::<usize, AppError>(count as usize)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!(
            "Counted {} filtered records in collection table {}",
            count, table_name_for_debug
        );
        Ok(count)
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
    async fn store_permissions(
        &self,
        permissions: &oxide_core::CollectionPermissions,
    ) -> Result<(), AppError> {
        use oxide_core::auth::PermissionService;
        <Self as PermissionService>::store_permissions(self, permissions).await
    }

    /// Get permissions for a collection
    async fn get_permissions(
        &self,
        collection: &str,
    ) -> Result<Option<oxide_core::CollectionPermissions>, AppError> {
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

    async fn authenticate_user(
        &self,
        auth_request: crate::db::AuthRequest,
        auth_config: &oxide_core::auth::AuthCollectionConfig,
    ) -> Result<crate::db::AuthResponse, AppError> {
        SqliteDb::authenticate_user(self, auth_request, auth_config).await
    }

    async fn register_user(
        &self,
        register_request: crate::db::RegisterRequest,
        auth_config: &oxide_core::auth::AuthCollectionConfig,
    ) -> Result<String, AppError> {
        SqliteDb::register_user(self, register_request, auth_config).await
    }

    async fn find_user_by_identifier(
        &self,
        collection: &str,
        identifier_field: &str,
        identifier_value: &str,
    ) -> Result<crate::Record, AppError> {
        SqliteDb::find_user_by_identifier(self, collection, identifier_field, identifier_value)
            .await
    }

    async fn list_auth_collections(&self) -> Result<Vec<oxide_core::CollectionSchema>, AppError> {
        SqliteDb::list_auth_collections(self).await
    }

    async fn populate_relationships(
        &self,
        collection: &str,
        records: &mut [Record],
    ) -> Result<(), AppError> {
        SqliteDb::populate_relationships(self, collection, records).await
    }

    async fn populate_specific_relationships(
        &self,
        collection: &str,
        records: &mut [Record],
        field_names: &[String],
    ) -> Result<(), AppError> {
        SqliteDb::populate_specific_relationships(self, collection, records, field_names).await
    }

    async fn get_related_records(
        &self,
        target_collection: &str,
        record_ids: &[String],
        display_field: Option<&str>,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, AppError> {
        SqliteDb::get_related_records(self, target_collection, record_ids, display_field).await
    }

    /// Get comprehensive dashboard statistics
    async fn get_dashboard_statistics(&self) -> Result<oxide_core::DashboardStats, AppError> {
        SqliteDb::get_dashboard_statistics(self).await
    }

    /// Get basic system statistics
    async fn get_system_statistics(&self) -> Result<oxide_core::SystemStats, AppError> {
        SqliteDb::get_system_statistics(self).await
    }

    /// Get statistics for all collections
    async fn get_collection_statistics(
        &self,
    ) -> Result<Vec<oxide_core::CollectionStatsEntry>, AppError> {
        SqliteDb::get_collection_statistics(self).await
    }

    /// Get storage usage information
    async fn get_storage_usage(&self) -> Result<oxide_core::StorageUsage, AppError> {
        SqliteDb::get_storage_usage(self).await
    }

    /// Record an activity entry for the dashboard
    async fn record_dashboard_activity(
        &self,
        activity: oxide_core::ActivityEntry,
    ) -> Result<(), AppError> {
        SqliteDb::record_dashboard_activity(self, activity).await
    }

    /// Get recent activities for the dashboard
    async fn get_recent_dashboard_activities(
        &self,
        limit: usize,
    ) -> Result<Vec<oxide_core::ActivityEntry>, AppError> {
        SqliteDb::get_recent_dashboard_activities(self, limit).await
    }
}
