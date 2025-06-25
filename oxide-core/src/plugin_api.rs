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

/// HTTP request context passed to plugin HTTP handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestContext {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: String,
    /// Request path
    pub path: String,
    /// Query parameters
    pub query_params: std::collections::HashMap<String, String>,
    /// Request headers
    pub headers: std::collections::HashMap<String, String>,
    /// Request body
    pub body: Option<String>,
    /// Path parameters from route matching
    pub path_params: std::collections::HashMap<String, String>,
    /// User context (if authenticated)
    pub user: Option<serde_json::Value>,
}

/// HTTP response from plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    /// HTTP status code
    pub status_code: u16,
    /// Response headers
    pub headers: std::collections::HashMap<String, String>,
    /// Response body
    pub body: String,
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self {
            status_code: 200,
            headers: std::collections::HashMap::new(),
            body: String::new(),
        }
    }
}

/// HTTP route registration request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRegistration {
    /// HTTP method
    pub method: String,
    /// Route path (can include parameters like /users/:id)
    pub path: String,
    /// Plugin function name to call for this route
    pub handler_function: String,
    /// Plugin name that owns this route
    pub plugin_name: String,
}

/// CRUD operation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrudRequest {
    /// Collection name
    pub collection: String,
    /// Operation type
    pub operation: CrudOperationType,
    /// Data for create/update operations
    pub data: Option<serde_json::Value>,
    /// Filter for read/update/delete operations
    pub filter: Option<serde_json::Value>,
    /// Options for the operation
    pub options: Option<serde_json::Value>,
}

/// CRUD operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrudResult {
    /// Whether the operation was successful
    pub success: bool,
    /// Result data (records for read, ID for create, count for update/delete)
    pub data: Option<serde_json::Value>,
    /// Error message if operation failed
    pub error: Option<String>,
}

/// Types of CRUD operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CrudOperationType {
    Create,
    Read,
    Update,
    Delete,
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

    // Virtual File System host functions
    /// Function signature for writing a file to VFS
    /// Parameters: namespace (string), request (JSON)
    /// Returns: FileMetadata as JSON string
    pub const VFS_WRITE_FILE: &str = "vfs_write_file";

    /// Function signature for reading a file from VFS
    /// Parameters: namespace (string), request (JSON)
    /// Returns: FileReadResponse as JSON string
    pub const VFS_READ_FILE: &str = "vfs_read_file";

    /// Function signature for deleting a file from VFS
    /// Parameters: namespace (string), identifier (JSON)
    /// Returns: success boolean
    pub const VFS_DELETE_FILE: &str = "vfs_delete_file";

    /// Function signature for listing files in VFS
    /// Parameters: namespace (string), request (JSON)
    /// Returns: FileListResponse as JSON string
    pub const VFS_LIST_FILES: &str = "vfs_list_files";

    /// Function signature for getting VFS usage stats
    /// Parameters: namespace (string)
    /// Returns: VfsUsageStats as JSON string
    pub const VFS_GET_USAGE_STATS: &str = "vfs_get_usage_stats";

    /// Function signature for registering HTTP routes
    /// Parameters: method, path, handler_name (strings)
    pub const REGISTER_HTTP_ROUTE: &str = "register_http_route";

    /// Function signature for creating records
    /// Parameters: collection, data (strings)
    /// Returns: created record ID
    pub const CREATE_RECORD: &str = "create_record";

    /// Function signature for reading records
    /// Parameters: collection, filter (strings)
    /// Returns: JSON array of records
    pub const READ_RECORDS: &str = "read_records";

    /// Function signature for updating a single record
    /// Parameters: collection, record_id, data (strings)
    /// Returns: updated record
    pub const UPDATE_RECORD: &str = "update_record";

    /// Function signature for deleting a single record
    /// Parameters: collection, record_id (strings)
    /// Returns: deleted record
    pub const DELETE_RECORD: &str = "delete_record";

    /// Function signature for updating records
    /// Parameters: collection, filter, data (strings)
    /// Returns: number of affected records
    pub const UPDATE_RECORDS: &str = "update_records";

    /// Function signature for deleting records
    /// Parameters: collection, filter (strings)
    /// Returns: number of deleted records
    pub const DELETE_RECORDS: &str = "delete_records";

    /// Function signature for getting current HTTP request context
    /// Returns: JSON-serialized HttpRequestContext
    pub const GET_HTTP_REQUEST: &str = "get_http_request";

    /// Function signature for setting HTTP response
    /// Parameters: status_code, headers, body (as JSON)
    pub const SET_HTTP_RESPONSE: &str = "set_http_response";
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

    /// Called to register HTTP routes during plugin initialization
    pub const REGISTER_ROUTES: &str = "register_routes";

    /// Called when an HTTP request is received for a plugin route
    /// Plugin should define custom handler functions for each route
    pub const HANDLE_HTTP_REQUEST: &str = "handle_http_request";
}

/// Trait that defines the plugin runtime interface
/// 
/// This abstraction allows for different plugin runtime implementations
/// (e.g., Wasmtime, WASI, or other WebAssembly runtimes) to be used
/// interchangeably within the OxideDB ecosystem.
pub trait PluginRuntime: Send + Sync {
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

    /// Get runtime name for identification
    fn runtime_name(&self) -> &'static str;

    /// Get runtime version for compatibility checking
    fn runtime_version(&self) -> &'static str;
}

/// Factory trait for creating plugin runtime instances
/// 
/// This allows for runtime-agnostic plugin runtime creation,
/// enabling different implementations to be selected at runtime
/// based on configuration or other criteria.
pub trait PluginRuntimeFactory: Send + Sync {
    /// The concrete runtime type this factory creates
    type Runtime: PluginRuntime;

    /// Create a new plugin runtime instance
    fn create_runtime(&self) -> PluginResult<Self::Runtime>;

    /// Get the name of the runtime this factory creates
    fn runtime_type(&self) -> &'static str;

    /// Check if this factory supports the given runtime configuration
    fn supports_config(&self, config: &PluginRuntimeConfig) -> bool;
}

/// Configuration for plugin runtime creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRuntimeConfig {
    /// The preferred runtime type (e.g., "wasmtime", "wasmtime-wasi")
    pub runtime_type: String,

    /// Maximum memory limit for plugins (in bytes)
    pub memory_limit: Option<u64>,

    /// Maximum execution time for plugin functions (in milliseconds)
    pub timeout_ms: Option<u64>,

    /// Security policies to apply
    pub security_policies: Option<serde_json::Value>,

    /// Additional runtime-specific configuration
    pub runtime_specific: serde_json::Value,
}
