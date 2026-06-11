//! Runtime Utilities
//!
//! Shared utility functions used across the plugin runtime implementation.

use crate::host_state::HostState;
use std::sync::{Arc, Mutex};
use wasmtime::Caller;

/// Helper function to allocate plugin memory and copy data.
///
/// This function allocates memory in the plugin's WebAssembly linear memory
/// and copies the provided data into that memory space. It returns the
/// pointer and length of the allocated memory.
///
/// # Arguments
/// * `caller` - The Wasmtime caller context
/// * `data` - The data to copy into plugin memory
///
/// # Returns
/// * `Some((ptr, len))` - Success: pointer to allocated memory and length
/// * `None` - Failure: unable to allocate or copy data
pub fn allocate_plugin_memory_and_copy(
    caller: &mut Caller<'_, Arc<Mutex<HostState>>>,
    data: &[u8],
) -> Option<(i32, i32)> {
    let len = i32::try_from(data.len()).ok()?;

    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
        // Call plugin's alloc function to get memory
        if let Some(alloc_export) = caller.get_export("alloc") {
            if let Some(alloc_func_raw) = alloc_export.into_func() {
                if let Ok(alloc_func) = alloc_func_raw.typed::<i32, i32>(&mut *caller) {
                    if let Ok(ptr) = alloc_func.call(&mut *caller, len) {
                        // Copy data to plugin memory
                        let memory_data = memory.data_mut(&mut *caller);
                        let start = usize::try_from(ptr).ok()?;
                        let end = start.checked_add(data.len())?;

                        if end <= memory_data.len() {
                            memory_data[start..end].copy_from_slice(data);
                            return Some((ptr, len));
                        }
                    }
                }
            }
        }
    }
    None
}

/// Helper function to read string data from plugin memory.
///
/// # Arguments
/// * `caller` - The Wasmtime caller context
/// * `ptr` - Pointer to the start of the string in plugin memory
/// * `len` - Length of the string in bytes
///
/// # Returns
/// * `Ok(String)` - Successfully read string
/// * `Err(&str)` - Error message describing the failure
pub fn read_string_from_plugin_memory(
    caller: &mut Caller<'_, Arc<Mutex<HostState>>>,
    ptr: i32,
    len: i32,
) -> Result<String, &'static str> {
    if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
        let data = memory.data(caller);
        let start = usize::try_from(ptr).map_err(|_| "Negative memory pointer")?;
        let len = usize::try_from(len).map_err(|_| "Negative memory length")?;
        let end = start
            .checked_add(len)
            .ok_or("Memory access range overflow")?;

        if end <= data.len() {
            std::str::from_utf8(&data[start..end])
                .map(|s| s.to_string())
                .map_err(|_| "Invalid UTF-8 string")
        } else {
            Err("Memory access out of bounds")
        }
    } else {
        Err("Plugin memory not found")
    }
}

/// Read a checked byte slice from plugin memory data.
pub fn read_memory_slice<'a>(
    data: &'a [u8],
    ptr: i32,
    len: i32,
    label: &str,
) -> wasmtime::Result<&'a [u8]> {
    let start = usize::try_from(ptr)
        .map_err(|_| wasmtime::Error::msg(format!("negative pointer for {}", label)))?;
    let len = usize::try_from(len)
        .map_err(|_| wasmtime::Error::msg(format!("negative length for {}", label)))?;
    let end = start
        .checked_add(len)
        .ok_or_else(|| wasmtime::Error::msg(format!("memory range overflow for {}", label)))?;

    data.get(start..end)
        .ok_or_else(|| wasmtime::Error::msg(format!("failed to read {} from memory", label)))
}

/// Create a standardized error response for database operations.
///
/// # Arguments
/// * `error` - The error message to include in the response
///
/// # Returns
/// * Serialized JSON error response as bytes
pub fn create_error_response(error: &str) -> Vec<u8> {
    let error_response = serde_json::json!({
        "success": false,
        "error": error
    });
    error_response.to_string().into_bytes()
}

/// Create a standardized success response for database operations.
///
/// # Arguments
/// * `data` - The data to include in the response
///
/// # Returns
/// * Serialized JSON success response as bytes
pub fn create_success_response(data: serde_json::Value) -> Vec<u8> {
    let response = serde_json::json!({
        "success": true,
        "data": data
    });
    response.to_string().into_bytes()
}
