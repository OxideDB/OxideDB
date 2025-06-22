//! OxideDB Plugin SDK
//! 
//! This SDK provides a high-level API for developing OxideDB plugins with WASM.
//! It abstracts away the low-level FFI boilerplate and provides a clean,
//! Rust-idiomatic interface for plugin development.
//! 
//! # Features
//! 
//! - **Event Handling**: Simple trait-based event handling
//! - **HTTP Routes**: Easy HTTP route registration and handling
//! - **Database Operations**: High-level database CRUD operations
//! - **Logging**: Structured logging support
//! - **Memory Management**: Automatic memory management for WASM
//! 
//! # Quick Start
//! 
//! ```rust
//! use oxide_plugin_sdk::prelude::*;
//! 
//! struct MyPlugin;
//! 
//! impl PluginEventHandler for MyPlugin {
//!     fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
//!         // Your plugin logic here
//!         Ok(PluginResponse::allow())
//!     }
//! }
//! 
//! // Export the plugin
//! oxide_plugin_sdk::export_plugin!(MyPlugin);
//! ```

pub mod types;
pub mod host;
pub mod memory;
pub mod macros;
pub mod http;
pub mod database;
pub mod logging;
pub mod prelude;

// Re-export commonly used types
pub use types::*;
pub use host::Host;
pub use memory::MemoryManager;

/// Result type for plugin operations
pub type PluginResult<T> = Result<T, PluginError>;

/// Plugin error types
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Host function call failed: {0}")]
    HostCallFailed(String),
    
    #[error("Invalid data format: {0}")]
    InvalidData(String),
    
    #[error("Memory allocation failed")]
    MemoryFailed,
    
    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),
    
    #[error("Plugin execution error: {0}")]
    ExecutionError(String),
}

/// Trait for handling plugin events
/// 
/// Implement this trait to define your plugin's behavior for different events.
/// Only implement the methods for events you want to handle.
pub trait PluginEventHandler {
    /// Called before a record is created
    fn on_before_create(&mut self, _event: &EventPayload) -> PluginResult<PluginResponse> {
        Ok(PluginResponse::allow())
    }
    
    /// Called after a record is created
    fn on_after_create(&mut self, _event: &EventPayload) -> PluginResult<PluginResponse> {
        Ok(PluginResponse::allow())
    }
    
    /// Called before a record is updated
    fn on_before_update(&mut self, _event: &EventPayload) -> PluginResult<PluginResponse> {
        Ok(PluginResponse::allow())
    }
    
    /// Called after a record is updated
    fn on_after_update(&mut self, _event: &EventPayload) -> PluginResult<PluginResponse> {
        Ok(PluginResponse::allow())
    }
    
    /// Called before a record is deleted
    fn on_before_delete(&mut self, _event: &EventPayload) -> PluginResult<PluginResponse> {
        Ok(PluginResponse::allow())
    }
    
    /// Called after a record is deleted
    fn on_after_delete(&mut self, _event: &EventPayload) -> PluginResult<PluginResponse> {
        Ok(PluginResponse::allow())
    }
    
    /// Called when the plugin is initialized
    fn on_init(&mut self) -> PluginResult<()> {
        Ok(())
    }
    
    /// Called when the plugin is being unloaded
    fn on_cleanup(&mut self) -> PluginResult<()> {
        Ok(())
    }
}

/// Trait for handling HTTP requests
/// 
/// Implement this trait if your plugin needs to handle HTTP requests.
#[cfg(feature = "http")]
pub trait PluginHttpHandler {
    /// Handle an HTTP request
    fn handle_request(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse>;
}

/// Global plugin instance storage
use std::sync::{Mutex, OnceLock};

static PLUGIN_INSTANCE: OnceLock<Mutex<Option<Box<dyn PluginEventHandler + Send>>>> = OnceLock::new();

#[cfg(feature = "http")]
static HTTP_HANDLER: OnceLock<Mutex<Option<Box<dyn PluginHttpHandler + Send>>>> = OnceLock::new();

/// Initialize the plugin instance
pub fn init_plugin<T: PluginEventHandler + Send + 'static>(plugin: T) {
    let _ = PLUGIN_INSTANCE.set(Mutex::new(Some(Box::new(plugin))));
}

/// Initialize the HTTP handler
#[cfg(feature = "http")]
pub fn init_http_handler<T: PluginHttpHandler + Send + 'static>(handler: T) {
    let boxed_handler = Box::new(handler);
    
    // Check if HTTP_HANDLER is already initialized
    if let Some(existing) = HTTP_HANDLER.get() {
        // Handler already exists, update it
        if let Ok(mut guard) = existing.lock() {
            *guard = Some(boxed_handler);
            crate::host::Host::log_info("HTTP handler re-initialized");
        } else {
            crate::host::Host::log_error("Failed to lock existing HTTP handler for re-initialization");
        }
    } else {
        // Handler doesn't exist, initialize it
        match HTTP_HANDLER.set(Mutex::new(Some(boxed_handler))) {
            Ok(()) => {
                crate::host::Host::log_info("HTTP handler successfully initialized");
            }
            Err(_) => {
                // This should not happen since we checked above, but handle it gracefully
                crate::host::Host::log_error("Failed to initialize HTTP handler - already set by another thread");
            }
        }
    }
}

