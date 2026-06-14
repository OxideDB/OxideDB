//! HTTP-related Host Functions
//!
//! These functions provide HTTP capabilities for plugins to register routes,
//! handle HTTP requests, and send HTTP responses.

use crate::host_state::{lock_host_state, record_host_call, PluginStoreData};
use crate::utils::{allocate_plugin_memory_and_copy, read_string_from_plugin_memory};
use oxide_core::plugin_api::{host_functions, HttpResponse, PluginError, RouteRegistration};
use oxide_core::plugin_security::PluginCapability;
use std::collections::HashMap;
use tracing::{info, warn};
use wasmtime::{Caller, Linker};

/// Define HTTP-related host functions in the linker.
///
/// This includes:
/// - `register_http_route(method_ptr, method_len, path_ptr, path_len, handler_ptr, handler_len)`: Register HTTP route
/// - `get_http_request()`: Get current HTTP request data
/// - `set_http_response(status_code, headers_ptr, headers_len, body_ptr, body_len)`: Set HTTP response
pub fn define_http_functions(linker: &mut Linker<PluginStoreData>) -> Result<(), PluginError> {
    // register_http_route(method_ptr: *const u8, method_len: usize, path_ptr: *const u8, path_len: usize, handler_ptr: *const u8, handler_len: usize)
    linker
        .func_wrap(
            "env",
            host_functions::REGISTER_HTTP_ROUTE,
            |mut caller: Caller<'_, PluginStoreData>,
             method_ptr: i32,
             method_len: i32,
             path_ptr: i32,
             path_len: i32,
             handler_ptr: i32,
             handler_len: i32|
             -> i32 {
                if !record_host_call(caller.data(), "recording register_http_route host call") {
                    return -1;
                }

                let method =
                    match read_string_from_plugin_memory(&mut caller, method_ptr, method_len) {
                        Ok(s) => s,
                        Err(_) => return -1,
                    };

                let path = match read_string_from_plugin_memory(&mut caller, path_ptr, path_len) {
                    Ok(s) => s,
                    Err(_) => return -1,
                };

                let handler_function =
                    match read_string_from_plugin_memory(&mut caller, handler_ptr, handler_len) {
                        Ok(s) => s,
                        Err(_) => return -1,
                    };

                let plugin_name = {
                    let Some(state) = lock_host_state(
                        caller.data(),
                        "reading current plugin for route registration",
                    ) else {
                        return -1;
                    };
                    state.current_plugin.clone()
                };

                if let Some(plugin_name) = plugin_name {
                    {
                        let Some(state) = lock_host_state(
                            caller.data(),
                            "checking route registration capability",
                        ) else {
                            return -1;
                        };
                        if !state.current_plugin_can_register_http_route(&method, &path) {
                            warn!(
                                "Denied HTTP route registration for plugin '{}': {} {}",
                                plugin_name, method, path
                            );
                            return -1;
                        }
                    }

                    let route = RouteRegistration {
                        method,
                        path,
                        handler_function,
                        plugin_name,
                    };

                    let route_path = format!("{} {}", route.method, route.path);

                    let Some(mut state) =
                        lock_host_state(caller.data(), "storing registered HTTP route")
                    else {
                        return -1;
                    };
                    state.registered_routes.push(route);
                    info!("Registered HTTP route: {}", route_path);
                    0 // Success
                } else {
                    -1 // Error: no plugin context
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!(
                "Failed to define register_http_route: {}",
                e
            ))
        })?;

    // get_http_request() -> i32 (returns length, copies data to plugin memory)
    linker
        .func_wrap(
            "env",
            host_functions::GET_HTTP_REQUEST,
            |mut caller: Caller<'_, PluginStoreData>| -> i32 {
                if !record_host_call(caller.data(), "recording get_http_request host call") {
                    return -1;
                }

                let request = {
                    let Some(state) =
                        lock_host_state(caller.data(), "reading current HTTP request")
                    else {
                        return -1;
                    };
                    if !state.current_plugin_has_capability(&PluginCapability::HandleHttpRequests) {
                        return -1;
                    }
                    state.current_http_request.clone()
                };

                if let Some(request) = request {
                    if let Ok(request_json) = serde_json::to_string(&request) {
                        let request_bytes = request_json.as_bytes().to_vec();

                        if let Some((ptr, len)) =
                            allocate_plugin_memory_and_copy(&mut caller, &request_bytes)
                        {
                            let Ok(ptr) = u32::try_from(ptr) else {
                                return -1;
                            };
                            let Ok(len_u32) = u32::try_from(len) else {
                                return -1;
                            };
                            let Some(mut state) = lock_host_state(
                                caller.data(),
                                "storing HTTP request result pointer",
                            ) else {
                                return -1;
                            };
                            state.result_buffer =
                                [ptr.to_le_bytes().to_vec(), len_u32.to_le_bytes().to_vec()]
                                    .concat();

                            return len;
                        }
                    }
                }
                -1
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define get_http_request: {}", e))
        })?;

    // set_http_response(status_ptr: *const u8, headers_ptr: *const u8, headers_len: usize, body_ptr: *const u8, body_len: usize)
    linker
        .func_wrap(
            "env",
            host_functions::SET_HTTP_RESPONSE,
            |mut caller: Caller<'_, PluginStoreData>,
             status_code: i32,
             headers_ptr: i32,
             headers_len: i32,
             body_ptr: i32,
             body_len: i32|
             -> i32 {
                if !record_host_call(caller.data(), "recording set_http_response host call") {
                    return -1;
                }

                {
                    let Some(state) =
                        lock_host_state(caller.data(), "checking set_http_response capability")
                    else {
                        return -1;
                    };
                    if !state.current_plugin_has_capability(&PluginCapability::HandleHttpRequests)
                    {
                        warn!("set_http_response denied: current plugin lacks HandleHttpRequests capability");
                        return -1;
                    }
                }

                let headers_json = if headers_len > 0 {
                    match read_string_from_plugin_memory(&mut caller, headers_ptr, headers_len) {
                        Ok(s) => s,
                        Err(_) => return -1,
                    }
                } else {
                    "{}".to_string()
                };

                let body = if body_len > 0 {
                    match read_string_from_plugin_memory(&mut caller, body_ptr, body_len) {
                        Ok(s) => s,
                        Err(_) => return -1,
                    }
                } else {
                    String::new()
                };

                let headers: HashMap<String, String> =
                    serde_json::from_str(&headers_json).unwrap_or_default();
                let Ok(status_code) = u16::try_from(status_code) else {
                    warn!("Plugin attempted to set invalid negative HTTP status");
                    return -1;
                };
                if !(100..=599).contains(&status_code) {
                    warn!("Plugin attempted to set invalid HTTP status: {}", status_code);
                    return -1;
                }

                let response = HttpResponse {
                    status_code,
                    headers,
                    body,
                };

                if let Ok(response_json) = serde_json::to_string(&response) {
                    let Some(mut state) =
                        lock_host_state(caller.data(), "storing plugin HTTP response")
                    else {
                        return -1;
                    };
                    state.http_response_buffer = response_json.into_bytes();
                    info!("Set HTTP response: status {}", status_code);
                    0 // Success
                } else {
                    -1 // Error serializing response
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define set_http_response: {}", e))
        })?;

    Ok(())
}
