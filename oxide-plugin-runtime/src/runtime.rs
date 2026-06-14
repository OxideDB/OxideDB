//! Wasmtime-based Plugin Runtime Implementation
//!
//! This module implements the PluginRuntime trait using Wasmtime,
//! providing plugin loading, execution, and security management.

use crate::host_functions::{
    define_database_functions, define_event_functions, define_http_functions,
    define_logging_functions, register_vfs_functions,
};
use crate::host_state::{ExecutionContext, HostState, PluginStoreData};
use oxide_core::{
    plugin_api::{
        plugin_exports, EventPayload, PluginError, PluginResponse, PluginResult, PluginRuntime,
    },
    plugin_security::{
        ExecutionStats, PluginCapability, PluginSecurityManager, PluginTrustLevel, ResourceLimits,
        SecurityAuditEntry, SecurityPolicies, SecurityViolation,
    },
};
use oxide_db::Db;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tracing::{debug, error, info, warn};
use wasmtime::{Config, Engine, ExternType, Instance, Linker, Module, Store};

const DEFAULT_PLUGIN_FUEL_PER_MILLISECOND: u64 = 10_000;

/// Wasmtime-based implementation of the PluginRuntime trait
pub struct WasmtimePluginRuntime {
    engine: Engine,
    linker: Linker<PluginStoreData>,
    store: Store<PluginStoreData>,
    modules: HashMap<String, Module>,
    instances: HashMap<String, Instance>,
    security_manager: PluginSecurityManager,
    database: Arc<dyn Db>,
}

fn configured_engine() -> PluginResult<Engine> {
    let mut config = Config::new();
    config.consume_fuel(true);
    Engine::new(&config).map_err(|e| {
        PluginError::InitializationFailed(format!("Failed to initialize Wasmtime engine: {}", e))
    })
}

fn configured_store(engine: &Engine) -> Store<PluginStoreData> {
    let host_state = Arc::new(Mutex::new(HostState::default()));
    let mut store = Store::new(engine, PluginStoreData::new(host_state));
    store.limiter(|state| state);
    store
}

impl WasmtimePluginRuntime {
    /// Create a new Wasmtime plugin runtime with database
    pub fn new(database: Arc<dyn Db>) -> PluginResult<Self> {
        let engine = configured_engine()?;
        let linker = Linker::new(&engine);
        let store = configured_store(&engine);

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
    pub fn new_with_security_policies(
        database: Arc<dyn Db>,
        policies: SecurityPolicies,
    ) -> PluginResult<Self> {
        let engine = configured_engine()?;
        let linker = Linker::new(&engine);
        let store = configured_store(&engine);

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

    /// Get the security policies applied to this runtime.
    pub fn security_policies(&self) -> &SecurityPolicies {
        self.security_manager.policies()
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
        register_vfs_functions(&mut self.linker).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to register VFS functions: {}", e))
        })?;

        Ok(())
    }

