//! Hello Plugin - A simple WASM plugin for OxideDB
//!
//! This plugin demonstrates the basic plugin API by:
//! 1. Importing host functions (log_info, set_error, get_event_payload)
//! 2. Exporting the on_before_create function
//! 3. Processing events and calling host functions

use serde::{Deserialize, Serialize};

/// Plugin response structure matching the host contract
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginResponse {
    pub allow: bool,
    pub modified_data: Option<String>,
    pub error_message: Option<String>,
    pub metadata: serde_json::Value,
}

/// Event payload structure matching the host contract
#[derive(Debug, Deserialize)]
pub struct EventPayload {
    pub event_type: String,
    pub collection: String,
    pub data: String,
    pub metadata: serde_json::Value,
}

// Import host functions that the plugin can call
extern "C" {
    /// Get the current event payload as JSON string
    fn get_event_payload() -> i32;

    /// Log an info message to the host
    fn log_info(ptr: *const u8, len: usize);

    /// Set an error message (prevents operation from continuing)
    fn set_error(ptr: *const u8, len: usize);

    /// Get the result of get_event_payload call
    fn get_result_ptr() -> i32;
    fn get_result_len() -> i32;
}

// Global buffer for sharing data with host
static mut RESULT_BUFFER: Vec<u8> = Vec::new();
static mut RESPONSE_BUFFER: Vec<u8> = Vec::new();

/// Helper function to call host's log_info
fn host_log_info(message: &str) {
    let bytes = message.as_bytes();
    unsafe {
        log_info(bytes.as_ptr(), bytes.len());
    }
}

/// Helper function to call host's set_error
fn host_set_error(message: &str) {
    let bytes = message.as_bytes();
    unsafe {
        set_error(bytes.as_ptr(), bytes.len());
    }
}

/// Helper function to get event payload from host
fn host_get_event_payload() -> Result<EventPayload, String> {
    unsafe {
        let result = get_event_payload();
        if result < 0 {
            return Err("Failed to get event payload".to_string());
        }

        let ptr = get_result_ptr();
        let len = get_result_len();

        if ptr == 0 || len == 0 {
            return Err("Invalid payload pointer or length".to_string());
        }

        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        let json_str = std::str::from_utf8(slice).map_err(|e| format!("Invalid UTF-8: {}", e))?;

        serde_json::from_str(json_str).map_err(|e| format!("Failed to parse payload: {}", e))
    }
}

/// Export function for memory allocation (required for string passing)
#[no_mangle]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// Export function for memory deallocation
#[no_mangle]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, 0, size);
    }
}

/// Set response data that the host can retrieve
#[no_mangle]
pub extern "C" fn set_response(ptr: *const u8, len: usize) {
    unsafe {
        RESPONSE_BUFFER.clear();
        RESPONSE_BUFFER.extend_from_slice(std::slice::from_raw_parts(ptr, len));
    }
}

/// Get response data length
#[no_mangle]
pub extern "C" fn get_response_len() -> usize {
    unsafe { RESPONSE_BUFFER.len() }
}

/// Get response data pointer
#[no_mangle]
pub extern "C" fn get_response_ptr() -> *const u8 {
    unsafe { RESPONSE_BUFFER.as_ptr() }
}

/// Main plugin function: called before a record is created
#[no_mangle]
pub extern "C" fn on_before_create() -> i32 {
    host_log_info("Hello from WASM plugin! on_before_create called");

    // Get the event payload from the host
    let payload = match host_get_event_payload() {
        Ok(payload) => payload,
        Err(e) => {
            let error_msg = format!("Failed to get event payload: {}", e);
            host_set_error(&error_msg);
            return -1;
        }
    };

    host_log_info(&format!(
        "Processing event: {} for collection: {} with data: {}",
        payload.event_type, payload.collection, payload.data
    ));

    // For the PoC, let's set an error message as requested in the requirements
    host_set_error("Hello Wasm plugin test error message");

    // Create a response
    let response = PluginResponse {
        allow: false, // Don't allow the operation due to test error
        modified_data: None,
        error_message: Some("Plugin test error".to_string()),
        metadata: serde_json::json!({ "plugin": "hello-plugin", "version": "0.1.0" }),
    };

    // Serialize response
    match serde_json::to_string(&response) {
        Ok(response_json) => {
            let bytes = response_json.as_bytes();
            unsafe {
                RESPONSE_BUFFER.clear();
                RESPONSE_BUFFER.extend_from_slice(bytes);
            }
            0 // Success
        }
        Err(_) => {
            host_set_error("Failed to serialize plugin response");
            -1
        }
    }
}

/// Plugin initialization function
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    host_log_info("Hello plugin initialized!");
    0
}

/// Plugin cleanup function
#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 {
    host_log_info("Hello plugin cleaned up!");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_response_serialization() {
        let response = PluginResponse {
            allow: true,
            modified_data: None,
            error_message: None,
            metadata: serde_json::json!({}),
        };

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("\"allow\":true"));
    }
}
