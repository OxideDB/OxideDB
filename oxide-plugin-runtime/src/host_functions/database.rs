//! Database-related Host Functions
//!
//! These functions provide database access capabilities for plugins to perform
//! CRUD operations on collections within the OxideDB system.

use crate::host_state::HostState;
use crate::utils::{read_string_from_plugin_memory, allocate_plugin_memory_and_copy, create_error_response, create_success_response};
use oxide_core::plugin_api::{host_functions, PluginError};
use oxide_core::event::types::{RecordData, RecordId};
use oxide_db::{Db, db::ListParams};
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info};
use wasmtime::{Caller, Linker};

/// Define database-related host functions in the linker.
///
/// This includes:
/// - `create_record(collection_ptr, collection_len, data_ptr, data_len)`: Create a new record
/// - `read_records(collection_ptr, collection_len, filter_ptr, filter_len)`: Read records with filtering
/// - `update_record(collection_ptr, collection_len, record_id_ptr, record_id_len, data_ptr, data_len)`: Update a record
/// - `delete_record(collection_ptr, collection_len, record_id_ptr, record_id_len)`: Delete a record
pub fn define_database_functions(
    linker: &mut Linker<Arc<Mutex<HostState>>>,
    database: Arc<dyn Db>,
) -> Result<(), PluginError> {
    // create_record(collection_ptr: *const u8, collection_len: usize, data_ptr: *const u8, data_len: usize) -> i32
    {
        let db = database.clone();
        linker
            .func_wrap(
                "env",
                host_functions::CREATE_RECORD,
                move |mut caller: Caller<'_, Arc<Mutex<HostState>>>,
                      collection_ptr: i32, collection_len: i32,
                      data_ptr: i32, data_len: i32| -> i32 {
                    handle_create_record(&mut caller, &db, collection_ptr, collection_len, data_ptr, data_len)
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
                move |mut caller: Caller<'_, Arc<Mutex<HostState>>>,
                      collection_ptr: i32, collection_len: i32,
                      filter_ptr: i32, filter_len: i32| -> i32 {
                    handle_read_records(&mut caller, &db, collection_ptr, collection_len, filter_ptr, filter_len)
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
                move |mut caller: Caller<'_, Arc<Mutex<HostState>>>,
                      collection_ptr: i32, collection_len: i32,
                      record_id_ptr: i32, record_id_len: i32,
                      data_ptr: i32, data_len: i32| -> i32 {
                    handle_update_record(&mut caller, &db, collection_ptr, collection_len, record_id_ptr, record_id_len, data_ptr, data_len)
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
                move |mut caller: Caller<'_, Arc<Mutex<HostState>>>,
                      collection_ptr: i32, collection_len: i32,
                      record_id_ptr: i32, record_id_len: i32| -> i32 {
                    handle_delete_record(&mut caller, &db, collection_ptr, collection_len, record_id_ptr, record_id_len)
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define delete_record: {}", e))
            })?;
    }

    Ok(())
}

/// Handle create_record host function call.
fn handle_create_record(
    caller: &mut Caller<'_, Arc<Mutex<HostState>>>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i32 {
    caller.data().lock().unwrap().record_host_call();

    let db = db.clone();

    // Clear HTTP result buffer before database operation
    {
        let mut state = caller.data().lock().unwrap();
        state.result_buffer.clear();
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
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
    let can_perform_db_ops = {
        let state = caller.data().lock().unwrap();
        let can_perform = state.can_perform_database_operations();
        debug!("Database operation check: can_perform={}, execution_context={:?}, has_http_request={}",
               can_perform, state.execution_context, state.current_http_request.is_some());
        can_perform
    };

    if !can_perform_db_ops {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        let result_bytes = create_error_response("Database operations not allowed during event handling to prevent circular dependencies");

        if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
            let mut state = caller.data().lock().unwrap();
            state.result_buffer = [
                (ptr as u32).to_le_bytes().to_vec(),
                (len as u32).to_le_bytes().to_vec(),
            ].concat();
        }
        return -1;
    }

    // Use the same pattern as read_records - spawn thread with new runtime
    let collection_clone = collection.clone();
    let result = std::thread::spawn(move || {
        // Try to get current runtime handle, or create a new one
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // We have an active runtime, use it
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.create_record(&collection_clone, record_data).await
                })
            }
            Err(_) => {
                // No active runtime, create a new one
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.create_record(&collection_clone, record_data).await
                })
            }
        }
    }).join();

    // Mark that we're exiting the database operation
    {
        let mut state = caller.data().lock().unwrap();
        state.exit_database_operation();
    }

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
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
                info!("Created record in collection: {}", collection);
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for create_record result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to create record: {}", e);
            let result_bytes = create_error_response(&e.to_string());

            // Allocate plugin memory and copy error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
        Err(_) => {
            error!("Database operation timed out");
            let result_bytes = create_error_response("Database operation timed out");

            // Allocate plugin memory and copy timeout error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
    }
}

