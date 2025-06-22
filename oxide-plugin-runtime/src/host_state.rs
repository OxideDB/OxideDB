//! Host state management for plugin runtime
//!
//! This module provides shared state between the host and plugins
//! for communication and data exchange.

use oxide_core::plugin_api::{HttpRequestContext, RouteRegistration};

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
#[derive(Debug, Clone)]
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
} 