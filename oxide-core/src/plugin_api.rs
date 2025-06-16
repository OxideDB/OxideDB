//! Plugin API Contract for OxideDB
//!
//! This module defines the interface between the host (OxideDB) and Wasm plugins.
//! It establishes a clear contract for:
//! - Functions the host provides to plugins (host functions)
//! - Functions that plugins must export for the host to call (plugin exports)

use crate::AppError;
use serde::{Deserialize, Serialize};

/// Standard result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Errors that can occur during plugin execution
#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum PluginError {
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid plugin response: {0}")]
    InvalidResponse(String),

    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    #[error("Plugin function not exported: {0}")]
    FunctionNotExported(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),
}

impl From<PluginError> for AppError {
    fn from(err: PluginError) -> Self {
        AppError::Plugin {
            plugin_name: "unknown".to_string(),
            message: err.to_string(),
        }
    }
}

/// Payload structure for event data passed between host and plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    /// The event type (e.g., "BeforeRecordCreate", "AfterRecordUpdate")
    pub event_type: String,

    /// The collection/table name being operated on
    pub collection: String,

    /// The data being operated on (JSON-serialized)
    pub data: String,

    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Response from a plugin after processing an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    /// Whether the plugin wants to allow the operation to continue
    pub allow: bool,

    /// Optional modified data (if the plugin wants to transform the data)
    pub modified_data: Option<String>,

    /// Optional error message (if allow is false)
    pub error_message: Option<String>,

    /// Additional metadata to pass back to the host
    pub metadata: serde_json::Value,
}

impl Default for PluginResponse {
    fn default() -> Self {
        Self {
            allow: true,
            modified_data: None,
            error_message: None,
            metadata: serde_json::Value::Null,
        }
    }
}

/// Host functions that plugins can call
/// These are implemented by the host and made available to the plugin runtime
pub mod host_functions {


    /// Function signature for getting the current event payload
    /// Returns JSON-serialized EventPayload
    pub const GET_EVENT_PAYLOAD: &str = "get_event_payload";

    /// Function signature for logging info messages from plugins
    /// Parameters: message (string)
    pub const LOG_INFO: &str = "log_info";

    /// Function signature for logging error messages from plugins  
    /// Parameters: message (string)
    pub const LOG_ERROR: &str = "log_error";

    /// Function signature for setting an error (prevents operation from continuing)
    /// Parameters: message (string)
    pub const SET_ERROR: &str = "set_error";

    /// Function signature for getting configuration value
    /// Parameters: key (string)
    /// Returns: JSON value as string
    pub const GET_CONFIG: &str = "get_config";
}

/// Plugin export functions that must be implemented by plugins
/// These are called by the host when events occur
pub mod plugin_exports {
    /// Called before a record is created
    pub const ON_BEFORE_CREATE: &str = "on_before_create";

    /// Called after a record is created
    pub const ON_AFTER_CREATE: &str = "on_after_create";

    /// Called before a record is updated
    pub const ON_BEFORE_UPDATE: &str = "on_before_update";

    /// Called after a record is updated
    pub const ON_AFTER_UPDATE: &str = "on_after_update";

    /// Called before a record is deleted
    pub const ON_BEFORE_DELETE: &str = "on_before_delete";

    /// Called after a record is deleted
    pub const ON_AFTER_DELETE: &str = "on_after_delete";

    /// Called when the plugin is initialized
    pub const PLUGIN_INIT: &str = "plugin_init";

    /// Called when the plugin is being unloaded
    pub const PLUGIN_CLEANUP: &str = "plugin_cleanup";
}

/// Trait that defines the plugin runtime interface
pub trait PluginRuntime {
    /// Load a plugin from bytes
    fn load_plugin(&mut self, name: &str, wasm_bytes: &[u8]) -> PluginResult<()>;

    /// Call a plugin function with the given payload
    fn call_plugin_function(
        &mut self,
        plugin_name: &str,
        function_name: &str,
        payload: &EventPayload,
    ) -> PluginResult<PluginResponse>;

    /// Check if a plugin exports a specific function
    fn has_function(&self, plugin_name: &str, function_name: &str) -> bool;

    /// Unload a plugin
    fn unload_plugin(&mut self, plugin_name: &str) -> PluginResult<()>;

    /// List all loaded plugins
    fn list_plugins(&self) -> Vec<String>;
}