/// Handle read_records host function call.
fn handle_read_records(
    caller: &mut Caller<'_, Arc<Mutex<HostState>>>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    filter_ptr: i32,
    filter_len: i32,
) -> i32 {
    caller.data().lock().unwrap().record_host_call();

    let db = db.clone();

    // Clear HTTP result buffer before database operation
    {
        let mut state = caller.data().lock().unwrap();
        state.result_buffer.clear();
    }

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

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
        match serde_json::from_str::<ListParams>(&filter_json) {
            Ok(params) => params,
            Err(_) => ListParams::default(),
        }
    } else {
        ListParams::default()
    };

    // Use tokio::task::spawn_blocking with proper runtime coordination
    let collection_clone = collection.clone();
    let result = std::thread::spawn(move || {
        // Try to get current runtime handle, or create a new one
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // We have an active runtime, use it
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.list_records(&collection_clone, list_params).await
                })
            }
            Err(_) => {
                // No active runtime, create a new one
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.list_records(&collection_clone, list_params).await
                })
            }
        }
    }).join();

    match result {
        Ok(Ok(records)) => {
            let response_data = serde_json::json!(
                records.iter().map(|record| {
                    serde_json::json!({
                        "id": record.id,
                        "collection": collection,
                        "data": record.data,
                        "created_at": record.created_at.to_string(),
                        "updated_at": record.updated_at.to_string()
                    })
                }).collect::<Vec<_>>()
            );

            let result_bytes = create_success_response(response_data);

            // Allocate plugin memory and copy data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
                info!("Read {} records from collection: {}", records.len(), collection);
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for read_records result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to read records: {}", e);
            let result_bytes = create_error_response(&e.to_string());

            // Allocate plugin memory and copy error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
        Err(_) => {
            error!("Database operation thread panicked or failed");
            let result_bytes = create_error_response("Database operation failed");

            // Allocate plugin memory and copy error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
    }
}

/// Handle update_record host function call.
fn handle_update_record(
    caller: &mut Caller<'_, Arc<Mutex<HostState>>>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    record_id_ptr: i32,
    record_id_len: i32,
    data_ptr: i32,
    data_len: i32,
) -> i32 {
    caller.data().lock().unwrap().record_host_call();

    let db = db.clone();

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

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
    let can_perform_db_ops = {
        let state = caller.data().lock().unwrap();
        state.can_perform_database_operations()
    };

    if !can_perform_db_ops {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        let result_bytes = create_error_response("Database operations not allowed during event handling to prevent circular dependencies");

        if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
            let mut state = caller.data().lock().unwrap();
            state.result_buffer = [
                (ptr as u32).to_le_bytes().to_vec(),
                (len as u32).to_le_bytes().to_vec(),
            ].concat();
        }
        return -1;
    }

    // Use the same pattern as read_records - spawn thread with new runtime
    let collection_clone = collection.clone();
    let record_id_clone = record_id.clone();
    let result = std::thread::spawn(move || {
        // Try to get current runtime handle, or create a new one
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // We have an active runtime, use it
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.update_record(&collection_clone, &RecordId::from(record_id_clone), record_data).await
                })
            }
            Err(_) => {
                // No active runtime, create a new one
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.update_record(&collection_clone, &RecordId::from(record_id_clone), record_data).await
                })
            }
        }
    }).join();

    // Mark that we're exiting the database operation
    {
        let mut state = caller.data().lock().unwrap();
        state.exit_database_operation();
    }

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
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
                info!("Updated record {} in collection: {}", record.id, collection);
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for update_record result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to update record: {}", e);
            let result_bytes = create_error_response(&e.to_string());

            // Allocate plugin memory and copy error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
        Err(_) => {
            error!("Database operation timed out");
            let result_bytes = create_error_response("Database operation timed out");

            // Allocate plugin memory and copy timeout error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
    }
}

