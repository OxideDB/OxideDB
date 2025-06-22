//! Wasmtime-based Plugin Runtime Implementation
//!
//! This module implements the PluginRuntime trait using Wasmtime,
//! providing plugin loading, execution, and security management.

use crate::host_state::HostState;
use oxide_core::{
    plugin_api::{host_functions, EventPayload, PluginError, PluginResponse, PluginResult, PluginRuntime},
    plugin_security::{
        PluginSecurityManager, PluginCapability, PluginTrustLevel, SecurityPolicies,
        SecurityViolation, ResourceLimits, ExecutionStats, SecurityAuditEntry
    },
    CrudOperation,
    event::types::{RecordData, RecordId},
};
use oxide_db::{Db, db::ListParams};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};
use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};

/// Wasmtime-based implementation of the PluginRuntime trait
pub struct WasmtimePluginRuntime {
    engine: Engine,
    linker: Linker<Arc<Mutex<HostState>>>,
    store: Store<Arc<Mutex<HostState>>>,
    modules: HashMap<String, Module>,
    instances: HashMap<String, Instance>,
    security_manager: PluginSecurityManager,
    database: Arc<dyn Db>,
}

impl WasmtimePluginRuntime {
    /// Create a new Wasmtime plugin runtime with database
    pub fn new(database: Arc<dyn Db>) -> PluginResult<Self> {
        let engine = Engine::default();
        let linker = Linker::new(&engine);
        let host_state = Arc::new(Mutex::new(HostState::default()));
        let store = Store::new(&engine, host_state);

        // Create security manager with default policies
        let security_manager = PluginSecurityManager::new();

        // Define host functions that plugins can call
        let mut runtime = Self {
            engine,
            linker,
            store,
            modules: HashMap::new(),
            instances: HashMap::new(),
            security_manager,
            database,
        };

        runtime.define_host_functions()?;
        Ok(runtime)
    }

    /// Create a new Wasmtime plugin runtime with custom security policies
    pub fn new_with_security_policies(database: Arc<dyn Db>, policies: SecurityPolicies) -> PluginResult<Self> {
        let engine = Engine::default();
        let linker = Linker::new(&engine);
        let host_state = Arc::new(Mutex::new(HostState::default()));
        let store = Store::new(&engine, host_state);

        // Create security manager with custom policies
        let security_manager = PluginSecurityManager::with_policies(policies);

        // Define host functions that plugins can call
        let mut runtime = Self {
            engine,
            linker,
            store,
            modules: HashMap::new(),
            instances: HashMap::new(),
            security_manager,
            database,
        };

        runtime.define_host_functions()?;
        Ok(runtime)
    }