    fn host_state(&self, action: &str) -> PluginResult<MutexGuard<'_, HostState>> {
        self.store.data().host_state().lock().map_err(|_| {
            PluginError::ExecutionFailed(format!(
                "Plugin host state lock was poisoned while {}",
                action
            ))
        })
    }

    fn try_host_state(&self, action: &str) -> Option<MutexGuard<'_, HostState>> {
        match self.store.data().host_state().lock() {
            Ok(state) => Some(state),
            Err(_) => {
                error!("Plugin host state lock was poisoned while {}", action);
                None
            }
        }
    }

    /// Get the current host state (for testing)
    pub fn get_host_state(&self) -> Arc<Mutex<HostState>> {
        self.store.data().host_state().clone()
    }

    /// Get the error message set by the plugin (for testing)
    pub fn get_plugin_error(&self) -> Option<String> {
        self.try_host_state("reading plugin error")
            .and_then(|state| state.error_message.clone())
    }

    /// Get log messages from the plugin (for testing)
    pub fn get_plugin_logs(&self) -> Vec<String> {
        self.try_host_state("reading plugin logs")
            .map(|state| state.log_messages.clone())
            .unwrap_or_default()
    }

    /// Set the current event payload for plugin processing
    fn set_current_payload(&mut self, payload: &EventPayload) -> PluginResult<()> {
        let payload_json = serde_json::to_string(payload).map_err(|e| {
            PluginError::ExecutionFailed(format!("Failed to serialize payload: {}", e))
        })?;

        let mut state = self.host_state("setting current event payload")?;
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
        let response_start = usize::try_from(response_ptr).map_err(|_| {
            PluginError::InvalidResponse(format!(
                "Plugin returned negative response pointer: {}",
                response_ptr
            ))
        })?;
        let response_len = usize::try_from(response_len).map_err(|_| {
            PluginError::InvalidResponse(format!(
                "Plugin returned invalid response length: {}",
                response_len
            ))
        })?;
        let response_end = response_start.checked_add(response_len).ok_or_else(|| {
            PluginError::InvalidResponse(
                "Plugin response pointer overflowed memory bounds".to_string(),
            )
        })?;
        let response_bytes = data.get(response_start..response_end).ok_or_else(|| {
            PluginError::InvalidResponse(format!(
                "Plugin response range {}..{} exceeds memory size {}",
                response_start,
                response_end,
                data.len()
            ))
        })?;
        let response_json = std::str::from_utf8(response_bytes)
            .map_err(|e| PluginError::InvalidResponse(format!("Invalid UTF-8: {}", e)))?;

        serde_json::from_str(response_json)
            .map_err(|e| PluginError::InvalidResponse(format!("Failed to parse response: {}", e)))
    }

    fn initialize_plugin(&mut self, name: &str) -> PluginResult<()> {
        let result = (|| {
            self.reset_execution_metrics()?;
            self.reset_execution_budget(name)?;
            let instance = self
                .instances
                .get(name)
                .ok_or_else(|| PluginError::PluginNotFound(name.to_string()))?;

            {
                let mut state = self.host_state("preparing plugin initialization")?;
                state.current_plugin = Some(name.to_string());
                state.current_http_request = None;
                state.error_message = None;
                state.set_execution_context(ExecutionContext::Idle);
            }

            let init = instance
                .get_typed_func::<(), i32>(&mut self.store, plugin_exports::PLUGIN_INIT)
                .map_err(|e| {
                    PluginError::FunctionNotExported(format!(
                        "{}: {}",
                        plugin_exports::PLUGIN_INIT,
                        e
                    ))
                })?;

            let code = init.call(&mut self.store, ()).map_err(|e| {
                PluginError::InitializationFailed(format!(
                    "Plugin '{}' initialization failed: {}",
                    name, e
                ))
            })?;
            self.ensure_no_host_security_error()?;

            if code != 0 {
                return Err(PluginError::InitializationFailed(format!(
                    "Plugin '{}' initialization returned non-zero status {}",
                    name, code
                )));
            }

            Ok(())
        })();

        let cleanup_result = self
            .host_state("clearing plugin initialization context")
            .map(|mut state| {
                state.current_plugin = None;
                state.set_execution_context(ExecutionContext::Idle);
            });

        match (result, cleanup_result) {
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    fn remove_plugin_runtime_state(&mut self, plugin_name: &str) {
        self.security_manager.unregister_plugin(plugin_name);
        self.modules.remove(plugin_name);
        self.instances.remove(plugin_name);

        if let Some(mut state) = self.try_host_state("removing plugin runtime state") {
            state
                .registered_routes
                .retain(|route| route.plugin_name != plugin_name);
            state.remove_plugin_capabilities(plugin_name);
        }
    }

    fn cleanup_plugin(&mut self, plugin_name: &str) -> PluginResult<()> {
        if !self.has_function(plugin_name, plugin_exports::PLUGIN_CLEANUP) {
            debug!(
                "Plugin '{}' does not export '{}'; skipping cleanup",
                plugin_name,
                plugin_exports::PLUGIN_CLEANUP
            );
            return Ok(());
        }

        let result = (|| {
            self.reset_execution_metrics()?;
            self.reset_execution_budget(plugin_name)?;
            let instance = self
                .instances
                .get(plugin_name)
                .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

            {
                let mut state = self.host_state("preparing plugin cleanup")?;
                state.current_plugin = Some(plugin_name.to_string());
                state.current_http_request = None;
                state.error_message = None;
                state.set_execution_context(ExecutionContext::Idle);
            }

            let cleanup = instance
                .get_typed_func::<(), i32>(&mut self.store, plugin_exports::PLUGIN_CLEANUP)
                .map_err(|e| {
                    PluginError::FunctionNotExported(format!(
                        "{}: {}",
                        plugin_exports::PLUGIN_CLEANUP,
                        e
                    ))
                })?;

            let code = cleanup.call(&mut self.store, ()).map_err(|e| {
                PluginError::ExecutionFailed(format!(
                    "Plugin '{}' cleanup failed: {}",
                    plugin_name, e
                ))
            })?;
            self.ensure_no_host_security_error()?;

            if code != 0 {
                return Err(PluginError::ExecutionFailed(format!(
                    "Plugin '{}' cleanup returned non-zero status {}",
                    plugin_name, code
                )));
            }

            Ok(())
        })();

        let cleanup_result = self
            .host_state("clearing plugin cleanup context")
            .map(|mut state| {
                state.current_plugin = None;
                state.current_http_request = None;
                state.error_message = None;
                state.set_execution_context(ExecutionContext::Idle);
                state.exit_database_operation();
            });

        match (result, cleanup_result) {
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Ok(()), Ok(())) => Ok(()),
        }
    }

    fn sync_plugin_capabilities_to_host_state(&mut self, plugin_name: &str) {
        let capabilities = self
            .security_manager
            .get_context(plugin_name)
            .map(|context| context.capabilities.iter().cloned().collect())
            .unwrap_or_default();

        if let Some(mut state) = self.try_host_state("syncing plugin capabilities") {
            state.set_plugin_capabilities(plugin_name.to_string(), capabilities);
        }
    }

    /// Load a plugin with specific trust level and capabilities
    pub fn load_plugin_with_trust(
        &mut self,
        name: &str,
        wasm_bytes: &[u8],
        trust_level: PluginTrustLevel,
        capabilities: Vec<PluginCapability>,
        limits: ResourceLimits,
    ) -> PluginResult<()> {
        debug!(
            "Loading plugin '{}' with trust level {:?}",
            name, trust_level
        );

        // Compile the module
        let module = Module::new(&self.engine, wasm_bytes).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to compile module: {}", e))
        })?;

        self.set_active_memory_limit(&limits);

        // Instantiate the module
        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to instantiate module: {}", e))
            })?;

        if self.instances.contains_key(name) {
            if let Err(error) = self.cleanup_plugin(name) {
                warn!(
                    "Plugin '{}' cleanup failed before reload; replacing runtime state anyway: {}",
                    name, error
                );
            }
        }
        self.remove_plugin_runtime_state(name);

        // Register plugin with specified security settings
        self.security_manager
            .register_plugin(name.to_string(), Some(trust_level.clone()))
            .map_err(|e| {
                PluginError::SecurityViolation(format!("Failed to register plugin: {:?}", e))
            })?;
        self.security_manager
            .set_resource_limits(name, limits)
            .map_err(|e| {
                PluginError::SecurityViolation(format!("Failed to apply resource limits: {:?}", e))
            })?;

        // Grant specified capabilities
        for capability in capabilities {
            self.security_manager
                .grant_capability(name, capability)
                .map_err(|e| {
                    PluginError::SecurityViolation(format!("Failed to grant capability: {:?}", e))
                })?;
        }

        self.sync_plugin_capabilities_to_host_state(name);

        // Store module and instance
        self.modules.insert(name.to_string(), module);
        self.instances.insert(name.to_string(), instance);

        if let Err(e) = self.initialize_plugin(name) {
            self.remove_plugin_runtime_state(name);
            return Err(e);
        }

        info!(
            "Plugin '{}' loaded successfully with trust level {:?}",
            name, trust_level
        );
        Ok(())
    }

    /// Grant a capability to a plugin
    pub fn grant_plugin_capability(
        &mut self,
        plugin_name: &str,
        capability: PluginCapability,
    ) -> PluginResult<()> {
        self.security_manager
            .grant_capability(plugin_name, capability)
            .map_err(|e| {
                PluginError::SecurityViolation(format!("Failed to grant capability: {:?}", e))
            })?;

        self.sync_plugin_capabilities_to_host_state(plugin_name);
        Ok(())
    }

    /// Revoke a capability from a plugin
    pub fn revoke_plugin_capability(
        &mut self,
        plugin_name: &str,
        capability: &PluginCapability,
    ) -> PluginResult<()> {
        self.security_manager
            .revoke_capability(plugin_name, capability)
            .map_err(|e| {
                PluginError::SecurityViolation(format!("Failed to revoke capability: {:?}", e))
            })?;

        self.sync_plugin_capabilities_to_host_state(plugin_name);
        Ok(())
    }

    /// Suspend a plugin due to security violations
    pub fn suspend_plugin(&mut self, plugin_name: &str, _reason: String) -> PluginResult<()> {
        self.security_manager
            .suspend_plugin(plugin_name)
            .map_err(|e| {
                PluginError::SecurityViolation(format!("Failed to suspend plugin: {:?}", e))
            })?;
        info!("Plugin '{}' has been suspended", plugin_name);
        Ok(())
    }

    /// Resume a suspended plugin
    pub fn resume_plugin(&mut self, plugin_name: &str) -> PluginResult<()> {
        self.security_manager
            .resume_plugin(plugin_name)
            .map_err(|e| {
                PluginError::SecurityViolation(format!("Failed to resume plugin: {:?}", e))
            })?;
        info!("Plugin '{}' has been resumed", plugin_name);
        Ok(())
    }

    /// Get security audit log for a plugin
    pub fn get_plugin_audit_log(&self, plugin_name: &str) -> Vec<SecurityAuditEntry> {
        self.security_manager
            .get_audit_log()
            .iter()
            .filter(|entry| entry.plugin_name == plugin_name)
            .cloned()
            .collect()
    }

    /// Check if a plugin has a specific capability
    pub fn plugin_has_capability(&self, plugin_name: &str, capability: &PluginCapability) -> bool {
        self.security_manager
            .has_capability(plugin_name, capability)
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

    fn require_plugin_capability(
        &mut self,
        plugin_name: &str,
        function_name: &str,
        required_capability: PluginCapability,
    ) -> PluginResult<()> {
        let has_capability = self
            .security_manager
            .has_capability(plugin_name, &required_capability)
            .map_err(|e| PluginError::SecurityViolation(e.to_string()))?;

        if !has_capability {
            let violation = SecurityViolation::UnauthorizedHostFunction {
                function_name: function_name.to_string(),
                required_capability: required_capability.clone(),
            };
            let _ = self
                .security_manager
                .record_violation(plugin_name, violation);

            return Err(PluginError::SecurityViolation(format!(
                "Plugin '{}' lacks required capability {:?} for function '{}'",
                plugin_name, required_capability, function_name
            )));
        }

        Ok(())
    }

    fn validate_event_response_capabilities(
        &mut self,
        plugin_name: &str,
        function_name: &str,
        response: &PluginResponse,
    ) -> PluginResult<()> {
        if response.modified_data.is_some() {
            self.require_plugin_capability(
                plugin_name,
                function_name,
                PluginCapability::ModifyEventData,
            )?;
        }

        if function_name.starts_with("on_before") && !response.allow {
            self.require_plugin_capability(
                plugin_name,
                function_name,
                PluginCapability::BlockOperations,
            )?;
        }

        Ok(())
    }

    /// Reset per-execution telemetry before entering plugin code.
    fn reset_execution_metrics(&mut self) -> PluginResult<()> {
        let mut state = self.host_state("resetting plugin execution metrics")?;
        state.current_execution_host_calls = 0;
        state.host_security_error = None;
        state.result_buffer.clear();
        state.db_result_buffer.clear();
        state.clear_function_results();
        Ok(())
    }

    fn resource_limits_for_plugin(&self, plugin_name: &str) -> ResourceLimits {
        self.security_manager
            .get_context(plugin_name)
            .map(|context| context.resource_limits.clone())
            .unwrap_or_else(|| {
                self.security_manager
                    .policies()
                    .default_resource_limits
                    .clone()
            })
    }

    fn set_active_memory_limit(&mut self, limits: &ResourceLimits) {
        self.store
            .data_mut()
            .set_max_memory_bytes(limits.max_memory);
    }

    fn reset_execution_budget(&mut self, plugin_name: &str) -> PluginResult<()> {
        let limits = self.resource_limits_for_plugin(plugin_name);
        self.set_active_memory_limit(&limits);
        {
            let mut state = self.host_state("setting plugin host call budget")?;
            state.set_max_host_calls_per_execution(limits.max_host_calls);
        }
        let fuel = limits
            .max_execution_time
            .saturating_mul(DEFAULT_PLUGIN_FUEL_PER_MILLISECOND)
            .max(DEFAULT_PLUGIN_FUEL_PER_MILLISECOND);

        self.store
            .set_fuel(fuel)
            .map_err(|e| PluginError::ExecutionFailed(format!("Failed to set plugin fuel: {}", e)))
    }

    fn ensure_no_host_security_error(&self) -> PluginResult<()> {
        let state = self.host_state("checking plugin host security state")?;
        if let Some(message) = &state.host_security_error {
            return Err(PluginError::SecurityViolation(message.clone()));
        }

        Ok(())
    }

    /// Record elapsed execution telemetry after a plugin call completes.
    fn record_execution_metrics(&mut self, plugin_name: &str, started_at: Instant, failed: bool) {
        let execution_time_ms = started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        let host_function_calls = self
            .try_host_state("recording plugin execution metrics")
            .map(|state| state.current_execution_host_calls)
            .unwrap_or_default();
        let peak_memory_usage = self.get_plugin_memory_usage(plugin_name);

        if let Err(e) = self.security_manager.record_execution(
            plugin_name,
            execution_time_ms,
            failed,
            host_function_calls,
            peak_memory_usage,
        ) {
            debug!(
                "Failed to record execution metrics for plugin '{}': {}",
                plugin_name, e
            );
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
        self.try_host_state("reading registered plugin HTTP routes")
            .map(|state| state.registered_routes.clone())
            .unwrap_or_default()
    }

    /// Handle an HTTP request for a plugin route
    pub fn handle_http_request(
        &mut self,
        plugin_name: &str,
        _handler_function: &str, // Not used anymore, kept for backward compatibility
        request: &oxide_core::plugin_api::HttpRequestContext,
    ) -> PluginResult<oxide_core::plugin_api::HttpResponse> {
        let started_at = Instant::now();
        self.reset_execution_metrics()?;
        let result = self.handle_http_request_inner(plugin_name, request);
        self.record_execution_metrics(plugin_name, started_at, result.is_err());
        self.clear_http_request_context();
        result
    }

    fn handle_http_request_inner(
        &mut self,
        plugin_name: &str,
        request: &oxide_core::plugin_api::HttpRequestContext,
    ) -> PluginResult<oxide_core::plugin_api::HttpResponse> {
        debug!(
            "Handling HTTP request: {}::{}",
            plugin_name, "handle_http_request"
        );
        self.reset_execution_budget(plugin_name)?;

        if self.is_plugin_suspended(plugin_name) {
            return Err(PluginError::SecurityViolation(format!(
                "Plugin '{}' is suspended",
                plugin_name
            )));
        }

        let can_handle_http = self
            .security_manager
            .has_capability(plugin_name, &PluginCapability::HandleHttpRequests)
            .map_err(|e| PluginError::SecurityViolation(e.to_string()))?;

        if !can_handle_http {
            return Err(PluginError::SecurityViolation(format!(
                "Plugin '{}' lacks HandleHttpRequests capability",
                plugin_name
            )));
        }

        // Set the current HTTP request context
        {
            let mut state = self.host_state("setting plugin HTTP request context")?;
            state.current_http_request = Some(request.clone());
            state.http_response_buffer.clear();
            state.error_message = None;
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

        let result = func.call(&mut self.store, ()).map_err(|e| {
            PluginError::ExecutionFailed(format!("HTTP handler call failed: {}", e))
        })?;
        self.ensure_no_host_security_error()?;

        if result != 0 {
            return Err(PluginError::ExecutionFailed(format!(
                "HTTP handler returned error code: {}",
                result
            )));
        }

        // Get the HTTP response from the plugin
        let response_json = {
            let state = self.host_state("reading plugin HTTP response")?;
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

    fn clear_http_request_context(&mut self) {
        if let Some(mut state) = self.try_host_state("clearing plugin HTTP request context") {
            state.current_http_request = None;
            state.http_response_buffer.clear();
            state.current_plugin = None;
            state.host_security_error = None;
            state.set_execution_context(ExecutionContext::Idle);
            state.exit_database_operation();
        }
    }

    /// Get database operation result from the plugin's result buffer
    pub fn get_db_result(&self) -> Option<serde_json::Value> {
        let state = self.try_host_state("reading plugin database result")?;
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
        if let Some(mut state) = self.try_host_state("clearing plugin runtime state") {
            state.current_http_request = None;
            state.http_response_buffer.clear();
            state.result_buffer.clear();
            state.db_result_buffer.clear();
            state.log_messages.clear();
            state.error_message = None;
            state.host_security_error = None;
            state.current_plugin = None;
            state.clear_function_results();
            state.set_execution_context(ExecutionContext::Idle);
            state.exit_database_operation();
        }
    }
}

impl PluginRuntime for WasmtimePluginRuntime {
    fn load_plugin(&mut self, name: &str, wasm_bytes: &[u8]) -> PluginResult<()> {
        debug!("Loading plugin: {}", name);

        // Compile the module
        let module = Module::new(&self.engine, wasm_bytes).map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to compile module: {}", e))
        })?;

        let limits = self
            .security_manager
            .policies()
            .default_resource_limits
            .clone();
        self.set_active_memory_limit(&limits);

        // Instantiate the module
        let instance = self
            .linker
            .instantiate(&mut self.store, &module)
            .map_err(|e| {
                PluginError::InitializationFailed(format!("Failed to instantiate module: {}", e))
            })?;

        self.remove_plugin_runtime_state(name);

        // Register plugin with security manager
        // Default to Untrusted level for new plugins
        self.security_manager
            .register_plugin(name.to_string(), Some(PluginTrustLevel::Untrusted))
            .map_err(|e| PluginError::SecurityViolation(e.to_string()))?;

        // Grant basic logging capabilities by default
        self.security_manager
            .grant_capability(name, PluginCapability::LogInfo)
            .map_err(|e| {
                PluginError::SecurityViolation(format!(
                    "Failed to grant logging capability: {:?}",
                    e
                ))
            })?;

        self.security_manager
            .grant_capability(name, PluginCapability::LogError)
            .map_err(|e| {
                PluginError::SecurityViolation(format!(
                    "Failed to grant logging capability: {:?}",
                    e
                ))
            })?;

        self.sync_plugin_capabilities_to_host_state(name);

        // Store module and instance
        self.modules.insert(name.to_string(), module);
        self.instances.insert(name.to_string(), instance);

        if let Err(e) = self.initialize_plugin(name) {
            self.remove_plugin_runtime_state(name);
            return Err(e);
        }

        info!(
            "Plugin '{}' loaded successfully with security context",
            name
        );
        Ok(())
    }

    fn call_plugin_function(
        &mut self,
        plugin_name: &str,
        function_name: &str,
        payload: &EventPayload,
    ) -> PluginResult<PluginResponse> {
        let started_at = Instant::now();
        self.reset_execution_metrics()?;

        let result = (|| {
            debug!(
                "Calling plugin function: {}::{}",
                plugin_name, function_name
            );

            if !self.instances.contains_key(plugin_name) {
                return Err(PluginError::PluginNotFound(plugin_name.to_string()));
            }

            if self.is_plugin_suspended(plugin_name) {
                return Err(PluginError::SecurityViolation(format!(
                    "Plugin '{}' is suspended",
                    plugin_name
                )));
            }

            self.require_plugin_capability(
                plugin_name,
                function_name,
                PluginCapability::ReadEventData,
            )?;

            // Set the current payload for the plugin to access
            self.reset_execution_budget(plugin_name)?;
            self.set_current_payload(payload)?;

            // Clear previous state and set current plugin context
            {
                let mut state = self.host_state("preparing plugin event call")?;
                state.log_messages.clear();
                state.error_message = None;
                state.current_plugin = Some(plugin_name.to_string());

                // Preserve HTTP request context when switching to event handler
                // This allows database operations during HTTP request event handling
                if state.current_http_request.is_some() {
                    // We're in an HTTP request context, so keep that context active
                    // even when handling events triggered by the HTTP handler
                    debug!(
                        "Preserving HTTP context during event handling for plugin: {}",
                        plugin_name
                    );
                } else {
                    // Only set to EventHandler if we're not in an HTTP request
                    debug!(
                        "Setting execution context to EventHandler for plugin: {}",
                        plugin_name
                    );
                    state.set_execution_context(ExecutionContext::EventHandler);
                }
            }

            debug!(
                "About to get plugin instance and call function: {}::{}",
                plugin_name, function_name
            );

            // Get the instance and call the function
            let instance = self
                .instances
                .get(plugin_name)
                .ok_or_else(|| PluginError::PluginNotFound(plugin_name.to_string()))?;

            debug!(
                "Got plugin instance, about to get typed function: {}",
                function_name
            );

            // Call the plugin function
            let func = instance
                .get_typed_func::<(), i32>(&mut self.store, function_name)
                .map_err(|e| {
                    PluginError::FunctionNotExported(format!("{}: {}", function_name, e))
                })?;

            debug!(
                "Got typed function, about to call plugin function: {}::{}",
                plugin_name, function_name
            );

            let result = func.call(&mut self.store, ()).map_err(|e| {
                PluginError::ExecutionFailed(format!("Function call failed: {}", e))
            })?;
            self.ensure_no_host_security_error()?;

            debug!(
                "Plugin function call completed with result: {} for {}::{}",
                result, plugin_name, function_name
            );

            // Check if plugin set an error
            let state = self.host_state("reading plugin event error state")?;
            if let Some(error_msg) = &state.error_message {
                debug!("Plugin set error: {}", error_msg);
            }

            drop(state);

            // Get the plugin response
            let response = self.get_plugin_response(plugin_name)?;
            self.validate_event_response_capabilities(plugin_name, function_name, &response)?;

            Ok(response)
        })();

        self.record_execution_metrics(plugin_name, started_at, result.is_err());
        result
    }

    fn has_function(&self, plugin_name: &str, function_name: &str) -> bool {
        self.modules
            .get(plugin_name)
            .map(|module| {
                module.exports().any(|export| {
                    export.name() == function_name && matches!(export.ty(), ExternType::Func(_))
                })
            })
            .unwrap_or(false)
    }

    fn unload_plugin(&mut self, plugin_name: &str) -> PluginResult<()> {
        let removed_count = {
            let state = self.host_state("counting plugin routes before unload")?;
            state
                .registered_routes
                .iter()
                .filter(|route| route.plugin_name == plugin_name)
                .count()
        };

        if self.instances.contains_key(plugin_name) {
            if let Err(error) = self.cleanup_plugin(plugin_name) {
                warn!(
                    "Plugin '{}' cleanup failed during unload; removing runtime state anyway: {}",
                    plugin_name, error
                );
            }
        }

        self.remove_plugin_runtime_state(plugin_name);

        if removed_count > 0 {
            info!(
                "Cleaned up {} registered routes for plugin '{}'",
                removed_count, plugin_name
            );
        }

        info!(
            "Plugin '{}' unloaded and security context cleared",
            plugin_name
        );
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

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::auth::{AuthService, AuthServiceConfig};
    use oxide_core::plugin_api::RouteRegistration;
    use oxide_core::InMemoryEventBus;
    use oxide_db::SqliteDb;

    fn test_runtime() -> WasmtimePluginRuntime {
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let event_bus = Arc::new(InMemoryEventBus::new());
        let db = SqliteDb::new(":memory:", event_bus, auth_service)
            .map_err(|error| PluginError::InitializationFailed(error.to_string()))
            .and_then(|db| WasmtimePluginRuntime::new(Arc::new(db)));

        match db {
            Ok(runtime) => runtime,
            Err(error) => panic!("failed to create test runtime: {}", error),
        }
    }

    #[test]
    fn unload_plugin_removes_registered_routes() {
        let mut runtime = test_runtime();
        let host_state = runtime.get_host_state();
        {
            let mut state = match host_state.lock() {
                Ok(state) => state,
                Err(error) => panic!("failed to acquire host state lock: {}", error),
            };
            state.registered_routes.push(RouteRegistration {
                method: "GET".to_string(),
                path: "/api/test".to_string(),
                handler_function: "handle_http_request".to_string(),
                plugin_name: "test-plugin".to_string(),
            });
            state.registered_routes.push(RouteRegistration {
                method: "GET".to_string(),
                path: "/api/other".to_string(),
                handler_function: "handle_http_request".to_string(),
                plugin_name: "other-plugin".to_string(),
            });
        }

        assert!(runtime.unload_plugin("test-plugin").is_ok());

        let routes = runtime.get_registered_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].plugin_name, "other-plugin");
    }

    #[test]
    fn plugin_memory_grow_is_limited_during_call() {
        let mut runtime = test_runtime();
        let wasm = br#"
            (module
              (memory (export "memory") 1)
              (func (export "plugin_init") (result i32)
                i32.const 0)
              (func (export "on_before_create") (result i32)
                i32.const 1
                memory.grow
                drop
                i32.const 0)
              (func (export "get_response_len") (result i32)
                i32.const 0)
              (func (export "get_response_ptr") (result i32)
                i32.const 0))
        "#;
        let one_wasm_page = 64 * 1024;

        runtime
            .load_plugin_with_trust(
                "memory-plugin",
                wasm,
                PluginTrustLevel::PartiallyTrusted,
                vec![PluginCapability::ReadEventData],
                ResourceLimits {
                    max_memory: one_wasm_page,
                    max_execution_time: 1_000,
                    max_host_calls: 10,
                    rate_limit: 60,
                },
            )
            .unwrap();

        let payload = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "posts".to_string(),
            data: "{}".to_string(),
            metadata: serde_json::Value::Null,
        };

        runtime
            .call_plugin_function("memory-plugin", "on_before_create", &payload)
            .unwrap();

        assert_eq!(
            runtime.get_plugin_memory_usage("memory-plugin"),
            one_wasm_page
        );
    }

    #[test]
    fn plugin_host_call_limit_is_enforced_during_call() {
        let mut runtime = test_runtime();
        let wasm = br#"
            (module
              (import "env" "get_event_payload" (func $get_event_payload (result i32)))
              (memory (export "memory") 1)
              (func (export "plugin_init") (result i32)
                i32.const 0)
              (func (export "on_before_create") (result i32)
                call $get_event_payload
                drop
                call $get_event_payload
                drop
                i32.const 0)
              (func (export "get_response_len") (result i32)
                i32.const 0)
              (func (export "get_response_ptr") (result i32)
                i32.const 0))
        "#;

        runtime
            .load_plugin_with_trust(
                "host-call-plugin",
                wasm,
                PluginTrustLevel::PartiallyTrusted,
                vec![PluginCapability::ReadEventData],
                ResourceLimits {
                    max_memory: 64 * 1024,
                    max_execution_time: 1_000,
                    max_host_calls: 1,
                    rate_limit: 60,
                },
            )
            .unwrap();

        let payload = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "posts".to_string(),
            data: "{}".to_string(),
            metadata: serde_json::Value::Null,
        };

        let error = runtime
            .call_plugin_function("host-call-plugin", "on_before_create", &payload)
            .unwrap_err();

        assert!(
            matches!(error, PluginError::SecurityViolation(message) if message.contains("host function call limit exceeded"))
        );
        let stats = runtime
            .get_plugin_stats("host-call-plugin")
            .expect("plugin stats should be recorded");
        assert_eq!(stats.host_function_calls, 2);
        assert_eq!(stats.failed_executions, 1);
    }

    #[test]
    fn plugin_initial_memory_is_limited_during_load() {
        let mut runtime = test_runtime();
        let wasm = br#"
            (module
              (memory (export "memory") 2)
              (func (export "plugin_init") (result i32)
                i32.const 0))
        "#;

        let result = runtime.load_plugin_with_trust(
            "oversized-memory-plugin",
            wasm,
            PluginTrustLevel::PartiallyTrusted,
            vec![PluginCapability::ReadEventData],
            ResourceLimits {
                max_memory: 64 * 1024,
                max_execution_time: 1_000,
                max_host_calls: 10,
                rate_limit: 60,
            },
        );

        assert!(result.is_err());
        assert!(!runtime
            .list_plugins()
            .contains(&"oversized-memory-plugin".to_string()));
    }
}
