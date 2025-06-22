//! Prelude module for convenient imports
//! 
//! This module re-exports the most commonly used types and traits
//! from the OxideDB Plugin SDK, allowing for simple imports:
//! 
//! ```rust
//! use oxide_plugin_sdk::prelude::*;
//! ```

// Core types and traits
pub use crate::{
    PluginResult, PluginError, PluginEventHandler,
    EventPayload, PluginResponse,
    init_plugin
};

// Type shortcuts
pub use crate::types::{
    HttpRequestContext, HttpResponse, Record, DatabaseResult, LogLevel,
};

// High-level APIs
pub use crate::host::Host;
pub use crate::memory::MemoryManager;
pub use crate::logging::{Logger, LogBuilder};
pub use crate::database::{Database, QueryBuilder, RecordBuilder};

// HTTP support (feature-gated)
#[cfg(feature = "http")]
pub use crate::{PluginHttpHandler, init_http_handler, with_http_handler};

#[cfg(feature = "http")]
pub use crate::http::{Http, HttpHandler, FunctionHandler, JsonResponseBuilder};

// Macros
pub use crate::{
    export_plugin,
    register_routes,
    log_info, log_error, log_warn, log_debug,
    json_response, error_response, text_response,
};

#[cfg(feature = "http")]
pub use crate::export_http_plugin;

// Re-export commonly used external types
pub use serde::{Serialize, Deserialize};
pub use serde_json::{json, Value as JsonValue}; 