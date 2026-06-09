//! Event-related Host Functions
//!
//! These functions handle event payload management and result buffer access
//! for plugin communication during event processing.

use crate::host_state::HostState;
use oxide_core::plugin_api::{host_functions, PluginError};
use std::sync::{Arc, Mutex};
use wasmtime::{Caller, Linker};

/// Define event-related host functions in the linker.
///
/// This includes:
/// - `get_event_payload()`: Returns event payload data to the plugin
/// - `get_result_ptr()`: Returns pointer to result data in plugin memory
/// - `get_result_len()`: Returns length of result data
pub fn define_event_functions(
    linker: &mut Linker<Arc<Mutex<HostState>>>,
) -> Result<(), PluginError> {
    // get_event_payload() -> i32 (returns length, copies data to plugin memory)
    linker
        .func_wrap(
            "env",
            host_functions::GET_EVENT_PAYLOAD,
            |mut caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                caller.data().lock().unwrap().record_host_call();

                let state = caller.data().lock().unwrap();
                if let Some(payload) = &state.current_payload {
                    let payload_bytes = payload.as_bytes().to_vec();
                    drop(state); // Release the lock early

                    // Get plugin memory and allocate space
                    if let Some(memory) =
                        caller.get_export("memory").and_then(|e| e.into_memory())
                    {
                        // Call plugin's alloc function to get memory
                        if let Some(alloc_export) = caller.get_export("alloc") {
                            if let Some(alloc_func_raw) = alloc_export.into_func() {
                                if let Ok(alloc_func) =
                                    alloc_func_raw.typed::<i32, i32>(&mut caller)
                                {
                                    if let Ok(ptr) =
                                        alloc_func.call(&mut caller, payload_bytes.len() as i32)
                                    {
                                        // Copy data to plugin memory
                                        let data = memory.data_mut(&mut caller);
                                        let start = ptr as usize;
                                        let end = start + payload_bytes.len();

                                        if end <= data.len() {
                                            data[start..end].copy_from_slice(&payload_bytes);

                                            // Store the pointer and length in result buffer for get_result_ptr/len
                                            let mut state = caller.data().lock().unwrap();
                                            state.result_buffer = [
                                                (ptr as u32).to_le_bytes().to_vec(),
                                                (payload_bytes.len() as u32)
                                                    .to_le_bytes()
                                                    .to_vec(),
                                            ]
                                            .concat();

                                            return payload_bytes.len() as i32;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                -1
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!(
                "Failed to define get_event_payload: {}",
                e
            ))
        })?;

    // get_result_ptr() -> i32 (returns pointer to allocated plugin memory)
    linker
        .func_wrap(
            "env",
            "get_result_ptr",
            |caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                caller.data().lock().unwrap().record_host_call();

                let state = caller.data().lock().unwrap();
                if state.result_buffer.len() >= 4 {
                    // Read pointer from first 4 bytes (works for all operations)
                    u32::from_le_bytes([
                        state.result_buffer[0],
                        state.result_buffer[1],
                        state.result_buffer[2],
                        state.result_buffer[3],
                    ]) as i32
                } else {
                    0
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define get_result_ptr: {}", e))
        })?;

    // get_result_len() -> i32 (returns length of data)
    linker
        .func_wrap(
            "env",
            "get_result_len",
            |caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                caller.data().lock().unwrap().record_host_call();

                let state = caller.data().lock().unwrap();
                if state.result_buffer.len() >= 8 {
                    // Read length from bytes 4-7 (works for all operations)
                    u32::from_le_bytes([
                        state.result_buffer[4],
                        state.result_buffer[5],
                        state.result_buffer[6],
                        state.result_buffer[7],
                    ]) as i32
                } else {
                    0
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define get_result_len: {}", e))
        })?;

    Ok(())
}