/// Handle delete_record host function call.
fn handle_delete_record(
    caller: &mut Caller<'_, Arc<Mutex<HostState>>>,
    db: &Arc<dyn Db>,
    collection_ptr: i32,
    collection_len: i32,
    record_id_ptr: i32,
    record_id_len: i32,
) -> i32 {
    caller.data().lock().unwrap().record_host_call();

    let db = db.clone();

    let collection = match read_string_from_plugin_memory(caller, collection_ptr, collection_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    let record_id = match read_string_from_plugin_memory(caller, record_id_ptr, record_id_len) {
        Ok(s) => s,
        Err(_) => return -1,
    };

    // Check if we can perform database operations (prevent reentrancy)
    let can_perform_db_ops = {
        let state = caller.data().lock().unwrap();
        state.can_perform_database_operations()
    };

    if !can_perform_db_ops {
        error!("Database operation blocked - plugin is in event handler context (prevents circular dependency)");
        let result_bytes = create_error_response("Database operations not allowed during event handling to prevent circular dependencies");

        if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
            let mut state = caller.data().lock().unwrap();
            state.result_buffer = [
                (ptr as u32).to_le_bytes().to_vec(),
                (len as u32).to_le_bytes().to_vec(),
            ].concat();
        }
        return -1;
    }

    // Use the same pattern as read_records - spawn thread with new runtime
    let collection_clone = collection.clone();
    let record_id_clone = record_id.clone();
    let result = std::thread::spawn(move || {
        // Try to get current runtime handle, or create a new one
        match tokio::runtime::Handle::try_current() {
            Ok(_handle) => {
                // We have an active runtime, use it
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.delete_record(&collection_clone, &RecordId::from(record_id_clone)).await
                })
            }
            Err(_) => {
                // No active runtime, create a new one
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(async {
                    db.delete_record(&collection_clone, &RecordId::from(record_id_clone)).await
                })
            }
        }
    }).join();

    // Mark that we're exiting the database operation
    {
        let mut state = caller.data().lock().unwrap();
        state.exit_database_operation();
    }

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
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
                info!("Deleted record {} from collection: {}", record.id, collection);
                0 // Success
            } else {
                error!("Failed to allocate plugin memory for delete_record result");
                -1 // Error
            }
        }
        Ok(Err(e)) => {
            error!("Failed to delete record: {}", e);
            let result_bytes = create_error_response(&e.to_string());

            // Allocate plugin memory and copy error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
        Err(_) => {
            error!("Database operation timed out");
            let result_bytes = create_error_response("Database operation timed out");

            // Allocate plugin memory and copy timeout error data
            if let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, &result_bytes) {
                let mut state = caller.data().lock().unwrap();
                state.result_buffer = [
                    (ptr as u32).to_le_bytes().to_vec(),
                    (len as u32).to_le_bytes().to_vec(),
                ].concat();
            }
            -1 // Error
        }
    }
}
