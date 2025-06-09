//! Wasmtime-based Plugin Runtime for OxideDB
//!
//! This module implements the plugin runtime using Wasmtime, providing:
//! - Host function implementations that plugins can call
//! - Plugin loading and execution management
//! - Memory management for host-plugin communication

use oxide_core::{
    plugin_api::{host_functions, EventPayload, PluginError, PluginResponse, PluginResult, PluginRuntime},
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, error, info, warn};
use wasmtime::{Caller, Engine, Instance, Linker, Module, Store};

/// Shared state between host and plugin for communication
#[derive(Debug, Clone)]
struct HostState {
    /// Current event payload being processed
    current_payload: Option<String>,

    /// Messages logged by plugins
    log_messages: Vec<String>,

    /// Error message set by plugin (if any)
    error_message: Option<String>,

    /// Result buffer for host function returns
    result_buffer: Vec<u8>,
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            current_payload: None,
            log_messages: Vec::new(),
            error_message: None,
            result_buffer: Vec::new(),
        }
    }
}

/// Wasmtime-based implementation of the PluginRuntime trait
pub struct WasmtimePluginRuntime {
    engine: Engine,
    linker: Linker<Arc<Mutex<HostState>>>,
    store: Store<Arc<Mutex<HostState>>>,
    modules: HashMap<String, Module>,
    instances: HashMap<String, Instance>,
}

impl WasmtimePluginRuntime {
    /// Create a new Wasmtime plugin runtime
    pub fn new() -> PluginResult<Self> {
        let engine = Engine::default();
        let linker = Linker::new(&engine);
        let host_state = Arc::new(Mutex::new(HostState::default()));
        let store = Store::new(&engine, host_state);

        // Define host functions that plugins can call
        let mut runtime = Self {
            engine,
            linker,
            store,
            modules: HashMap::new(),
            instances: HashMap::new(),
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
                                                state.result_buffer = vec![
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
                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    {
                        let data = memory.data(&caller);
                        if let Ok(message) =
                            std::str::from_utf8(&data[ptr as usize..(ptr + len) as usize])
                        {
                            info!("[PLUGIN] {}", message);
                            caller
                                .data()
                                .lock()
                                .unwrap()
                                .log_messages
                                .push(message.to_string());
                        }
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
                    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory())
                    {
                        let data = memory.data(&caller);
                        if let Ok(message) =
                            std::str::from_utf8(&data[ptr as usize..(ptr + len) as usize])
                        {
                            error!("[PLUGIN] {}", message);
                            caller
                                .data()
                                .lock()
                                .unwrap()
                                .log_messages
                                .push(format!("ERROR: {}", message));
                        }
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

        // Store module and instance
        self.modules.insert(name.to_string(), module);
        self.instances.insert(name.to_string(), instance);

        info!("Plugin '{}' loaded successfully", name);
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

        // Set the current payload for the plugin to access
        self.set_current_payload(payload)?;

        // Clear previous state
        {
            let mut state = self.store.data().lock().unwrap();
            state.log_messages.clear();
            state.error_message = None;
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

    fn has_function(&self, plugin_name: &str, function_name: &str) -> bool {
        // For now, we'll assume the function exists if the plugin is loaded
        // A more robust implementation would check the exports
        self.instances.contains_key(plugin_name)
    }

    fn unload_plugin(&mut self, plugin_name: &str) -> PluginResult<()> {
        self.modules.remove(plugin_name);
        self.instances.remove(plugin_name);
        info!("Plugin '{}' unloaded", plugin_name);
        Ok(())
    }

    fn list_plugins(&self) -> Vec<String> {
        self.instances.keys().cloned().collect()
    }
}
