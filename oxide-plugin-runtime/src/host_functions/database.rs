//! Database-related Host Functions
//!
//! These functions provide database access capabilities for plugins to perform
//! CRUD operations on collections within the OxideDB system.

use crate::host_state::{lock_host_state, record_host_call, PluginStoreData};
use crate::utils::{
    allocate_plugin_memory_and_copy, create_error_response, create_success_response,
    read_string_from_plugin_memory,
};
use oxide_core::auth::CrudOperation;
use oxide_core::collection::CollectionSchema;
use oxide_core::event::types::{RecordData, RecordId};
use oxide_core::plugin_api::{host_functions, PluginError};
use oxide_core::plugin_security::CollectionOperation;
use oxide_db::{db::ListParams, Db};
use serde::Serialize;
use std::sync::Arc;
use tracing::{debug, error, info};
use wasmtime::{Caller, Linker};

/// Define database-related host functions in the linker.
///
/// This includes:
/// - `create_record(collection_ptr, collection_len, data_ptr, data_len)`: Create a new record
/// - `read_records(collection_ptr, collection_len, filter_ptr, filter_len)`: Read records with filtering
/// - `update_record(collection_ptr, collection_len, record_id_ptr, record_id_len, data_ptr, data_len)`: Update a record
/// - `delete_record(collection_ptr, collection_len, record_id_ptr, record_id_len)`: Delete a record
/// - `create_collection(schema_ptr, schema_len)`: Create a collection
/// - `list_collections()`: List collection schemas visible to the plugin
/// - `get_collection_schema(collection_ptr, collection_len)`: Read a collection schema
/// - `update_collection_schema(collection_ptr, collection_len, schema_ptr, schema_len)`: Update a collection schema
/// - `delete_collection(collection_ptr, collection_len)`: Delete a collection
/// - `collection_exists(collection_ptr, collection_len)`: Check if a collection exists
/// - `get_collection_stats(collection_ptr, collection_len)`: Read collection statistics
pub fn define_database_functions(
    linker: &mut Linker<PluginStoreData>,
    database: Arc<dyn Db>,
) -> Result<(), PluginError> {
    // create_record(collection_ptr: *const u8, collection_len: usize, data_ptr: *const u8, data_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::CREATE_RECORD,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32,
                      data_ptr: i32,
                      data_len: i32|
                      -> i32 {
                    handle_create_record(
                        &mut caller,
                        &db,
                        collection_ptr,
                        collection_len,
                        data_ptr,
                        data_len,
                    )
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define create_record: {}", e))
            })?;
    }

    // read_records(collection_ptr: *const u8, collection_len: usize, filter_ptr: *const u8, filter_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::READ_RECORDS,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32,
                      filter_ptr: i32,
                      filter_len: i32|
                      -> i32 {
                    handle_read_records(
                        &mut caller,
                        &db,
                        collection_ptr,
                        collection_len,
                        filter_ptr,
                        filter_len,
                    )
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define read_records: {}", e))
            })?;
    }

    // update_record(collection_ptr: *const u8, collection_len: usize, record_id_ptr: *const u8, record_id_len: usize, data_ptr: *const u8, data_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::UPDATE_RECORD,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32,
                      record_id_ptr: i32,
                      record_id_len: i32,
                      data_ptr: i32,
                      data_len: i32|
                      -> i32 {
                    handle_update_record(
                        &mut caller,
                        &db,
                        collection_ptr,
                        collection_len,
                        record_id_ptr,
                        record_id_len,
                        data_ptr,
                        data_len,
                    )
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define update_record: {}", e))
            })?;
    }

    // delete_record(collection_ptr: *const u8, collection_len: usize, record_id_ptr: *const u8, record_id_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::DELETE_RECORD,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32,
                      record_id_ptr: i32,
                      record_id_len: i32|
                      -> i32 {
                    handle_delete_record(
                        &mut caller,
                        &db,
                        collection_ptr,
                        collection_len,
                        record_id_ptr,
                        record_id_len,
                    )
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define delete_record: {}", e))
            })?;
    }

    // create_collection(schema_ptr: *const u8, schema_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::CREATE_COLLECTION,
                move |mut caller: Caller<'_, PluginStoreData>,
                      schema_ptr: i32,
                      schema_len: i32|
                      -> i32 {
                    handle_create_collection(&mut caller, &db, schema_ptr, schema_len)
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define create_collection: {}",
                    e
                ))
            })?;
    }

    // list_collections() -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::LIST_COLLECTIONS,
                move |mut caller: Caller<'_, PluginStoreData>| -> i32 {
                    handle_list_collections(&mut caller, &db)
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define list_collections: {}",
                    e
                ))
            })?;
    }

    // get_collection_schema(collection_ptr: *const u8, collection_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::GET_COLLECTION_SCHEMA,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32|
                      -> i32 {
                    handle_get_collection_schema(&mut caller, &db, collection_ptr, collection_len)
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define get_collection_schema: {}",
                    e
                ))
            })?;
    }

    // update_collection_schema(collection_ptr: *const u8, collection_len: usize, schema_ptr: *const u8, schema_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::UPDATE_COLLECTION_SCHEMA,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32,
                      schema_ptr: i32,
                      schema_len: i32|
                      -> i32 {
                    handle_update_collection_schema(
                        &mut caller,
                        &db,
                        collection_ptr,
                        collection_len,
                        schema_ptr,
                        schema_len,
                    )
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define update_collection_schema: {}",
                    e
                ))
            })?;
    }

    // delete_collection(collection_ptr: *const u8, collection_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::DELETE_COLLECTION,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32|
                      -> i32 {
                    handle_delete_collection(&mut caller, &db, collection_ptr, collection_len)
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define delete_collection: {}",
                    e
                ))
            })?;
    }

    // collection_exists(collection_ptr: *const u8, collection_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::COLLECTION_EXISTS,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32|
                      -> i32 {
                    handle_collection_exists(&mut caller, &db, collection_ptr, collection_len)
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define collection_exists: {}",
                    e
                ))
            })?;
    }

    // get_collection_stats(collection_ptr: *const u8, collection_len: usize) -> i32
    {
        let db = database;
        linker
            .func_wrap(
                "env",
                host_functions::GET_COLLECTION_STATS,
                move |mut caller: Caller<'_, PluginStoreData>,
                      collection_ptr: i32,
                      collection_len: i32|
                      -> i32 {
                    handle_get_collection_stats(&mut caller, &db, collection_ptr, collection_len)
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define get_collection_stats: {}",
                    e
                ))
            })?;
    }

    Ok(())
}

