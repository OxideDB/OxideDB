//! Hello Plugin - A simple WASM plugin for OxideDB
//!
//! This plugin demonstrates the basic plugin API by:
//! 1. Importing host functions (log_info, log_error, set_error, get_event_payload)
//! 2. Exporting the on_before_create function
//! 3. Processing events and calling host functions
//! 4. Working with the security-enhanced plugin system

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
// These correspond to the security-enhanced plugin API
extern "C" {
    /// Get the current event payload as JSON string
    /// Requires ReadEventData capability
    fn get_event_payload() -> i32;

    /// Log an info message to the host
    /// Requires LogInfo capability
    fn log_info(ptr: *const u8, len: usize);

    /// Log an error message to the host
    /// Requires LogError capability
    fn log_error(ptr: *const u8, len: usize);

    /// Set an error message (prevents operation from continuing)
    /// Requires BlockOperations capability
    fn set_error(ptr: *const u8, len: usize);

    /// Get the result of get_event_payload call
    fn get_result_ptr() -> i32;
    fn get_result_len() -> i32;
}

use std::sync::Mutex;
use std::sync::OnceLock;

// Global buffer for sharing data with host
static RESPONSE_BUFFER: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

/// Helper function to call host's log_info
/// This function requires the LogInfo capability to be granted to the plugin
fn host_log_info(message: &str) {
    let bytes = message.as_bytes();
    unsafe {
        log_info(bytes.as_ptr(), bytes.len());
    }
}

/// Helper function to call host's log_error
/// This function requires the LogError capability to be granted to the plugin
fn host_log_error(message: &str) {
    let bytes = message.as_bytes();
    unsafe {
        log_error(bytes.as_ptr(), bytes.len());
    }
}

/// Helper function to call host's set_error
/// This function requires the BlockOperations capability to be granted to the plugin
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
/// 
/// # Safety
/// 
/// This function is unsafe because it reconstructs a Vec from raw parts.
/// The caller must ensure that:
/// - `ptr` was originally allocated by the `alloc` function in this module
/// - `size` matches the original capacity used when allocating the memory
/// - The pointer has not been deallocated previously
/// - No other references to this memory exist
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    let _ = Vec::from_raw_parts(ptr, 0, size);
}

/// Set response data that the host can retrieve
/// 
/// # Safety
/// 
/// This function is unsafe because it dereferences a raw pointer (`ptr`) to create a slice.
/// The caller must ensure that:
/// - `ptr` is valid and points to at least `len` bytes of readable memory
/// - The memory pointed to by `ptr` remains valid for the duration of this function call
/// - `len` accurately represents the number of bytes available at `ptr`
#[no_mangle]
pub unsafe extern "C" fn set_response(ptr: *const u8, len: usize) {
    let buffer = RESPONSE_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(mut buf) = buffer.lock() {
        buf.clear();
        buf.extend_from_slice(std::slice::from_raw_parts(ptr, len));
    }
}

/// Get response data length
#[no_mangle]
pub extern "C" fn get_response_len() -> usize {
    let buffer = RESPONSE_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(buf) = buffer.lock() {
        buf.len()
    } else {
        0
    }
}

/// Get response data pointer
#[no_mangle]
pub extern "C" fn get_response_ptr() -> *const u8 {
    let buffer = RESPONSE_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
    if let Ok(buf) = buffer.lock() {
        buf.as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Helper function to serialize and set plugin response
fn set_plugin_response(response: &PluginResponse) -> Result<(), String> {
    let response_json = serde_json::to_string(response)
        .map_err(|e| format!("Failed to serialize response: {}", e))?;
    
    let bytes = response_json.as_bytes();
    unsafe {
        set_response(bytes.as_ptr(), bytes.len());
    }
    Ok(())
}

/// Main plugin function - called before create operations
/// This function demonstrates:
/// 1. Getting event payload from host
/// 2. Processing the data
/// 3. Logging information (both info and error)
/// 4. Setting response data
/// 5. Working with the security-enhanced plugin system
#[no_mangle]
pub extern "C" fn on_before_create() -> i32 {
    host_log_info("Hello Plugin: on_before_create called");

    // Get the event payload from the host
    let payload = match host_get_event_payload() {
        Ok(payload) => payload,
        Err(e) => {
            let error_msg = format!("Failed to get event payload: {}", e);
            host_log_error(&error_msg);
            host_set_error(&error_msg);
            return 1; // Error code
        }
    };

    host_log_info(&format!(
        "Processing event: {} for collection: {}",
        payload.event_type, payload.collection
    ));

    // Example: Validate collection name for security
    if payload.collection.contains("admin") || payload.collection.contains("system") {
        let error_msg = format!("Access denied to restricted collection: {}", payload.collection);
        host_log_error(&error_msg);
        
        let response = PluginResponse {
             allow: false,
             modified_data: None,
             error_message: Some(error_msg.clone()),
             metadata: serde_json::json!({
                 "processed_by": "hello-plugin",
                 "version": "1.0.0",
                 "security_check": "failed"
             }),
         };
         
         match set_plugin_response(&response) {
             Ok(_) => return 0, // Success code (operation blocked as intended)
             Err(e) => {
                 host_set_error(&format!("Failed to set response: {}", e));
                 return 1; // Error code
             }
         }
    }

    // Example: Modify the data by adding a timestamp and plugin info
    let modified_data = if let Ok(mut data_obj) = serde_json::from_str::<serde_json::Value>(&payload.data) {
        // Add plugin metadata
        if let Some(obj) = data_obj.as_object_mut() {
            obj.insert(
                "plugin_processed_at".to_string(),
                serde_json::Value::String("2024-01-01T00:00:00Z".to_string()),
            );
            obj.insert(
                "plugin_name".to_string(),
                serde_json::Value::String("hello-plugin".to_string()),
            );
        }
        Some(data_obj.to_string())
    } else {
        host_log_info("Could not parse data as JSON, leaving unchanged");
        None
    };

    // Create response
    let response = PluginResponse {
        allow: true,
        modified_data,
        error_message: None,
        metadata: serde_json::json!({
            "processed_by": "hello-plugin",
            "version": "1.0.0",
            "security_check": "passed"
        }),
    };

    // Set the response for the host to retrieve
      match set_plugin_response(&response) {
          Ok(_) => {
              host_log_info("Hello Plugin: Successfully processed create event");
              0 // Success code
          }
          Err(e) => {
              let error_msg = format!("Failed to set response: {}", e);
              host_log_error(&error_msg);
              host_set_error(&error_msg);
              1 // Error code
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
