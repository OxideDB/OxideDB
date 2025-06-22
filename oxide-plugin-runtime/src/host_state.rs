//! Host state management for plugin runtime
//!
//! This module provides shared state between the host and plugins
//! for communication and data exchange.

use oxide_core::plugin_api::{HttpRequestContext, RouteRegistration};

/// Shared state between host and plugin for communication
#[derive(Debug, Clone, Default)]
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
} 