    /// Define all host functions that plugins can import
    fn define_host_functions(&mut self) -> PluginResult<()> {
        // get_event_payload() -> i32 (returns length, copies data to plugin memory)
        self.linker
            .func_wrap(
                "env",
                host_functions::GET_EVENT_PAYLOAD,
                |mut caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                    let state = caller.data().lock().unwrap();
                    if let Some(payload) = &state.current_payload {
                        let payload_bytes = payload.as_bytes().to_vec();
                        drop(state); // Release the lock early

                        // Get plugin memory and allocate space
                        if let Some(memory) =
                            caller.get_export("memory").and_then(|e| e.into_memory())
                        {
                            // Call plugin's alloc function to get memory
                            if let Some(alloc_export) = caller.get_export("alloc") {
                                if let Some(alloc_func_raw) = alloc_export.into_func() {
                                    if let Ok(alloc_func) =
                                        alloc_func_raw.typed::<i32, i32>(&mut caller)
                                    {
                                        if let Ok(ptr) =
                                            alloc_func.call(&mut caller, payload_bytes.len() as i32)
                                        {
                                            // Copy data to plugin memory
                                            let data = memory.data_mut(&mut caller);
                                            let start = ptr as usize;
                                            let end = start + payload_bytes.len();

                                            if end <= data.len() {
                                                data[start..end].copy_from_slice(&payload_bytes);

                                                // Store the pointer and length in result buffer for get_result_ptr/len
                                                let mut state = caller.data().lock().unwrap();
                                                state.result_buffer = [
                                                    (ptr as u32).to_le_bytes().to_vec(),
                                                    (payload_bytes.len() as u32)
                                                        .to_le_bytes()
                                                        .to_vec(),
                                                ]
                                                .concat();

                                                return payload_bytes.len() as i32;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    -1
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Failed to define get_event_payload: {}",
                    e
                ))
            })?;

        // get_result_ptr() -> i32 (returns pointer to allocated plugin memory)
        self.linker
            .func_wrap(
                "env",
                "get_result_ptr",
                |caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                    let state = caller.data().lock().unwrap();
                    if state.result_buffer.len() >= 4 {
                        // Return the pointer stored in the first 4 bytes
                        u32::from_le_bytes([
                            state.result_buffer[0],
                            state.result_buffer[1],
                            state.result_buffer[2],
                            state.result_buffer[3],
                        ]) as i32
                    } else {
                        0
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define get_result_ptr: {}", e))
            })?;

        // get_result_len() -> i32 (returns length of data)
        self.linker
            .func_wrap(
                "env",
                "get_result_len",
                |caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                    let state = caller.data().lock().unwrap();
                    if state.result_buffer.len() >= 8 {
                        // Return the length stored in bytes 4-7
                        u32::from_le_bytes([
                            state.result_buffer[4],
                            state.result_buffer[5],
                            state.result_buffer[6],
                            state.result_buffer[7],
                        ]) as i32
                    } else {
                        0
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define get_result_len: {}", e))
            })?;

        // log_info(ptr: *const u8, len: usize)
        self.linker
            .func_wrap(
                "env",
                host_functions::LOG_INFO,
                |mut caller: Caller<'_, Arc<Mutex<HostState>>>, ptr: i32, len: i32| {
                    // Get current plugin name for security validation
                    let plugin_name = {
                        let state = caller.data().lock().unwrap();
                        state.current_plugin.clone()
                    };

                    if let Some(plugin_name) = plugin_name {
                        // Note: Security validation would be done in the runtime's call_plugin_function
                        // This is a simplified version for the host function
                        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                        {
                            let data = memory.data(&caller);
                            if let Ok(message) =
                                std::str::from_utf8(&data[ptr as usize..(ptr + len) as usize])
                            {
                                info!("[PLUGIN:{}] {}", plugin_name, message);
                                caller
                                    .data()
                                    .lock()
                                    .unwrap()
                                    .log_messages
                                    .push(format!("[{}] {}", plugin_name, message));
                            }
                        }
                    } else {
                        warn!("log_info called without plugin context");
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define log_info: {}", e))
            })?;

        // log_error(ptr: *const u8, len: usize)
        self.linker
            .func_wrap(
                "env",
                host_functions::LOG_ERROR,
                |mut caller: Caller<'_, Arc<Mutex<HostState>>>, ptr: i32, len: i32| {
                    // Get current plugin name for security validation
                    let plugin_name = {
                        let state = caller.data().lock().unwrap();
                        state.current_plugin.clone()
                    };

                    if let Some(plugin_name) = plugin_name {
                        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                        {
                            let data = memory.data(&caller);
                            if let Ok(message) =
                                std::str::from_utf8(&data[ptr as usize..(ptr + len) as usize])
                            {
                                error!("[PLUGIN:{}] {}", plugin_name, message);
                                caller
                                    .data()
                                    .lock()
                                    .unwrap()
                                    .log_messages
                                    .push(format!("ERROR [{}]: {}", plugin_name, message));
                            }
                        }
                    } else {
                        warn!("log_error called without plugin context");
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define log_error: {}", e))
            })?;

        // set_error(ptr: *const u8, len: usize)
        self.linker
            .func_wrap(
                "env",
                host_functions::SET_ERROR,
                |mut caller: Caller<'_, Arc<Mutex<HostState>>>, ptr: i32, len: i32| {
                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    {
                        let data = memory.data(&caller);
                        if let Ok(message) =
                            std::str::from_utf8(&data[ptr as usize..(ptr + len) as usize])
                        {
                            warn!("[PLUGIN ERROR] {}", message);
                            caller.data().lock().unwrap().error_message = Some(message.to_string());
                        }
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define set_error: {}", e))
            })?;

        // register_http_route(method_ptr: *const u8, method_len: usize, path_ptr: *const u8, path_len: usize, handler_ptr: *const u8, handler_len: usize)
        self.linker
            .func_wrap(
                "env",
                host_functions::REGISTER_HTTP_ROUTE,
                |mut caller: Caller<'_, Arc<Mutex<HostState>>>, 
                 method_ptr: i32, method_len: i32,
                 path_ptr: i32, path_len: i32,
                 handler_ptr: i32, handler_len: i32| -> i32 {
                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                        let data = memory.data(&caller);
                        
                        let method = match std::str::from_utf8(&data[method_ptr as usize..(method_ptr + method_len) as usize]) {
                            Ok(s) => s.to_string(),
                            Err(_) => return -1,
                        };
                        
                        let path = match std::str::from_utf8(&data[path_ptr as usize..(path_ptr + path_len) as usize]) {
                            Ok(s) => s.to_string(),
                            Err(_) => return -1,
                        };
                        
                        let handler_function = match std::str::from_utf8(&data[handler_ptr as usize..(handler_ptr + handler_len) as usize]) {
                            Ok(s) => s.to_string(),
                            Err(_) => return -1,
                        };

                        let plugin_name = {
                            let state = caller.data().lock().unwrap();
                            state.current_plugin.clone()
                        };

                        if let Some(plugin_name) = plugin_name {
                            let route = oxide_core::plugin_api::RouteRegistration {
                                method,
                                path,
                                handler_function,
                                plugin_name,
                            };

                            let route_path = format!("{} {}", route.method, route.path);

                            let mut state = caller.data().lock().unwrap();
                            state.registered_routes.push(route);
                            info!("Registered HTTP route: {}", route_path);
                            0 // Success
                        } else {
                            -1 // Error: no plugin context
                        }
                    } else {
                        -1 // Error: no memory
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define register_http_route: {}", e))
            })?;

        // create_record(collection_ptr: *const u8, collection_len: usize, data_ptr: *const u8, data_len: usize) -> i32
        self.linker
            .func_wrap(
                "env",
                host_functions::CREATE_RECORD,
                {
                    let db = self.database.clone();
                    move |mut caller: Caller<'_, Arc<Mutex<HostState>>>, 
                          collection_ptr: i32, collection_len: i32,
                          data_ptr: i32, data_len: i32| -> i32 {
                        let db = db.clone(); // Clone inside the closure for each call
                        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                            let data = memory.data(&caller);
                            
                            let collection = match std::str::from_utf8(&data[collection_ptr as usize..(collection_ptr + collection_len) as usize]) {
                                Ok(s) => s.to_string(),
                                Err(_) => return -1,
                            };
                            
                            let record_data_str = match std::str::from_utf8(&data[data_ptr as usize..(data_ptr + data_len) as usize]) {
                                Ok(s) => s.to_string(),
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

                            // Use spawn_blocking for async database operation to avoid blocking the async runtime
                            let collection_clone = collection.clone();
                            let result = std::thread::spawn(move || {
                                let rt = tokio::runtime::Handle::try_current()
                                    .or_else(|_| {
                                        // Fallback to creating a new runtime if we're not in an async context
                                        tokio::runtime::Runtime::new().map(|rt| rt.handle().clone())
                                    });
                                
                                match rt {
                                    Ok(handle) => {
                                        handle.block_on(async {
                                            db.create_record(&collection_clone, record_data).await
                                        })
                                    }
                                    Err(_) => {
                                        // Last resort: create a new runtime
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async {
                                            db.create_record(&collection_clone, record_data).await
                                        })
                                    }
                                }
                            }).join();

                            match result {
                                Ok(Ok(record)) => {
                                    let response = serde_json::json!({
                                        "success": true,
                                        "data": {
                                            "id": record.id,
                                            "collection": collection,
                                            "data": record.data,
                                            "created_at": record.created_at,
                                            "updated_at": record.updated_at
                                        }
                                    });

                                    let result_bytes = response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    info!("Created record in collection: {}", collection);
                                    0 // Success
                                }
                                Ok(Err(e)) => {
                                    error!("Failed to create record: {}", e);
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": e.to_string()
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                                Err(_) => {
                                    error!("Database operation thread panicked");
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": "Database operation failed"
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                            }
                        } else {
                            -1 // Error: no memory
                        }
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define create_record: {}", e))
            })?;

        // read_records(collection_ptr: *const u8, collection_len: usize, filter_ptr: *const u8, filter_len: usize) -> i32
        self.linker
            .func_wrap(
                "env",
                host_functions::READ_RECORDS,
                {
                    let db = self.database.clone();
                    move |mut caller: Caller<'_, Arc<Mutex<HostState>>>, 
                          collection_ptr: i32, collection_len: i32,
                          filter_ptr: i32, filter_len: i32| -> i32 {
                        let db = db.clone(); // Clone inside the closure for each call
                        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                            let data = memory.data(&caller);
                            
                            let collection = match std::str::from_utf8(&data[collection_ptr as usize..(collection_ptr + collection_len) as usize]) {
                                Ok(s) => s.to_string(),
                                Err(_) => return -1,
                            };
                            
                            let filter_str = if filter_len > 0 {
                                match std::str::from_utf8(&data[filter_ptr as usize..(filter_ptr + filter_len) as usize]) {
                                    Ok(s) => Some(s.to_string()),
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

                            // Use spawn_blocking for async database operation to avoid blocking the async runtime
                            let collection_clone = collection.clone();
                            let result = std::thread::spawn(move || {
                                let rt = tokio::runtime::Handle::try_current()
                                    .or_else(|_| {
                                        tokio::runtime::Runtime::new().map(|rt| rt.handle().clone())
                                    });
                                
                                match rt {
                                    Ok(handle) => {
                                        handle.block_on(async {
                                            db.list_records(&collection_clone, list_params).await
                                        })
                                    }
                                    Err(_) => {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async {
                                            db.list_records(&collection_clone, list_params).await
                                        })
                                    }
                                }
                            }).join();

                            match result {
                                Ok(Ok(records)) => {
                                    let response = serde_json::json!({
                                        "success": true,
                                        "data": records.iter().map(|record| {
                                            serde_json::json!({
                                                "id": record.id,
                                                "collection": collection,
                                                "data": record.data,
                                                "created_at": record.created_at,
                                                "updated_at": record.updated_at
                                            })
                                        }).collect::<Vec<_>>()
                                    });

                                    let result_bytes = response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    info!("Read {} records from collection: {}", records.len(), collection);
                                    0 // Success
                                }
                                Ok(Err(e)) => {
                                    error!("Failed to read records: {}", e);
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": e.to_string()
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                                Err(_) => {
                                    error!("Database operation thread panicked");
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": "Database operation failed"
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                            }
                        } else {
                            -1 // Error: no memory
                        }
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define read_records: {}", e))
            })?;

        // update_record(collection_ptr: *const u8, collection_len: usize, record_id_ptr: *const u8, record_id_len: usize, data_ptr: *const u8, data_len: usize) -> i32
        self.linker
            .func_wrap(
                "env",
                host_functions::UPDATE_RECORD,
                {
                    let db = self.database.clone();
                    move |mut caller: Caller<'_, Arc<Mutex<HostState>>>, 
                          collection_ptr: i32, collection_len: i32,
                          record_id_ptr: i32, record_id_len: i32,
                          data_ptr: i32, data_len: i32| -> i32 {
                        let db = db.clone(); // Clone inside the closure for each call
                        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                            let data = memory.data(&caller);
                            
                            let collection = match std::str::from_utf8(&data[collection_ptr as usize..(collection_ptr + collection_len) as usize]) {
                                Ok(s) => s.to_string(),
                                Err(_) => return -1,
                            };
                            
                            let record_id = match std::str::from_utf8(&data[record_id_ptr as usize..(record_id_ptr + record_id_len) as usize]) {
                                Ok(s) => s.to_string(),
                                Err(_) => return -1,
                            };
                            
                            let record_data_str = match std::str::from_utf8(&data[data_ptr as usize..(data_ptr + data_len) as usize]) {
                                Ok(s) => s.to_string(),
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

                            // Use spawn_blocking for async database operation to avoid blocking the async runtime
                            let collection_clone = collection.clone();
                            let record_id_clone = record_id.clone();
                            let result = std::thread::spawn(move || {
                                let rt = tokio::runtime::Handle::try_current()
                                    .or_else(|_| {
                                        tokio::runtime::Runtime::new().map(|rt| rt.handle().clone())
                                    });
                                
                                match rt {
                                    Ok(handle) => {
                                        handle.block_on(async {
                                            db.update_record(&collection_clone, &RecordId::from(record_id_clone.clone()), record_data).await
                                        })
                                    }
                                    Err(_) => {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async {
                                            db.update_record(&collection_clone, &RecordId::from(record_id_clone), record_data).await
                                        })
                                    }
                                }
                            }).join();

                            match result {
                                Ok(Ok(record)) => {
                                    let response = serde_json::json!({
                                        "success": true,
                                        "data": {
                                            "id": record.id,
                                            "collection": collection,
                                            "data": record.data,
                                            "created_at": record.created_at,
                                            "updated_at": record.updated_at
                                        }
                                    });

                                    let result_bytes = response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    info!("Updated record {} in collection: {}", record.id, collection);
                                    0 // Success
                                }
                                Ok(Err(e)) => {
                                    error!("Failed to update record: {}", e);
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": e.to_string()
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                                Err(_) => {
                                    error!("Database operation thread panicked");
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": "Database operation failed"
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                            }
                        } else {
                            -1 // Error: no memory
                        }
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define update_record: {}", e))
            })?;

        // delete_record(collection_ptr: *const u8, collection_len: usize, record_id_ptr: *const u8, record_id_len: usize) -> i32
        self.linker
            .func_wrap(
                "env",
                host_functions::DELETE_RECORD,
                {
                    let db = self.database.clone();
                    move |mut caller: Caller<'_, Arc<Mutex<HostState>>>, 
                          collection_ptr: i32, collection_len: i32,
                          record_id_ptr: i32, record_id_len: i32| -> i32 {
                        let db = db.clone(); // Clone inside the closure for each call
                        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                            let data = memory.data(&caller);
                            
                            let collection = match std::str::from_utf8(&data[collection_ptr as usize..(collection_ptr + collection_len) as usize]) {
                                Ok(s) => s.to_string(),
                                Err(_) => return -1,
                            };
                            
                            let record_id = match std::str::from_utf8(&data[record_id_ptr as usize..(record_id_ptr + record_id_len) as usize]) {
                                Ok(s) => s.to_string(),
                                Err(_) => return -1,
                            };

                            // Use spawn_blocking for async database operation to avoid blocking the async runtime
                            let collection_clone = collection.clone();
                            let record_id_clone = record_id.clone();
                            let result = std::thread::spawn(move || {
                                let rt = tokio::runtime::Handle::try_current()
                                    .or_else(|_| {
                                        tokio::runtime::Runtime::new().map(|rt| rt.handle().clone())
                                    });
                                
                                match rt {
                                    Ok(handle) => {
                                        handle.block_on(async {
                                            db.delete_record(&collection_clone, &RecordId::from(record_id_clone.clone())).await
                                        })
                                    }
                                    Err(_) => {
                                        let rt = tokio::runtime::Runtime::new().unwrap();
                                        rt.block_on(async {
                                            db.delete_record(&collection_clone, &RecordId::from(record_id_clone)).await
                                        })
                                    }
                                }
                            }).join();

                            match result {
                                Ok(Ok(record)) => {
                                    let response = serde_json::json!({
                                        "success": true,
                                        "data": {
                                            "id": record.id,
                                            "collection": collection,
                                            "data": record.data,
                                            "created_at": record.created_at,
                                            "updated_at": record.updated_at
                                        }
                                    });

                                    let result_bytes = response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    info!("Deleted record {} from collection: {}", record.id, collection);
                                    0 // Success
                                }
                                Ok(Err(e)) => {
                                    error!("Failed to delete record: {}", e);
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": e.to_string()
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                                Err(_) => {
                                    error!("Database operation thread panicked");
                                    let error_response = serde_json::json!({
                                        "success": false,
                                        "error": "Database operation failed"
                                    });
                                    let result_bytes = error_response.to_string().into_bytes();
                                    let mut state = caller.data().lock().unwrap();
                                    state.db_result_buffer = result_bytes;
                                    -1 // Error
                                }
                            }
                        } else {
                            -1 // Error: no memory
                        }
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define delete_record: {}", e))
            })?;

        // get_http_request() -> i32 (returns length, copies data to plugin memory)
        self.linker
            .func_wrap(
                "env",
                host_functions::GET_HTTP_REQUEST,
                |mut caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                    let state = caller.data().lock().unwrap();
                    if let Some(request) = &state.current_http_request {
                        if let Ok(request_json) = serde_json::to_string(request) {
                            let request_bytes = request_json.as_bytes().to_vec();
                            drop(state);

                            // Get plugin memory and allocate space
                            if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                                if let Some(alloc_export) = caller.get_export("alloc") {
                                    if let Some(alloc_func_raw) = alloc_export.into_func() {
                                        if let Ok(alloc_func) = alloc_func_raw.typed::<i32, i32>(&mut caller) {
                                            if let Ok(ptr) = alloc_func.call(&mut caller, request_bytes.len() as i32) {
                                                let data = memory.data_mut(&mut caller);
                                                let start = ptr as usize;
                                                let end = start + request_bytes.len();

                                                if end <= data.len() {
                                                    data[start..end].copy_from_slice(&request_bytes);

                                                    let mut state = caller.data().lock().unwrap();
                                                    state.result_buffer = [
                                                        (ptr as u32).to_le_bytes().to_vec(),
                                                        (request_bytes.len() as u32).to_le_bytes().to_vec(),
                                                    ].concat();

                                                    return request_bytes.len() as i32;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    -1
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define get_http_request: {}", e))
            })?;

        // set_http_response(status_ptr: *const u8, headers_ptr: *const u8, headers_len: usize, body_ptr: *const u8, body_len: usize)
        self.linker
            .func_wrap(
                "env",
                host_functions::SET_HTTP_RESPONSE,
                |mut caller: Caller<'_, Arc<Mutex<HostState>>>, 
                 status_code: i32,
                 headers_ptr: i32, headers_len: i32,
                 body_ptr: i32, body_len: i32| -> i32 {
                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                        let data = memory.data(&caller);
                        
                        let headers_json = if headers_len > 0 {
                            match std::str::from_utf8(&data[headers_ptr as usize..(headers_ptr + headers_len) as usize]) {
                                Ok(s) => s,
                                Err(_) => return -1,
                            }
                        } else {
                            "{}"
                        };
                        
                        let body = if body_len > 0 {
                            match std::str::from_utf8(&data[body_ptr as usize..(body_ptr + body_len) as usize]) {
                                Ok(s) => s.to_string(),
                                Err(_) => return -1,
                            }
                        } else {
                            String::new()
                        };

                        let headers: std::collections::HashMap<String, String> = 
                            serde_json::from_str(headers_json).unwrap_or_default();

                        let response = oxide_core::plugin_api::HttpResponse {
                            status_code: status_code as u16,
                            headers,
                            body,
                        };

                        if let Ok(response_json) = serde_json::to_string(&response) {
                            let mut state = caller.data().lock().unwrap();
                            state.http_response_buffer = response_json.into_bytes();
                            info!("Set HTTP response: status {}", status_code);
                            0 // Success
                        } else {
                            -1 // Error serializing response
                        }
                    } else {
                        -1 // Error: no memory
                    }
                },
            )
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to define set_http_response: {}", e))
            })?;

        Ok(())
    }

    /// Get the current host state (for testing)
    pub fn get_host_state(&self) -> Arc<Mutex<HostState>> {
        self.store.data().clone()
    }

    /// Get the error message set by the plugin (for testing)
    pub fn get_plugin_error(&self) -> Option<String> {
        self.store.data().lock().unwrap().error_message.clone()
    }

    /// Get log messages from the plugin (for testing)
    pub fn get_plugin_logs(&self) -> Vec<String> {
        self.store.data().lock().unwrap().log_messages.clone()
    }

    /// Set the current event payload for plugin processing
    fn set_current_payload(&mut self, payload: &EventPayload) -> PluginResult<()> {
        let payload_json = serde_json::to_string(payload).map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to serialize payload: {}", e))
        })?;

        let mut state = self.store.data().lock().unwrap();
        state.current_payload = Some(payload_json.clone());
        state.result_buffer = payload_json.into_bytes();
        drop(state);

        Ok(())
    }

    /// Get plugin response from the plugin's response buffer
    fn get_plugin_response(&mut self, plugin_name: &str) -> PluginResult<PluginResponse> {
        let instance = self
            .instances
            .get(plugin_name)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

        // Get response length
        let get_response_len = instance
            .get_typed_func::<(), i32>(&mut self.store, "get_response_len")
            .map_err(|e| PluginError::FunctionNotExported(format!("get_response_len: {}", e)))?;

        let response_len = get_response_len.call(&mut self.store, ()).map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to get response length: {}", e))
        })?;

        if response_len <= 0 {
            // Return default response if no response data
            return Ok(PluginResponse::default());
        }

        // Get response pointer
        let get_response_ptr = instance
            .get_typed_func::<(), i32>(&mut self.store, "get_response_ptr")
            .map_err(|e| PluginError::FunctionNotExported(format!("get_response_ptr: {}", e)))?;

        let response_ptr = get_response_ptr.call(&mut self.store, ()).map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to get response pointer: {}", e))
        })?;

        // Read response from plugin memory
        let memory = instance
            .get_memory(&mut self.store, "memory")
            .ok_or_else(|| PluginError::ExecutionFailed("Plugin memory not found".to_string()))?;

        let data = memory.data(&self.store);
        let response_bytes = &data[response_ptr as usize..(response_ptr + response_len) as usize];
        let response_json = std::str::from_utf8(response_bytes)
            .map_err(|e| PluginError::InvalidResponse(format!("Invalid UTF-8: {}", e)))?;

        serde_json::from_str(response_json)
            .map_err(|e| PluginError::InvalidResponse(format!("Failed to parse response: {}", e)))
    }

    /// Load a plugin with specific trust level and capabilities
    pub fn load_plugin_with_trust(
        &mut self,
        name: &str,
        wasm_bytes: &[u8],
        trust_level: PluginTrustLevel,
        capabilities: Vec<PluginCapability>,
        _limits: ResourceLimits,
    ) -> PluginResult<()> {
        debug!("Loading plugin '{}' with trust level {:?}", name, trust_level);

        // Compile the module
        let module = Module::new(&self.engine, wasm_bytes).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to compile module: {}", e))
        })?;

        // Instantiate the module
        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to instantiate module: {}", e))
            })?;

        // Register plugin with specified security settings
        self.security_manager.register_plugin(
            name.to_string(),
            Some(trust_level.clone()),
        )
        .map_err(|e| PluginError::SecurityViolation(format!("Failed to register plugin: {:?}", e)))?;

        // Grant specified capabilities
        for capability in capabilities {
            self.security_manager.grant_capability(name, capability)
                .map_err(|e| PluginError::SecurityViolation(format!("Failed to grant capability: {:?}", e)))?;
        }

        // Store module and instance
        self.modules.insert(name.to_string(), module);
        self.instances.insert(name.to_string(), instance);

        info!("Plugin '{}' loaded successfully with trust level {:?}", name, trust_level);
        Ok(())
    }

    /// Grant a capability to a plugin
    pub fn grant_plugin_capability(
        &mut self,
        plugin_name: &str,
        capability: PluginCapability,
    ) -> PluginResult<()> {
        self.security_manager.grant_capability(plugin_name, capability)
            .map_err(|e| PluginError::SecurityViolation(format!("Failed to grant capability: {:?}", e)))
    }

    /// Revoke a capability from a plugin
    pub fn revoke_plugin_capability(
        &mut self,
        plugin_name: &str,
        capability: &PluginCapability,
    ) -> PluginResult<()> {
        self.security_manager.revoke_capability(plugin_name, capability)
            .map_err(|e| PluginError::SecurityViolation(format!("Failed to revoke capability: {:?}", e)))
    }

    /// Suspend a plugin due to security violations
    pub fn suspend_plugin(&mut self, plugin_name: &str, _reason: String) -> PluginResult<()> {
        self.security_manager.suspend_plugin(plugin_name)
            .map_err(|e| PluginError::SecurityViolation(format!("Failed to suspend plugin: {:?}", e)))?;
        info!("Plugin '{}' has been suspended", plugin_name);
        Ok(())
    }

    /// Resume a suspended plugin
    pub fn resume_plugin(&mut self, plugin_name: &str) -> PluginResult<()> {
        let _ = self.security_manager.resume_plugin(plugin_name);
        info!("Plugin '{}' has been resumed", plugin_name);
        Ok(())
    }

    /// Get security audit log for a plugin
    pub fn get_plugin_audit_log(&self, plugin_name: &str) -> Vec<SecurityAuditEntry> {
        self.security_manager.get_audit_log()
            .iter()
            .filter(|entry| entry.plugin_name == plugin_name)
            .cloned()
            .collect()
    }

    /// Check if a plugin has a specific capability
    pub fn plugin_has_capability(&self, plugin_name: &str, capability: &PluginCapability) -> bool {
        self.security_manager.has_capability(plugin_name, capability)
            .unwrap_or(false)
    }

    /// Get plugin execution statistics
    pub fn get_plugin_stats(&self, _plugin_name: &str) -> Option<ExecutionStats> {
        // TODO: Implement execution statistics tracking
        // For now, return None as statistics are not yet implemented
        None
    }

    /// Get all registered HTTP routes from plugins
    pub fn get_registered_routes(&self) -> Vec<oxide_core::plugin_api::RouteRegistration> {
        self.store.data().lock().unwrap().registered_routes.clone()
    }

    /// Handle an HTTP request for a plugin route
    pub fn handle_http_request(
        &mut self,
        plugin_name: &str,
        handler_function: &str,
        request: &oxide_core::plugin_api::HttpRequestContext,
    ) -> PluginResult<oxide_core::plugin_api::HttpResponse> {
        debug!("Handling HTTP request: {}::{}", plugin_name, handler_function);

        // Set the current HTTP request context
        {
            let mut state = self.store.data().lock().unwrap();
            state.current_http_request = Some(request.clone());
            state.http_response_buffer.clear();
            state.current_plugin = Some(plugin_name.to_string());
        }

        // Get the instance and call the handler function
        let instance = self
            .instances
            .get(plugin_name)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

        let func = instance
            .get_typed_func::<(), i32>(&mut self.store, handler_function)
            .map_err(|e| PluginError::FunctionNotExported(format!("{}: {}", handler_function, e)))?;

        let result = func
            .call(&mut self.store, ())
            .map_err(|e| PluginError::ExecutionFailed(format!("HTTP handler call failed: {}", e)))?;

        if result != 0 {
            return Err(PluginError::ExecutionFailed(format!(
                "HTTP handler returned error code: {}",
                result
            )));
        }

        // Get the HTTP response from the plugin
        let response_json = {
            let state = self.store.data().lock().unwrap();
            if state.http_response_buffer.is_empty() {
                // Return default response if plugin didn't set one
                serde_json::to_string(&oxide_core::plugin_api::HttpResponse::default())
                    .unwrap_or_else(|_| "{}".to_string())
            } else {
                String::from_utf8_lossy(&state.http_response_buffer).to_string()
            }
        };

        serde_json::from_str(&response_json).map_err(|e| {
            PluginError::InvalidResponse(format!("Failed to parse HTTP response: {}", e))
        })
    }

    /// Get database operation result from the plugin's result buffer
    pub fn get_db_result(&self) -> Option<serde_json::Value> {
        let state = self.store.data().lock().unwrap();
        if state.db_result_buffer.is_empty() {
            None
        } else {
            let result_json = String::from_utf8_lossy(&state.db_result_buffer);
            serde_json::from_str(&result_json).ok()
        }
    }

    /// Clear all state buffers
    pub fn clear_state(&mut self) {
        let mut state = self.store.data().lock().unwrap();
        state.current_http_request = None;
        state.http_response_buffer.clear();
        state.db_result_buffer.clear();
        state.log_messages.clear();
        state.error_message = None;
        state.current_plugin = None;
    }
}

impl PluginRuntime for WasmtimePluginRuntime {
    fn load_plugin(&mut self, name: &str, wasm_bytes: &[u8]) -> PluginResult<()> {
        debug!("Loading plugin: {}", name);

        // Compile the module
        let module = Module::new(&self.engine, wasm_bytes).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to compile module: {}", e))
        })?;

        // Instantiate the module
        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to instantiate module: {}", e))
            })?;

        // Register plugin with security manager
        // Default to Untrusted level for new plugins
        self.security_manager.register_plugin(
            name.to_string(),
            Some(PluginTrustLevel::Untrusted),
        ).map_err(|e| PluginError::SecurityViolation(e.to_string()))?;

        // Grant basic logging capabilities by default
        self.security_manager.grant_capability(
            name,
            PluginCapability::LogInfo,
        ).map_err(|e| PluginError::SecurityViolation(format!("Failed to grant logging capability: {:?}", e)))?;

        self.security_manager.grant_capability(
            name,
            PluginCapability::LogError,
        ).map_err(|e| PluginError::SecurityViolation(format!("Failed to grant logging capability: {:?}", e)))?;

        // Store module and instance
        self.modules.insert(name.to_string(), module);
        self.instances.insert(name.to_string(), instance);

        info!("Plugin '{}' loaded successfully with security context", name);
        Ok(())
    }

    fn call_plugin_function(
        &mut self,
        plugin_name: &str,
        function_name: &str,
        payload: &EventPayload,
    ) -> PluginResult<PluginResponse> {
        debug!(
            "Calling plugin function: {}::{}",
            plugin_name, function_name
        );

        // Validate function call capability
        let required_capability = match function_name {
            "on_before_create" | "on_after_create" | "on_before_update" | "on_after_update" |
            "on_before_delete" | "on_after_delete" => PluginCapability::AccessCollection {
                collection: "*".to_string(),
                operations: vec![CrudOperation::Create, CrudOperation::Read, CrudOperation::Update, CrudOperation::Delete],
            },
            _ => PluginCapability::LogInfo, // Default capability for unknown functions
        };

        if !self.security_manager.has_capability(plugin_name, &required_capability)
            .map_err(|e| PluginError::SecurityViolation(e.to_string()))? {
            let violation = SecurityViolation::UnauthorizedHostFunction {
                function_name: function_name.to_string(),
                required_capability: required_capability.clone(),
            };
            let _ = self.security_manager.record_violation(plugin_name, violation);
            return Err(PluginError::SecurityViolation(
                format!("Plugin '{}' lacks required capability for function '{}'", plugin_name, function_name)
            ));
        }

        // Set the current payload for the plugin to access
        self.set_current_payload(payload)?;

        // Clear previous state and set current plugin context
        {
            let mut state = self.store.data().lock().unwrap();
            state.log_messages.clear();
            state.error_message = None;
            state.current_plugin = Some(plugin_name.to_string());
        }

        // Get the instance and call the function
        let instance = self
            .instances
            .get(plugin_name)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

        // Call the plugin function
        let func = instance
            .get_typed_func::<(), i32>(&mut self.store, function_name)
            .map_err(|e| PluginError::FunctionNotExported(format!("{}: {}", function_name, e)))?;

        let result = func
            .call(&mut self.store, ())
            .map_err(|e| PluginError::ExecutionFailed(format!("Function call failed: {}", e)))?;

        // Check if plugin set an error
        let state = self.store.data().lock().unwrap();
        if let Some(error_msg) = &state.error_message {
            debug!("Plugin set error: {}", error_msg);
        }

        drop(state);

        // Get the plugin response
        let response = self.get_plugin_response(plugin_name)?;

        debug!("Plugin function call completed with result: {}", result);
        Ok(response)
    }

    fn has_function(&self, plugin_name: &str, _function_name: &str) -> bool {
        // For now, we'll assume the function exists if the plugin is loaded
        // A more robust implementation would check the exports
        self.instances.contains_key(plugin_name)
    }

    fn unload_plugin(&mut self, plugin_name: &str) -> PluginResult<()> {
        // Remove from security manager
        self.security_manager.unregister_plugin(plugin_name);
        
        // Remove module and instance
        self.modules.remove(plugin_name);
        self.instances.remove(plugin_name);
        
        info!("Plugin '{}' unloaded and security context cleared", plugin_name);
        Ok(())
    }

    fn list_plugins(&self) -> Vec<String> {
        self.instances.keys().cloned().collect()
    }

    fn runtime_name(&self) -> &'static str {
        "wasmtime"
    }

    fn runtime_version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }
} 