fn write_error_result(caller: &mut Caller<'_, PluginStoreData>, message: &str) {
    let result_bytes = create_error_response(message);
    let _ = write_bytes_result(caller, &result_bytes, "storing database error result");
}

fn write_success_result(
    caller: &mut Caller<'_, PluginStoreData>,
    data: serde_json::Value,
    action: &str,
) -> i32 {
    let result_bytes = create_success_response(data);
    if write_bytes_result(caller, &result_bytes, action) {
        0
    } else {
        error!("Failed to allocate plugin memory while {}", action);
        -1
    }
}

fn write_serialized_success<T: Serialize>(
    caller: &mut Caller<'_, PluginStoreData>,
    data: &T,
    action: &str,
) -> i32 {
    let data = match serde_json::to_value(data) {
        Ok(value) => value,
        Err(e) => {
            let message = format!("Failed to serialize database result: {}", e);
            error!("{}", message);
            write_error_result(caller, &message);
            return -1;
        }
    };

    write_success_result(caller, data, action)
}

#[derive(Serialize)]
struct CollectionExistsResponse {
    collection: String,
    exists: bool,
}

#[derive(Serialize)]
struct CollectionStatsResponse {
    name: String,
    record_count: usize,
    exists: bool,
    size_kb: f64,
    schema_version: Option<u32>,
    created_at: Option<i64>,
    updated_at: Option<i64>,
}

impl CollectionStatsResponse {
    fn missing(collection: String) -> Self {
        Self {
            name: collection,
            record_count: 0,
            exists: false,
            size_kb: 0.0,
            schema_version: None,
            created_at: None,
            updated_at: None,
        }
    }
}

fn write_bytes_result(
    caller: &mut Caller<'_, PluginStoreData>,
    result_bytes: &[u8],
    action: &str,
) -> bool {
    if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, result_bytes) {
        return write_result_buffer(caller, ptr, len, action);
    }

    false
}

