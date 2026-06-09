//! HTTP-related Host Functions
//!
//! These functions provide HTTP capabilities for plugins to register routes,
//! handle HTTP requests, and send HTTP responses.

use crate::host_state::HostState;
use crate::utils::read_string_from_plugin_memory;
use oxide_core::plugin_api::{host_functions, PluginError, RouteRegistration, HttpResponse};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::info;
use wasmtime::{Caller, Linker};

/// Define HTTP-related host functions in the linker.
///
/// This includes:
/// - `register_http_route(method_ptr, method_len, path_ptr, path_len, handler_ptr, handler_len)`: Register HTTP route
/// - `get_http_request()`: Get current HTTP request data
/// - `set_http_response(status_code, headers_ptr, headers_len, body_ptr, body_len)`: Set HTTP response
pub fn define_http_functions(
    linker: &mut Linker<Arc<Mutex<HostState>>>,
) -> Result<(), PluginError> {
    // register_http_route(method_ptr: *const u8, method_len: usize, path_ptr: *const u8, path_len: usize, handler_ptr: *const u8, handler_len: usize)
    linker
        .func_wrap(
            "env",
            host_functions::REGISTER_HTTP_ROUTE,
            |mut caller: Caller<'_, Arc<Mutex<HostState>>>,
             method_ptr: i32, method_len: i32,
             path_ptr: i32, path_len: i32,
             handler_ptr: i32, handler_len: i32| -> i32 {
                caller.data().lock().unwrap().record_host_call();

                let method = match read_string_from_plugin_memory(&mut caller, method_ptr, method_len) {
                    Ok(s) => s,
                    Err(_) => return -1,
                };

                let path = match read_string_from_plugin_memory(&mut caller, path_ptr, path_len) {
                    Ok(s) => s,
                    Err(_) => return -1,
                };

                let handler_function = match read_string_from_plugin_memory(&mut caller, handler_ptr, handler_len) {
                    Ok(s) => s,
                    Err(_) => return -1,
                };

                let plugin_name = {
                    let state = caller.data().lock().unwrap();
                    state.current_plugin.clone()
                };

                if let Some(plugin_name) = plugin_name {
                    let route = RouteRegistration {
                        method,
                        path,
                        handler_function,
                        plugin_name,
                    };

                    let route_path = format!("{} {}", route.method, route.path);

                    let mut state = caller.data().lock().unwrap();
                    state.registered_routes.push(route);
                    info!("Registered HTTP route: {}", route_path);
                    0 // Success
                } else {
                    -1 // Error: no plugin context
                }
            },
        )
        .map_err(|e| {
            PluginError::InitializationFailed(format!("Failed to define register_http_route: {}", e))
        })?;

    // get_http_request() -> i32 (returns length, copies data to plugin memory)
    linker
        .func_wrap(
            "env",
            host_functions::GET_HTTP_REQUEST,
            |mut caller: Caller<'_, Arc<Mutex<HostState>>>| -> i32 {
                caller.data().lock().unwrap().record_host_call();

                let state = caller.data().lock().unwrap();
                if let Some(request) = &state.current_http_request {
                    if let Ok(request_json) = serde_json::to_string(request) {
                        let request_bytes = request_json.as_bytes().to_vec();
                        drop(state);

                        // Get plugin memory and allocate space
                        if let Some(memory) = caller.get_export("memory").and_then(|e| e.into_memory()) {
                            if let Some(alloc_export) = caller.get_export("alloc") {
                                if let Some(alloc_func_raw) = alloc_export.into_func() {
                                    if let Ok(alloc_func) = alloc_func_raw.typed::<i32, i32>(&mut caller) {
                                        if let Ok(ptr) = alloc_func.call(&mut caller, request_bytes.len() as i32) {
                                            let data = memory.data_mut(&mut caller);
                                            let start = ptr as usize;
                                            let end = start + request_bytes.len();

                                            if end <= data.len() {
                                                data[start..end].copy_from_slice(&request_bytes);

                                                let mut state = caller.data().lock().unwrap();
                                                state.result_buffer = [
                                                    (ptr as u32).to_le_bytes().to_vec(),
                                                    (request_bytes.len() as u32).to_le_bytes().to_vec(),
                                                ].concat();

                                                return request_bytes.len() as i32;
                                            }
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
            PluginError::InitializationFailed(format!("Failed to define get_http_request: {}", e))
        })?;

    // set_http_response(status_ptr: *const u8, headers_ptr: *const u8, headers_len: usize, body_ptr: *const u8, body_len: usize)
    linker
        .func_wrap(
            "env",
            host_functions::SET_HTTP_RESPONSE,
            |mut caller: Caller<'_, Arc<Mutex<HostState>>>,
             status_code: i32,
             headers_ptr: i32, headers_len: i32,
             body_ptr: i32, body_len: i32| -> i32 {
                caller.data().lock().unwrap().record_host_call();

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

                let response = HttpResponse {
                    status_code: status_code as u16,
                    headers,
                    body,
                };

                if let Ok(response_json) = serde_json::to_string(&response) {
                    let mut state = caller.data().lock().unwrap();
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
