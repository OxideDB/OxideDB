//! Host state management for plugin runtime
//!
//! This module provides shared state between the host and plugins
//! for communication and data exchange.

use oxide_core::{
    auth::CrudOperation,
    plugin_api::{HttpRequestContext, RouteRegistration},
    plugin_security::PluginCapability,
    VfsServiceBridge,
};
use oxide_logging::LogServiceBridge;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Type alias for shared host state reference
pub type HostStateRef = Arc<Mutex<HostState>>;

/// Execution context for tracking plugin call stack
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionContext {
    /// Plugin is handling an HTTP request
    HttpRequest,
    /// Plugin is handling an event (before/after)
    EventHandler,
    /// Plugin is idle/not executing
    Idle,
}

/// Shared state between host and plugin for communication
#[derive(Clone)]
pub struct HostState {
    /// Current event payload being processed
    pub current_payload: Option<String>,

    /// Messages logged by plugins
    pub log_messages: Vec<String>,

    /// Error message set by plugin (if any)
    pub error_message: Option<String>,

    /// Result buffer for host function returns
    pub result_buffer: Vec<u8>,

    /// Current plugin name being executed
    pub current_plugin: Option<String>,

    /// Registered HTTP routes from plugins
    pub registered_routes: Vec<RouteRegistration>,

    /// Current HTTP request context (when handling HTTP requests)
    pub current_http_request: Option<HttpRequestContext>,

    /// HTTP response buffer for plugin responses
    pub http_response_buffer: Vec<u8>,

    /// Database operation results buffer
    pub db_result_buffer: Vec<u8>,

    /// Current execution context to prevent reentrancy issues
    pub execution_context: ExecutionContext,

    /// Whether the plugin is currently in a database operation
    pub in_database_operation: bool,

    /// VFS service bridge for file operations
    pub vfs_bridge: Option<Arc<dyn VfsServiceBridge>>,

    /// Logging service bridge for audit and general logging
    pub logging_bridge: Option<Arc<LogServiceBridge>>,

    /// Result storage for host function calls
    pub function_results: std::collections::HashMap<String, String>,

    /// Error storage for host function calls
    pub function_errors: std::collections::HashMap<String, String>,

    /// Host functions called during the current plugin execution
    pub current_execution_host_calls: u64,

    /// Granted capabilities for loaded plugins, mirrored from the runtime
    /// security manager for host function authorization.
    pub plugin_capabilities: HashMap<String, Vec<PluginCapability>>,
    // NOTE: Removed plugin_metadata field - using TOML-only metadata approach
    // Plugin metadata is now sourced exclusively from plugin.toml during installation
}

impl Default for HostState {
    fn default() -> Self {
        Self {
            current_payload: None,
            log_messages: Vec::new(),
            error_message: None,
            result_buffer: Vec::new(),
            current_plugin: None,
            registered_routes: Vec::new(),
            current_http_request: None,
            http_response_buffer: Vec::new(),
            db_result_buffer: Vec::new(),
            execution_context: ExecutionContext::Idle,
            in_database_operation: false,
            vfs_bridge: None,
            logging_bridge: None,
            function_results: std::collections::HashMap::new(),
            function_errors: std::collections::HashMap::new(),
            current_execution_host_calls: 0,
            plugin_capabilities: HashMap::new(),
        }
    }
}

impl HostState {
    /// Check if we're in a context that should allow database operations
    pub fn can_perform_database_operations(&self) -> bool {
        // If there's an active HTTP request, always allow database operations
        // This covers the case where an HTTP handler triggers events but still needs DB access
        if self.current_http_request.is_some() {
            return true;
        }

        // For non-HTTP contexts, only prevent database operations during event handling
        // to avoid circular dependencies (e.g., event handler triggering more events)
        match self.execution_context {
            ExecutionContext::HttpRequest => true, // Should not happen if no current_http_request, but allow anyway
            ExecutionContext::EventHandler => false, // Prevent reentrancy during pure event handling
            ExecutionContext::Idle => true,
        }
    }

    /// Set the execution context
    pub fn set_execution_context(&mut self, context: ExecutionContext) {
        self.execution_context = context;
    }

    /// Mark that we're entering a database operation
    pub fn enter_database_operation(&mut self) {
        self.in_database_operation = true;
    }

    /// Mark that we're exiting a database operation
    pub fn exit_database_operation(&mut self) {
        self.in_database_operation = false;
    }

    /// Store a result from a host function call
    pub fn store_result(&mut self, function_name: &str, result: String) {
        self.function_results
            .insert(function_name.to_string(), result);
    }

    /// Store an error from a host function call
    pub fn store_error(&mut self, function_name: &str, error: &str) {
        self.function_errors
            .insert(function_name.to_string(), error.to_string());
    }

    /// Get a result from a host function call
    pub fn get_result(&self, function_name: &str) -> Option<&String> {
        self.function_results.get(function_name)
    }

    /// Get an error from a host function call
    pub fn get_error(&self, function_name: &str) -> Option<&String> {
        self.function_errors.get(function_name)
    }

    /// Clear function results and errors
    pub fn clear_function_results(&mut self) {
        self.function_results.clear();
        self.function_errors.clear();
    }

    /// Increment host function calls for the active plugin execution.
    pub fn record_host_call(&mut self) {
        self.current_execution_host_calls = self.current_execution_host_calls.saturating_add(1);
    }

    /// Mirror a plugin's current capabilities into host state.
    pub fn set_plugin_capabilities(
        &mut self,
        plugin_name: impl Into<String>,
        capabilities: Vec<PluginCapability>,
    ) {
        self.plugin_capabilities
            .insert(plugin_name.into(), capabilities);
    }

    /// Remove mirrored capabilities for an unloaded plugin.
    pub fn remove_plugin_capabilities(&mut self, plugin_name: &str) {
        self.plugin_capabilities.remove(plugin_name);
    }

    /// Check whether the current plugin has a capability grant.
    pub fn current_plugin_has_capability(&self, requested: &PluginCapability) -> bool {
        self.current_plugin
            .as_ref()
            .and_then(|plugin_name| self.plugin_capabilities.get(plugin_name))
            .map(|capabilities| {
                capabilities
                    .iter()
                    .any(|capability| capability.grants(requested))
            })
            .unwrap_or(false)
    }

    /// Check whether the current plugin can register an HTTP route.
    pub fn current_plugin_can_register_http_route(&self, method: &str, path: &str) -> bool {
        self.current_plugin
            .as_ref()
            .and_then(|plugin_name| self.plugin_capabilities.get(plugin_name))
            .map(|capabilities| {
                capabilities
                    .iter()
                    .any(|capability| capability.allows_http_route(method, path))
            })
            .unwrap_or(false)
    }

    /// Check whether the current plugin can perform a database operation.
    pub fn current_plugin_can_access_collection(
        &self,
        operation: &CrudOperation,
        collection: &str,
    ) -> bool {
        self.current_plugin
            .as_ref()
            .and_then(|plugin_name| self.plugin_capabilities.get(plugin_name))
            .map(|capabilities| {
                capabilities
                    .iter()
                    .any(|capability| capability.allows_record_operation(operation, collection))
            })
            .unwrap_or(false)
    }

    // NOTE: Plugin metadata methods removed - using TOML-only metadata approach
    // Metadata is now sourced exclusively from plugin.toml during installation
    // and stored in the PluginConfiguration in the database
}