fn write_result_buffer(
    caller: &mut Caller<'_, PluginStoreData>,
    ptr: i32,
    len: i32,
    action: &str,
) -> bool {
    let Ok(ptr) = u32::try_from(ptr) else {
        error!("Plugin returned negative result pointer while {}", action);
        return false;
    };
    let Ok(len) = u32::try_from(len) else {
        error!("Plugin returned negative result length while {}", action);
        return false;
    };

    if let Some(mut state) = lock_host_state(caller.data(), action) {
        state.result_buffer = [ptr.to_le_bytes().to_vec(), len.to_le_bytes().to_vec()].concat();
        return true;
    }

    false
}

fn clear_result_buffer(caller: &mut Caller<'_, PluginStoreData>) -> bool {
    let Some(mut state) = lock_host_state(caller.data(), "clearing database result buffer") else {
        return false;
    };
    state.result_buffer.clear();
    true
}

fn exit_database_operation(caller: &mut Caller<'_, PluginStoreData>) {
    if let Some(mut state) = lock_host_state(caller.data(), "exiting database operation") {
        state.exit_database_operation();
    }
}

fn enter_database_operation(caller: &mut Caller<'_, PluginStoreData>) -> bool {
    let already_in_operation = {
        let Some(mut state) = lock_host_state(caller.data(), "entering database operation") else {
            write_error_result(caller, "Plugin host state not available");
            return false;
        };

        if state.in_database_operation {
            true
        } else {
            state.enter_database_operation();
            false
        }
    };

    if already_in_operation {
        write_error_result(caller, "Nested database operations are not allowed");
        return false;
    }

    true
}

fn ensure_database_capability(
    caller: &mut Caller<'_, PluginStoreData>,
    operation: CrudOperation,
    collection: &str,
) -> bool {
    let (allowed, plugin_name) = {
        let Some(state) = lock_host_state(caller.data(), "checking database capability") else {
            write_error_result(caller, "Plugin host state not available");
            return false;
        };
        (
            state.current_plugin_can_access_collection(&operation, collection),
            state
                .current_plugin
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        )
    };

    if allowed {
        return true;
    }

    let message = format!(
        "Plugin '{}' lacks {:?} access to collection '{}'",
        plugin_name, operation, collection
    );
    error!("{}", message);
    write_error_result(caller, &message);
    false
}

fn ensure_collection_capability(
    caller: &mut Caller<'_, PluginStoreData>,
    operation: CollectionOperation,
    collection: &str,
) -> bool {
    let (allowed, plugin_name) = {
        let Some(state) = lock_host_state(caller.data(), "checking collection capability") else {
            write_error_result(caller, "Plugin host state not available");
            return false;
        };
        (
            state.current_plugin_can_manage_collection(&operation, collection),
            state
                .current_plugin
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        )
    };

    if allowed {
        return true;
    }

    let message = format!(
        "Plugin '{}' lacks {:?} collection-management access to '{}'",
        plugin_name, operation, collection
    );
    error!("{}", message);
    write_error_result(caller, &message);
    false
}

fn ensure_any_collection_capability(
    caller: &mut Caller<'_, PluginStoreData>,
    operation: CollectionOperation,
) -> bool {
    let (allowed, plugin_name) = {
        let Some(state) = lock_host_state(
            caller.data(),
            "checking collection management operation capability",
        ) else {
            write_error_result(caller, "Plugin host state not available");
            return false;
        };
        (
            state.current_plugin_has_collection_management_operation(&operation),
            state
                .current_plugin
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        )
    };

    if allowed {
        return true;
    }

    let message = format!(
        "Plugin '{}' lacks {:?} collection-management access",
        plugin_name, operation
    );
    error!("{}", message);
    write_error_result(caller, &message);
    false
}

fn validate_plugin_collection_schema(schema: &CollectionSchema) -> Result<(), String> {
    if schema.name.is_empty() {
        return Err("Collection name cannot be empty".to_string());
    }

    if schema.name.starts_with('_') {
        return Err("System collections cannot be managed by plugins".to_string());
    }

    if schema.fields.is_empty() {
        return Err("Collection must have at least one field".to_string());
    }

    schema.validate_identifiers()
}

