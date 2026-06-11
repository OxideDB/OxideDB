//! Event-related Host Functions
//!
//! These functions handle event payload management and result buffer access
//! for plugin communication during event processing.

use crate::host_state::{lock_host_state, record_host_call, HostState};
use crate::utils::allocate_plugin_memory_and_copy;
use oxide_core::plugin_api::{host_functions, PluginError};
use oxide_core::plugin_security::PluginCapability;
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
                if !record_host_call(caller.data(), "recording get_event_payload host call") {
                    return -1;
                }

                let payload = {
                    let Some(state) =
                        lock_host_state(caller.data(), "reading current event payload")
                    else {
                        return -1;
                    };
                    if !state.current_plugin_has_capability(&PluginCapability::ReadEventData) {
                        return -1;
                    }
                    state.current_payload.clone()
                };

                if let Some(payload) = payload {
                    let payload_bytes = payload.as_bytes().to_vec();

                    if let Some((ptr, len)) =
                        allocate_plugin_memory_and_copy(&mut caller, &payload_bytes)
                    {
                        let Ok(ptr) = u32::try_from(ptr) else {
                            return -1;
                        };
                        let Ok(len_u32) = u32::try_from(len) else {
                            return -1;
                        };
                        let Some(mut state) =
                            lock_host_state(caller.data(), "storing event payload result pointer")
                        else {
                            return -1;
                        };
                        state.result_buffer =
                            [ptr.to_le_bytes().to_vec(), len_u32.to_le_bytes().to_vec()].concat();

                        return len;
                    }
                }
                -1
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define get_event_payload: {}", e))
        })?;

    // get_result_ptr() -> i32 (returns pointer to allocated plugin memory)
    linker
        .func_wrap(
            "env",
            "get_result_ptr",
            |caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                if !record_host_call(caller.data(), "recording get_result_ptr host call") {
                    return 0;
                }

                let Some(state) = lock_host_state(caller.data(), "reading result pointer") else {
                    return 0;
                };
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
                if !record_host_call(caller.data(), "recording get_result_len host call") {
                    return 0;
                }

                let Some(state) = lock_host_state(caller.data(), "reading result length") else {
                    return 0;
                };
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
