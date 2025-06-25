//! Prelude module for convenient imports in plugin development
//! 
//! This module re-exports commonly used types and traits to make
//! plugin development more ergonomic. Import this module with:
//! 
//! ```rust
//! use oxide_plugin_sdk::prelude::*;
//! ```

// Re-export main SDK types
pub use crate::{
    PluginResult, PluginError, PluginEventHandler, PluginResponse, EventPayload,
    Record, DatabaseResult, LogLevel,
    // Metadata types
    PluginMetadata, PluginRuntimeInfo, RuntimeDetails, MemoryInfo, PluginStatus, PluginStats,
};

// Re-export host interface
pub use crate::host::Host;

// Re-export memory management
pub use crate::memory::MemoryManager;

// Re-export HTTP types if HTTP feature is enabled
#[cfg(feature = "http")]
pub use crate::{PluginHttpHandler, HttpRequestContext, HttpResponse};

// Re-export HTTP module functionality
#[cfg(feature = "http")]
pub use crate::http::{Http, JsonResponseBuilder};

// Re-export database functionality
pub use crate::database::Database;

// Re-export logging macros
pub use crate::{log_info, log_error, log_warn, log_debug};

// Re-export useful external types
pub use serde_json::{json, Value as JsonValue};

// Export plugin initialization macros
pub use crate::export_plugin;

// Export HTTP macros if HTTP feature is enabled
#[cfg(feature = "http")]
pub use crate::export_http_plugin; 