fn ensure_user_collection_name(collection: &str) -> Result<(), String> {
    if collection.is_empty() {
        return Err("Collection name cannot be empty".to_string());
    }

    if collection.starts_with('_') {
        return Err("System collections cannot be managed by plugins".to_string());
    }

    Ok(())
}

fn read_collection_schema_from_plugin_memory(
    caller: &mut Caller<'_, PluginStoreData>,
    schema_ptr: i32,
    schema_len: i32,
) -> Result<CollectionSchema, String> {
    let schema_json = read_string_from_plugin_memory(caller, schema_ptr, schema_len)
        .map_err(|e| e.to_string())?;
    serde_json::from_str::<CollectionSchema>(&schema_json)
        .map_err(|e| format!("Failed to parse collection schema: {}", e))
}

fn can_perform_database_operations(
    caller: &mut Caller<'_, PluginStoreData>,
    log_context: bool,
) -> bool {
    let Some(state) = lock_host_state(caller.data(), "checking database operation context") else {
        write_error_result(caller, "Plugin host state not available");
        return false;
    };
    let can_perform = state.can_perform_database_operations();

    if log_context {
        debug!(
            "Database operation check: can_perform={}, execution_context={:?}, has_http_request={}",
            can_perform,
            state.execution_context,
            state.current_http_request.is_some()
        );
    }

    can_perform
}

