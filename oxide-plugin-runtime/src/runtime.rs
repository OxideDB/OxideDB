//! Wasmtime-based Plugin Runtime Implementation
//!
//! This module implements the PluginRuntime trait using Wasmtime,
//! providing plugin loading, execution, and security management.

use crate::host_functions::{
    define_database_functions, define_event_functions,
    define_http_functions, define_logging_functions, register_vfs_functions
};
use crate::host_state::{HostState, ExecutionContext};
use oxide_core::{
    plugin_api::{EventPayload, PluginError, PluginResponse, PluginResult, PluginRuntime},
    plugin_security::{
        PluginSecurityManager, PluginCapability, PluginTrustLevel, SecurityPolicies,
        SecurityViolation, ResourceLimits, ExecutionStats, SecurityAuditEntry
    },
    CrudOperation,
};
use oxide_db::Db;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info};
use wasmtime::{Engine, Instance, Linker, Module, Store};

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

    /// Define all host functions that plugins can import using modular approach
    fn define_host_functions(&mut self) -> PluginResult<()> {
        // Define event-related host functions
        define_event_functions(&mut self.linker)?;

        // Define logging-related host functions
        define_logging_functions(&mut self.linker)?;

        // Define HTTP-related host functions
        define_http_functions(&mut self.linker)?;

        // Define database-related host functions
        define_database_functions(&mut self.linker, self.database.clone())?;

        // Define VFS-related host functions
        register_vfs_functions(&mut self.linker)
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to register VFS functions: {}", e)))?;

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
    pub fn get_plugin_stats(&self, plugin_name: &str) -> Option<ExecutionStats> {
        self.security_manager.get_execution_stats(plugin_name)
    }

    /// Check whether a plugin has been suspended by the security manager.
    pub fn is_plugin_suspended(&self, plugin_name: &str) -> bool {
        self.security_manager
            .get_context(plugin_name)
            .map(|context| context.suspended)
            .unwrap_or(false)
    }

    /// Reset per-execution telemetry before entering plugin code.
    fn reset_execution_metrics(&mut self) {
        let mut state = self.store.data().lock().unwrap();
        state.current_execution_host_calls = 0;
    }

    /// Record elapsed execution telemetry after a plugin call completes.
    fn record_execution_metrics(&mut self, plugin_name: &str, started_at: Instant, failed: bool) {
        let execution_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let host_function_calls = self.store.data().lock().unwrap().current_execution_host_calls;
        let peak_memory_usage = self.get_plugin_memory_usage(plugin_name);

        if let Err(e) = self.security_manager.record_execution(
            plugin_name,
            execution_time_ms,
            failed,
            host_function_calls,
            peak_memory_usage,
        ) {
            debug!("Failed to record execution metrics for plugin '{}': {}", plugin_name, e);
        }
    }

    /// Return the current WebAssembly memory size for a plugin.
    fn get_plugin_memory_usage(&mut self, plugin_name: &str) -> u64 {
        self.instances
            .get(plugin_name)
            .and_then(|instance| instance.get_memory(&mut self.store, "memory"))
            .map(|memory| memory.data_size(&self.store) as u64)
            .unwrap_or(0)
    }

    /// Get all registered HTTP routes from plugins
    pub fn get_registered_routes(&self) -> Vec<oxide_core::plugin_api::RouteRegistration> {
        self.store.data().lock().unwrap().registered_routes.clone()
    }

    /// Handle an HTTP request for a plugin route
    pub fn handle_http_request(
        &mut self,
        plugin_name: &str,
        _handler_function: &str, // Not used anymore, kept for backward compatibility
        request: &oxide_core::plugin_api::HttpRequestContext,
    ) -> PluginResult<oxide_core::plugin_api::HttpResponse> {
        let started_at = Instant::now();
        self.reset_execution_metrics();
        let result = self.handle_http_request_inner(plugin_name, request);
        self.record_execution_metrics(plugin_name, started_at, result.is_err());
        result
    }

    fn handle_http_request_inner(
        &mut self,
        plugin_name: &str,
        request: &oxide_core::plugin_api::HttpRequestContext,
    ) -> PluginResult<oxide_core::plugin_api::HttpResponse> {
        debug!("Handling HTTP request: {}::{}", plugin_name, "handle_http_request");

        // Set the current HTTP request context
        {
            let mut state = self.store.data().lock().unwrap();
            state.current_http_request = Some(request.clone());
            state.http_response_buffer.clear();
            state.current_plugin = Some(plugin_name.to_string());
            state.set_execution_context(ExecutionContext::HttpRequest);
        }

        // Get the instance and call the generic HTTP handler function
        let instance = self
            .instances
            .get(plugin_name)
            .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

        // Call the generic handle_http_request function that the SDK exports
        let func = instance
            .get_typed_func::<(), i32>(&mut self.store, "handle_http_request")
            .map_err(|e| PluginError::FunctionNotExported(format!("handle_http_request: {}", e)))?;

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
        if state.result_buffer.len() >= 8 {
            // For database operations, read the length and then access plugin memory
            let len = u32::from_le_bytes([
                state.result_buffer[4],
                state.result_buffer[5],
                state.result_buffer[6],
                state.result_buffer[7],
            ]) as usize;

            if len > 0 {
                // Note: This is a simplified version. In practice, we'd need to access
                // the plugin memory at the stored pointer, but for testing purposes
                // we can return a success indicator
                Some(serde_json::json!({"status": "success", "data_length": len}))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Clear all state buffers
    pub fn clear_state(&mut self) {
        let mut state = self.store.data().lock().unwrap();
        state.current_http_request = None;
        state.http_response_buffer.clear();
        state.result_buffer.clear();
        state.log_messages.clear();
        state.error_message = None;
        state.current_plugin = None;
        state.set_execution_context(ExecutionContext::Idle);
        state.exit_database_operation();
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
        let started_at = Instant::now();
        self.reset_execution_metrics();

        let result = (|| {
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

                // Preserve HTTP request context when switching to event handler
                // This allows database operations during HTTP request event handling
                if state.current_http_request.is_some() {
                    // We're in an HTTP request context, so keep that context active
                    // even when handling events triggered by the HTTP handler
                    debug!("Preserving HTTP context during event handling for plugin: {}", plugin_name);
                } else {
                    // Only set to EventHandler if we're not in an HTTP request
                    debug!("Setting execution context to EventHandler for plugin: {}", plugin_name);
                    state.set_execution_context(ExecutionContext::EventHandler);
                }
            }

            debug!("About to get plugin instance and call function: {}::{}", plugin_name, function_name);

            // Get the instance and call the function
            let instance = self
                .instances
                .get(plugin_name)
                .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

            debug!("Got plugin instance, about to get typed function: {}", function_name);

            // Call the plugin function
            let func = instance
                .get_typed_func::<(), i32>(&mut self.store, function_name)
                .map_err(|e| PluginError::FunctionNotExported(format!("{}: {}", function_name, e)))?;

            debug!("Got typed function, about to call plugin function: {}::{}", plugin_name, function_name);

            let result = func
                .call(&mut self.store, ())
                .map_err(|e| PluginError::ExecutionFailed(format!("Function call failed: {}", e)))?;

            debug!("Plugin function call completed with result: {} for {}::{}", result, plugin_name, function_name);

            // Check if plugin set an error
            let state = self.store.data().lock().unwrap();
            if let Some(error_msg) = &state.error_message {
                debug!("Plugin set error: {}", error_msg);
            }

            drop(state);

            // Get the plugin response
            let response = self.get_plugin_response(plugin_name)?;

            Ok(response)
        })();

        self.record_execution_metrics(plugin_name, started_at, result.is_err());
        result
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

        // Clean up registered routes for this plugin
        {
            let mut state = self.store.data().lock().unwrap();
            let original_count = state.registered_routes.len();
            state.registered_routes.retain(|route| route.plugin_name != plugin_name);
            let removed_count = original_count - state.registered_routes.len();
            if removed_count > 0 {
                info!("Cleaned up {} registered routes for plugin '{}'", removed_count, plugin_name);
            }
        }

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