/// Get the plugin instance for event handling
pub fn with_plugin<F, R>(f: F) -> PluginResult<R>
where
    F: FnOnce(&mut dyn PluginEventHandler) -> PluginResult<R>,
{
    let instance = PLUGIN_INSTANCE.get()
        .ok_or_else(|| PluginError::ExecutionError("Plugin not initialized".to_string()))?;
    
    let mut guard = instance.lock()
        .map_err(|_| PluginError::ExecutionError("Failed to lock plugin instance".to_string()))?;
    
    let plugin = guard.as_mut()
        .ok_or_else(|| PluginError::ExecutionError("Plugin instance not found".to_string()))?;
    
    f(plugin.as_mut())
}

/// Get the HTTP handler for request handling
#[cfg(feature = "http")]
pub fn with_http_handler<F, R>(f: F) -> PluginResult<R>
where
    F: FnOnce(&mut dyn PluginHttpHandler) -> PluginResult<R>,
{
    let instance = HTTP_HANDLER.get()
        .ok_or_else(|| {
            crate::host::Host::log_error("HTTP_HANDLER.get() returned None - handler never initialized");
            PluginError::ExecutionError("HTTP handler not initialized".to_string())
        })?;
    
    let mut guard = instance.lock()
        .map_err(|_| {
            crate::host::Host::log_error("Failed to acquire lock on HTTP handler mutex");
            PluginError::ExecutionError("Failed to lock HTTP handler".to_string())
        })?;
    
    let handler = guard.as_mut()
        .ok_or_else(|| {
            crate::host::Host::log_error("HTTP handler mutex contains None - handler was initialized but then cleared");
            PluginError::ExecutionError("HTTP handler not found".to_string())
        })?;
    
    f(handler.as_mut())
}

/// Helper function to handle events and return responses
/// This is used by the export macros to reduce boilerplate
pub fn handle_event_with_response<F>(handler: F) -> i32
where
    F: FnOnce(&mut dyn PluginEventHandler, &EventPayload) -> PluginResult<PluginResponse>,
{
    match with_plugin(|plugin| {
        // Get the event payload from the host
        let payload = crate::host::Host::get_event_payload()?;
        
        // Call the handler function
        let response = handler(plugin, &payload)?;
        
        // Serialize and set the response
        let response_json = serde_json::to_string(&response)?;
        crate::memory::MemoryManager::set_response(response_json.as_bytes());
        
        Ok(())
    }) {
        Ok(()) => 0, // Success
        Err(e) => {
            crate::host::Host::log_error(&format!("Event handler failed: {}", e));
            1 // Error
        }
    }
}

/// Helper function to handle HTTP requests
/// This is used by the HTTP export macros
#[cfg(feature = "http")]
pub fn handle_http_request_impl() -> i32 {
    crate::host::Host::log_info("handle_http_request_impl called");
    
    // First check if HTTP handler is initialized
    if HTTP_HANDLER.get().is_none() {
        crate::host::Host::log_error("HTTP handler not initialized - HTTP_HANDLER is None");
        let error_response = crate::types::HttpResponse::error(500, "HTTP handler not initialized");
        let _ = crate::host::Host::set_http_response(&error_response);
        return 1;
    }
    
    crate::host::Host::log_info("HTTP_HANDLER.get() is Some, checking contents...");
    
    // Additional debug: check if the mutex contains an actual handler
    if let Some(handler_mutex) = HTTP_HANDLER.get() {
        if let Ok(guard) = handler_mutex.lock() {
            if guard.is_none() {
                crate::host::Host::log_error("HTTP_HANDLER mutex contains None - handler was not properly initialized");
                let error_response = crate::types::HttpResponse::error(500, "HTTP handler not initialized");
                let _ = crate::host::Host::set_http_response(&error_response);
                return 1;
            } else {
                crate::host::Host::log_info("HTTP handler mutex contains a valid handler, proceeding...");
            }
        } else {
            crate::host::Host::log_error("Failed to lock HTTP handler mutex");
            let error_response = crate::types::HttpResponse::error(500, "HTTP handler lock failed");
            let _ = crate::host::Host::set_http_response(&error_response);
            return 1;
        }
    }
    
    match with_http_handler(|handler| {
        crate::host::Host::log_info("HTTP handler found, processing request");
        
        // Get the HTTP request from the host
        let request = crate::host::Host::get_http_request()?;
        crate::host::Host::log_info(&format!("HTTP request: {} {}", request.method, request.path));
        
        // Call the handler
        let response = handler.handle_request(&request)?;
        crate::host::Host::log_info(&format!("HTTP response status: {}", response.status_code));
        
        // Set the response
        crate::host::Host::set_http_response(&response)?;
        
        Ok(())
    }) {
        Ok(()) => {
            crate::host::Host::log_info("HTTP request handled successfully");
            0 // Success
        }
        Err(e) => {
            crate::host::Host::log_error(&format!("HTTP handler failed: {}", e));
            // Try to send an error response
            let error_response = crate::types::HttpResponse::error(500, &e.to_string());
            let _ = crate::host::Host::set_http_response(&error_response);
            1 // Error
        }
    }
}

// Logging macros for convenient use
/// Log an info message
#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::host::Host::log_info(&format!($($arg)*))
    };
}

/// Log an error message
#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        $crate::host::Host::log_error(&format!($($arg)*))
    };
}

/// Log a warning message
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        $crate::host::Host::log_warn(&format!($($arg)*))
    };
}

/// Log a debug message
#[macro_export]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        $crate::host::Host::log_debug(&format!($($arg)*))
    };
} 