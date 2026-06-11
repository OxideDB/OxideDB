//! Logging-related Host Functions
//!
//! These functions provide logging capabilities for plugins to communicate
//! with the host system and report errors or information.

use crate::host_state::{lock_host_state, record_host_call, HostState};
use crate::utils::read_string_from_plugin_memory;
use oxide_core::plugin_api::{host_functions, PluginError};
use oxide_core::plugin_security::PluginCapability;
use std::sync::{Arc, Mutex};
use tracing::{error, info, warn};
use wasmtime::{Caller, Linker};

/// Define logging-related host functions in the linker.
///
/// This includes:
/// - `log_info(ptr, len)`: Log an informational message
/// - `log_error(ptr, len)`: Log an error message
/// - `set_error(ptr, len)`: Set an error message for the current operation
pub fn define_logging_functions(
    linker: &mut Linker<Arc<Mutex<HostState>>>,
) -> Result<(), PluginError> {
    // log_info(ptr: *const u8, len: usize)
    linker
        .func_wrap(
            "env",
            host_functions::LOG_INFO,
            |mut caller: Caller<'_, Arc<Mutex<HostState>>>, ptr: i32, len: i32| {
                if !record_host_call(caller.data(), "recording log_info host call") {
                    return;
                }

                let plugin_name = {
                    let Some(state) =
                        lock_host_state(caller.data(), "checking log_info capability")
                    else {
                        return;
                    };
                    if !state.current_plugin_has_capability(&PluginCapability::LogInfo) {
                        warn!("log_info denied: current plugin lacks LogInfo capability");
                        return;
                    }
                    state.current_plugin.clone()
                };

                if let Some(plugin_name) = plugin_name {
                    if let Ok(message) = read_string_from_plugin_memory(&mut caller, ptr, len) {
                        info!("[PLUGIN:{}] {}", plugin_name, message);
                        if let Some(mut state) =
                            lock_host_state(caller.data(), "storing plugin info log")
                        {
                            state
                                .log_messages
                                .push(format!("[{}] {}", plugin_name, message));
                        }
                    } else {
                        warn!("Failed to read log message from plugin memory");
                    }
                } else {
                    warn!("log_info called without plugin context");
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define log_info: {}", e))
        })?;

    // log_error(ptr: *const u8, len: usize)
    linker
        .func_wrap(
            "env",
            host_functions::LOG_ERROR,
            |mut caller: Caller<'_, Arc<Mutex<HostState>>>, ptr: i32, len: i32| {
                if !record_host_call(caller.data(), "recording log_error host call") {
                    return;
                }

                let plugin_name = {
                    let Some(state) =
                        lock_host_state(caller.data(), "checking log_error capability")
                    else {
                        return;
                    };
                    if !state.current_plugin_has_capability(&PluginCapability::LogError) {
                        warn!("log_error denied: current plugin lacks LogError capability");
                        return;
                    }
                    state.current_plugin.clone()
                };

                if let Some(plugin_name) = plugin_name {
                    if let Ok(message) = read_string_from_plugin_memory(&mut caller, ptr, len) {
                        error!("[PLUGIN:{}] {}", plugin_name, message);
                        if let Some(mut state) =
                            lock_host_state(caller.data(), "storing plugin error log")
                        {
                            state
                                .log_messages
                                .push(format!("ERROR [{}]: {}", plugin_name, message));
                        }
                    } else {
                        warn!("Failed to read error message from plugin memory");
                    }
                } else {
                    warn!("log_error called without plugin context");
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define log_error: {}", e))
        })?;

    // set_error(ptr: *const u8, len: usize)
    linker
        .func_wrap(
            "env",
            host_functions::SET_ERROR,
            |mut caller: Caller<'_, Arc<Mutex<HostState>>>, ptr: i32, len: i32| {
                if !record_host_call(caller.data(), "recording set_error host call") {
                    return;
                }

                {
                    let Some(state) =
                        lock_host_state(caller.data(), "checking set_error capability")
                    else {
                        return;
                    };
                    if !state.current_plugin_has_capability(&PluginCapability::BlockOperations) {
                        warn!("set_error denied: current plugin lacks BlockOperations capability");
                        return;
                    }
                }

                if let Ok(message) = read_string_from_plugin_memory(&mut caller, ptr, len) {
                    warn!("[PLUGIN ERROR] {}", message);
                    if let Some(mut state) =
                        lock_host_state(caller.data(), "storing plugin error message")
                    {
                        state.error_message = Some(message);
                    }
                } else {
                    warn!("Failed to read error message from plugin memory");
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define set_error: {}", e))
        })?;

    Ok(())
}