/// Handle create_record host function call.
fn handle_create_record(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording create_record host call") {
        return -1;
    }

    let db = db.clone();

    // Clear HTTP result buffer before database operation
    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    if !ensure_database_capability(caller, CrudOperation::Create, &collection) {
        return -1;
    }

    let record_data_str = match read_string_from_plugin_memory(caller, data_ptr, data_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    // Parse JSON data for the record
    let record_data: RecordData = match serde_json::from_str(&record_data_str) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to parse record data: {}", e);
            return -1;
        }
    };

    // Check if we can perform database operations (prevent reentrancy)
    let can_perform_db_ops = can_perform_database_operations(caller, true);

    if !can_perform_db_ops {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let result = super::bridge::run_host_operation(async move {
        db.create_record(&collection_clone, record_data).await
    });

    // Mark that we're exiting the database operation
    exit_database_operation(caller);

    match result {
        Ok(Ok(record)) => {
            let response_data = serde_json::json!({
                "id": record.id,
                "collection": collection,
                "data": record.data,
                "created_at": record.created_at.to_string(),
                "updated_at": record.updated_at.to_string()
            });

            let result_bytes = create_success_response(response_data);

            // Allocate plugin memory and copy data
            if write_bytes_result(caller, &result_bytes, "storing create_record result") {
                info!("Created record in collection: {}", collection);
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for create_record result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to create record: {}", e);
            write_error_result(caller, &e.to_string());
            -1 // Error
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1 // Error
        }
    }
}

/// Handle read_records host function call.
fn handle_read_records(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    filter_ptr: i32,
    filter_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording read_records host call") {
        return -1;
    }

    let db = db.clone();

    // Clear HTTP result buffer before database operation
    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    if !ensure_database_capability(caller, CrudOperation::Read, &collection) {
        return -1;
    }

    let filter_str = if filter_len > 0 {
        match read_string_from_plugin_memory(caller, filter_ptr, filter_len) {
            Ok(s) => Some(s),
            Err(_) => return -1,
        }
    } else {
        None
    };

    // Parse filter as ListParams if provided
    let list_params = if let Some(filter_json) = filter_str {
        serde_json::from_str::<ListParams>(&filter_json).unwrap_or_default()
    } else {
        ListParams::default()
    };

    let can_perform_db_ops = can_perform_database_operations(caller, false);
    if !can_perform_db_ops {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let result = super::bridge::run_host_operation(async move {
        db.list_records(&collection_clone, list_params).await
    });

    exit_database_operation(caller);

    match result {
        Ok(Ok(records)) => {
            let response_data = serde_json::json!(records
                .iter()
                .map(|record| {
                    serde_json::json!({
                        "id": record.id,
                        "collection": collection,
                        "data": record.data,
                        "created_at": record.created_at.to_string(),
                        "updated_at": record.updated_at.to_string()
                    })
                })
                .collect::<Vec<_>>());

            let result_bytes = create_success_response(response_data);

            // Allocate plugin memory and copy data
            if write_bytes_result(caller, &result_bytes, "storing read_records result") {
                info!(
                    "Read {} records from collection: {}",
                    records.len(),
                    collection
                );
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for read_records result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to read records: {}", e);
            write_error_result(caller, &e.to_string());
            -1 // Error
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1 // Error
        }
    }
}

/// Handle update_record host function call.
#[allow(clippy::too_many_arguments)]
fn handle_update_record(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    record_id_ptr: i32,
    record_id_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording update_record host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    if !ensure_database_capability(caller, CrudOperation::Update, &collection) {
        return -1;
    }

    let record_id = match read_string_from_plugin_memory(caller, record_id_ptr, record_id_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let record_data_str = match read_string_from_plugin_memory(caller, data_ptr, data_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    // Parse JSON data for the record
    let record_data: RecordData = match serde_json::from_str(&record_data_str) {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to parse record data: {}", e);
            return -1;
        }
    };

    // Check if we can perform database operations (prevent reentrancy)
    let can_perform_db_ops = can_perform_database_operations(caller, false);

    if !can_perform_db_ops {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let record_id_clone = record_id.clone();
    let result = super::bridge::run_host_operation(async move {
        db.update_record(
            &collection_clone,
            &RecordId::from(record_id_clone),
            record_data,
        )
        .await
    });

    // Mark that we're exiting the database operation
    exit_database_operation(caller);

    match result {
        Ok(Ok(record)) => {
            let response_data = serde_json::json!({
                "id": record.id,
                "collection": collection,
                "data": record.data,
                "created_at": record.created_at.to_string(),
                "updated_at": record.updated_at.to_string()
            });

            let result_bytes = create_success_response(response_data);

            // Allocate plugin memory and copy data
            if write_bytes_result(caller, &result_bytes, "storing update_record result") {
                info!("Updated record {} in collection: {}", record.id, collection);
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for update_record result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to update record: {}", e);
            write_error_result(caller, &e.to_string());
            -1 // Error
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1 // Error
        }
    }
}

/// Handle delete_record host function call.
fn handle_delete_record(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    record_id_ptr: i32,
    record_id_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording delete_record host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    if !ensure_database_capability(caller, CrudOperation::Delete, &collection) {
        return -1;
    }

    let record_id = match read_string_from_plugin_memory(caller, record_id_ptr, record_id_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    // Check if we can perform database operations (prevent reentrancy)
    let can_perform_db_ops = can_perform_database_operations(caller, false);

    if !can_perform_db_ops {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let record_id_clone = record_id.clone();
    let result = super::bridge::run_host_operation(async move {
        db.delete_record(&collection_clone, &RecordId::from(record_id_clone))
            .await
    });

    // Mark that we're exiting the database operation
    exit_database_operation(caller);

    match result {
        Ok(Ok(record)) => {
            let response_data = serde_json::json!({
                "id": record.id,
                "collection": collection,
                "data": record.data,
                "created_at": record.created_at.to_string(),
                "updated_at": record.updated_at.to_string()
            });

            let result_bytes = create_success_response(response_data);

            // Allocate plugin memory and copy data
            if write_bytes_result(caller, &result_bytes, "storing delete_record result") {
                info!(
                    "Deleted record {} from collection: {}",
                    record.id, collection
                );
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for delete_record result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to delete record: {}", e);
            write_error_result(caller, &e.to_string());
            -1 // Error
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1 // Error
        }
    }
}

/// Handle create_collection host function call.
fn handle_create_collection(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    schema_ptr: i32,
    schema_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording create_collection host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let schema = match read_collection_schema_from_plugin_memory(caller, schema_ptr, schema_len) {
        Ok(schema) => schema,
        Err(e) => {
            write_error_result(caller, &e);
            return -1;
        }
    };

    if let Err(e) = validate_plugin_collection_schema(&schema) {
        write_error_result(caller, &e);
        return -1;
    }

    if !ensure_collection_capability(caller, CollectionOperation::Create, &schema.name) {
        return -1;
    }

    if !can_perform_database_operations(caller, false) {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let schema_name = schema.name.clone();
    let response_schema = schema.clone();
    let result =
        super::bridge::run_host_operation(async move { db.create_collection(schema).await });

    exit_database_operation(caller);

    match result {
        Ok(Ok(())) => {
            info!("Created collection from plugin host call: {}", schema_name);
            write_serialized_success(caller, &response_schema, "storing create_collection result")
        }
        Ok(Err(e)) => {
            error!("Failed to create collection: {}", e);
            write_error_result(caller, &e.to_string());
            -1
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1
        }
    }
}

/// Handle list_collections host function call.
fn handle_list_collections(caller: &mut Caller<'_, PluginStoreData>, db: &Arc<dyn Db>) -> i32 {
    if !record_host_call(caller.data(), "recording list_collections host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    if !ensure_any_collection_capability(caller, CollectionOperation::List) {
        return -1;
    }

    if !can_perform_database_operations(caller, false) {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let result = super::bridge::run_host_operation(async move { db.list_collections().await });

    exit_database_operation(caller);

    match result {
        Ok(Ok(collections)) => {
            let filtered_collections = {
                let Some(state) =
                    lock_host_state(caller.data(), "filtering list_collections result")
                else {
                    write_error_result(caller, "Plugin host state not available");
                    return -1;
                };

                collections
                    .into_iter()
                    .filter(|schema| {
                        state.current_plugin_can_manage_collection(
                            &CollectionOperation::List,
                            &schema.name,
                        )
                    })
                    .collect::<Vec<_>>()
            };

            info!(
                "Listed {} collections for plugin host call",
                filtered_collections.len()
            );
            write_serialized_success(
                caller,
                &filtered_collections,
                "storing list_collections result",
            )
        }
        Ok(Err(e)) => {
            error!("Failed to list collections: {}", e);
            write_error_result(caller, &e.to_string());
            -1
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1
        }
    }
}

/// Handle get_collection_schema host function call.
fn handle_get_collection_schema(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording get_collection_schema host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(e) => {
            write_error_result(caller, e);
            return -1;
        }
    };

    if !ensure_collection_capability(caller, CollectionOperation::Read, &collection) {
        return -1;
    }

    if !can_perform_database_operations(caller, false) {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let result = super::bridge::run_host_operation(async move {
        db.get_collection_schema(&collection_clone).await
    });

    exit_database_operation(caller);

    match result {
        Ok(Ok(schema)) => {
            info!(
                "Read collection schema from plugin host call: {}",
                collection
            );
            write_serialized_success(caller, &schema, "storing get_collection_schema result")
        }
        Ok(Err(e)) => {
            error!("Failed to read collection schema: {}", e);
            write_error_result(caller, &e.to_string());
            -1
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1
        }
    }
}

/// Handle update_collection_schema host function call.
#[allow(clippy::too_many_arguments)]
fn handle_update_collection_schema(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    schema_ptr: i32,
    schema_len: i32,
) -> i32 {
    if !record_host_call(
        caller.data(),
        "recording update_collection_schema host call",
    ) {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(e) => {
            write_error_result(caller, e);
            return -1;
        }
    };

    if let Err(e) = ensure_user_collection_name(&collection) {
        write_error_result(caller, &e);
        return -1;
    }

    let schema = match read_collection_schema_from_plugin_memory(caller, schema_ptr, schema_len) {
        Ok(schema) => schema,
        Err(e) => {
            write_error_result(caller, &e);
            return -1;
        }
    };

    if schema.name != collection {
        write_error_result(caller, "Schema name must match collection name");
        return -1;
    }

    if let Err(e) = validate_plugin_collection_schema(&schema) {
        write_error_result(caller, &e);
        return -1;
    }

    if !ensure_collection_capability(caller, CollectionOperation::Update, &collection) {
        return -1;
    }

    if !can_perform_database_operations(caller, false) {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let response_schema = schema.clone();
    let result = super::bridge::run_host_operation(async move {
        db.update_collection_schema(&collection_clone, schema).await
    });

    exit_database_operation(caller);

    match result {
        Ok(Ok(())) => {
            info!(
                "Updated collection schema from plugin host call: {}",
                collection
            );
            write_serialized_success(
                caller,
                &response_schema,
                "storing update_collection_schema result",
            )
        }
        Ok(Err(e)) => {
            error!("Failed to update collection schema: {}", e);
            write_error_result(caller, &e.to_string());
            -1
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1
        }
    }
}

/// Handle delete_collection host function call.
fn handle_delete_collection(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording delete_collection host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(e) => {
            write_error_result(caller, e);
            return -1;
        }
    };

    if let Err(e) = ensure_user_collection_name(&collection) {
        write_error_result(caller, &e);
        return -1;
    }

    if !ensure_collection_capability(caller, CollectionOperation::Delete, &collection) {
        return -1;
    }

    if !can_perform_database_operations(caller, false) {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let result =
        super::bridge::run_host_operation(
            async move { db.delete_collection(&collection_clone).await },
        );

    exit_database_operation(caller);

    match result {
        Ok(Ok(())) => {
            info!("Deleted collection from plugin host call: {}", collection);
            write_success_result(
                caller,
                serde_json::json!({ "collection": collection }),
                "storing delete_collection result",
            )
        }
        Ok(Err(e)) => {
            error!("Failed to delete collection: {}", e);
            write_error_result(caller, &e.to_string());
            -1
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1
        }
    }
}

/// Handle collection_exists host function call.
fn handle_collection_exists(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording collection_exists host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(e) => {
            write_error_result(caller, e);
            return -1;
        }
    };

    if !ensure_collection_capability(caller, CollectionOperation::Exists, &collection) {
        return -1;
    }

    if !can_perform_database_operations(caller, false) {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let result =
        super::bridge::run_host_operation(
            async move { db.collection_exists(&collection_clone).await },
        );

    exit_database_operation(caller);

    match result {
        Ok(Ok(exists)) => {
            info!(
                "Checked collection existence from plugin host call: {}",
                collection
            );
            write_serialized_success(
                caller,
                &CollectionExistsResponse { collection, exists },
                "storing collection_exists result",
            )
        }
        Ok(Err(e)) => {
            error!("Failed to check collection existence: {}", e);
            write_error_result(caller, &e.to_string());
            -1
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1
        }
    }
}

/// Handle get_collection_stats host function call.
fn handle_get_collection_stats(
    caller: &mut Caller<'_, PluginStoreData>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
) -> i32 {
    if !record_host_call(caller.data(), "recording get_collection_stats host call") {
        return -1;
    }

    let db = db.clone();

    if !clear_result_buffer(caller) {
        return -1;
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(e) => {
            write_error_result(caller, e);
            return -1;
        }
    };

    if !ensure_collection_capability(caller, CollectionOperation::Stats, &collection) {
        return -1;
    }

    if !can_perform_database_operations(caller, false) {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        write_error_result(caller, "Database operations not allowed during event handling to prevent circular dependencies");
        return -1;
    }
    if !enter_database_operation(caller) {
        return -1;
    }

    let collection_clone = collection.clone();
    let result = super::bridge::run_host_operation(async move {
        let exists = db.collection_exists(&collection_clone).await?;
        if !exists {
            return Ok::<CollectionStatsResponse, oxide_core::AppError>(
                CollectionStatsResponse::missing(collection_clone),
            );
        }

        let record_count = db.count_records(&collection_clone).await?;
        let size_kb = db.get_collection_size_kb(&collection_clone).await?;
        let schema = db.get_collection_schema(&collection_clone).await?;

        Ok::<CollectionStatsResponse, oxide_core::AppError>(CollectionStatsResponse {
            name: collection_clone,
            record_count,
            exists,
            size_kb,
            schema_version: Some(schema.version),
            created_at: Some(schema.created_at),
            updated_at: Some(schema.updated_at),
        })
    });

    exit_database_operation(caller);

    match result {
        Ok(Ok(stats)) => {
            info!(
                "Read collection stats from plugin host call: {}",
                collection
            );
            write_serialized_success(caller, &stats, "storing get_collection_stats result")
        }
        Ok(Err(e)) => {
            error!("Failed to read collection stats: {}", e);
            write_error_result(caller, &e.to_string());
            -1
        }
        Err(e) => {
            error!("Database operation failed: {}", e);
            write_error_result(caller, &e);
            -1
        }
    }
}
