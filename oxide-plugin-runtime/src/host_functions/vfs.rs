//! Virtual File System Host Functions
//!
//! These functions allow plugins to interact with the VFS through the host,
//! maintaining security by preventing direct filesystem access.

use crate::{
    host_state::{lock_host_state, record_host_call, HostStateRef},
    utils::{allocate_plugin_memory_and_copy, read_memory_slice},
};
use oxide_core::{
    plugin_security::VfsOperation, FileIdentifier, FileListRequest, FileMoveRequest,
    FileReadRequest, FileWriteRequest,
};
use std::{future::Future, sync::OnceLock};
use tokio::runtime::RuntimeFlavor;
use tracing::warn;
use wasmtime::{Caller, Linker};

static VFS_HOST_RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

fn begin_vfs_call(state: &HostStateRef, function_name: &str, action: &str) -> bool {
    if !record_host_call(state, action) {
        return false;
    }

    let Some(mut state_guard) = lock_host_state(state, "clearing VFS host function result") else {
        return false;
    };
    state_guard.clear_function_result(function_name);
    true
}

fn ensure_vfs_capability(
    state: &HostStateRef,
    function_name: &str,
    operation: &VfsOperation,
    namespace: &str,
) -> bool {
    let Some(mut state_guard) = lock_host_state(state, "checking VFS capability") else {
        return false;
    };

    if state_guard.current_plugin_can_access_vfs(operation, namespace) {
        return true;
    }

    let plugin_name = state_guard
        .current_plugin
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let message = format!(
        "Plugin '{}' lacks {:?} VFS access to namespace '{}'",
        plugin_name, operation, namespace
    );
    warn!("{}", message);
    state_guard.store_error(function_name, &message);
    false
}

fn run_vfs_operation<F, T>(operation: F) -> Result<T, String>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
            Ok(tokio::task::block_in_place(|| handle.block_on(operation)))
        }
        _ => {
            let runtime = vfs_host_runtime()?;
            std::thread::spawn(move || runtime.block_on(operation))
                .join()
                .map_err(|_| "VFS operation thread panicked".to_string())
        }
    }
}

fn vfs_host_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    VFS_HOST_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("oxide-plugin-vfs-host")
                .build()
                .map_err(|e| format!("failed to initialize plugin VFS host runtime: {}", e))
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn store_vfs_result(
    caller: &mut Caller<'_, HostStateRef>,
    state: &HostStateRef,
    function_name: &str,
    result_json: String,
) -> wasmtime::Result<i64> {
    let Some((ptr, len)) = allocate_plugin_memory_and_copy(caller, result_json.as_bytes()) else {
        if let Some(mut state_guard) = lock_host_state(state, "storing VFS result allocation error")
        {
            state_guard.store_error(function_name, "failed to allocate plugin result memory");
        }
        return Ok(-1);
    };

    let ptr =
        u32::try_from(ptr).map_err(|_| wasmtime::Error::msg("invalid plugin result pointer"))?;
    let len =
        u32::try_from(len).map_err(|_| wasmtime::Error::msg("invalid plugin result length"))?;

    if let Some(mut state_guard) = lock_host_state(state, "storing VFS result") {
        state_guard.store_result(function_name, result_json);
        state_guard.result_buffer =
            [ptr.to_le_bytes().to_vec(), len.to_le_bytes().to_vec()].concat();
        Ok(0)
    } else {
        Ok(-1)
    }
}

/// Write a file to the VFS
pub fn vfs_write_file(
    mut caller: Caller<'_, HostStateRef>,
    namespace_ptr: i32,
    namespace_len: i32,
    request_ptr: i32,
    request_len: i32,
) -> wasmtime::Result<i64> {
    const FUNCTION_NAME: &str = "vfs_write_file";

    if !begin_vfs_call(
        caller.data(),
        FUNCTION_NAME,
        "recording vfs_write_file host call",
    ) {
        return Ok(-1);
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = read_memory_slice(data, namespace_ptr, namespace_len, "namespace")?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;
    let state = caller.data().clone();
    if !ensure_vfs_capability(&state, FUNCTION_NAME, &VfsOperation::Write, &namespace) {
        return Ok(-1);
    }

    // Read request from memory
    let request_bytes = read_memory_slice(data, request_ptr, request_len, "request")?;
    let request_json = String::from_utf8(request_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in request: {}", e)))?;

    let request: FileWriteRequest = serde_json::from_str(&request_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse request: {}", e)))?;

    // Execute VFS operation
    let operation_state = state.clone();
    let result = run_vfs_operation(async move {
        let vfs_bridge = {
            let Some(state_guard) =
                lock_host_state(&operation_state, "reading VFS bridge for write")
            else {
                return Err(oxide_core::vfs::VfsError::IoError {
                    message: "Plugin host state not available".to_string(),
                });
            };
            state_guard.vfs_bridge.clone()
        };
        if let Some(vfs_bridge) = vfs_bridge {
            vfs_bridge.vfs().write_file(&namespace, request).await
        } else {
            Err(oxide_core::vfs::VfsError::IoError {
                message: "VFS not available".to_string(),
            })
        }
    });

    match result {
        Ok(Ok(metadata)) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&metadata)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            store_vfs_result(&mut caller, &state, FUNCTION_NAME, result_json)
        }
        Ok(Err(e)) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS write error") {
                state_guard.store_error(FUNCTION_NAME, &e.to_string());
            }
            Ok(-1) // Error
        }
        Err(e) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS write error") {
                state_guard.store_error(FUNCTION_NAME, &e);
            }
            Ok(-1) // Error
        }
    }
}

/// Read a file from the VFS
pub fn vfs_read_file(
    mut caller: Caller<'_, HostStateRef>,
    namespace_ptr: i32,
    namespace_len: i32,
    request_ptr: i32,
    request_len: i32,
) -> wasmtime::Result<i64> {
    const FUNCTION_NAME: &str = "vfs_read_file";

    if !begin_vfs_call(
        caller.data(),
        FUNCTION_NAME,
        "recording vfs_read_file host call",
    ) {
        return Ok(-1);
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = read_memory_slice(data, namespace_ptr, namespace_len, "namespace")?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;
    let state = caller.data().clone();
    if !ensure_vfs_capability(&state, FUNCTION_NAME, &VfsOperation::Read, &namespace) {
        return Ok(-1);
    }

    // Read request from memory
    let request_bytes = read_memory_slice(data, request_ptr, request_len, "request")?;
    let request_json = String::from_utf8(request_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in request: {}", e)))?;

    let request: FileReadRequest = serde_json::from_str(&request_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse request: {}", e)))?;

    // Execute VFS operation
    let operation_state = state.clone();
    let result = run_vfs_operation(async move {
        let vfs_bridge = {
            let Some(state_guard) =
                lock_host_state(&operation_state, "reading VFS bridge for read")
            else {
                return Err(oxide_core::vfs::VfsError::IoError {
                    message: "Plugin host state not available".to_string(),
                });
            };
            state_guard.vfs_bridge.clone()
        };
        if let Some(vfs_bridge) = vfs_bridge {
            vfs_bridge.vfs().read_file(&namespace, request).await
        } else {
            Err(oxide_core::vfs::VfsError::IoError {
                message: "VFS not available".to_string(),
            })
        }
    });

    match result {
        Ok(Ok(response)) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&response)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            store_vfs_result(&mut caller, &state, FUNCTION_NAME, result_json)
        }
        Ok(Err(e)) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS read error") {
                state_guard.store_error(FUNCTION_NAME, &e.to_string());
            }
            Ok(-1) // Error
        }
        Err(e) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS read error") {
                state_guard.store_error(FUNCTION_NAME, &e);
            }
            Ok(-1) // Error
        }
    }
}

/// Move or rename a file in the VFS
pub fn vfs_move_file(
    mut caller: Caller<'_, HostStateRef>,
    namespace_ptr: i32,
    namespace_len: i32,
    request_ptr: i32,
    request_len: i32,
) -> wasmtime::Result<i64> {
    const FUNCTION_NAME: &str = "vfs_move_file";

    if !begin_vfs_call(
        caller.data(),
        FUNCTION_NAME,
        "recording vfs_move_file host call",
    ) {
        return Ok(-1);
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    let namespace_bytes = read_memory_slice(data, namespace_ptr, namespace_len, "namespace")?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;
    let state = caller.data().clone();
    if !ensure_vfs_capability(&state, FUNCTION_NAME, &VfsOperation::Move, &namespace) {
        return Ok(-1);
    }

    let request_bytes = read_memory_slice(data, request_ptr, request_len, "request")?;
    let request_json = String::from_utf8(request_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in request: {}", e)))?;

    let request: FileMoveRequest = serde_json::from_str(&request_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse request: {}", e)))?;

    let operation_state = state.clone();
    let result = run_vfs_operation(async move {
        let vfs_bridge = {
            let Some(state_guard) =
                lock_host_state(&operation_state, "reading VFS bridge for move")
            else {
                return Err(oxide_core::vfs::VfsError::IoError {
                    message: "Plugin host state not available".to_string(),
                });
            };
            state_guard.vfs_bridge.clone()
        };
        if let Some(vfs_bridge) = vfs_bridge {
            vfs_bridge.vfs().move_file(&namespace, request).await
        } else {
            Err(oxide_core::vfs::VfsError::IoError {
                message: "VFS not available".to_string(),
            })
        }
    });

    match result {
        Ok(Ok(metadata)) => {
            let result_json = serde_json::to_string(&metadata)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            store_vfs_result(&mut caller, &state, FUNCTION_NAME, result_json)
        }
        Ok(Err(e)) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS move error") {
                state_guard.store_error(FUNCTION_NAME, &e.to_string());
            }
            Ok(-1)
        }
        Err(e) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS move error") {
                state_guard.store_error(FUNCTION_NAME, &e);
            }
            Ok(-1)
        }
    }
}

/// Delete a file from the VFS
pub fn vfs_delete_file(
    mut caller: Caller<'_, HostStateRef>,
    namespace_ptr: i32,
    namespace_len: i32,
    identifier_ptr: i32,
    identifier_len: i32,
) -> wasmtime::Result<i64> {
    const FUNCTION_NAME: &str = "vfs_delete_file";

    if !begin_vfs_call(
        caller.data(),
        FUNCTION_NAME,
        "recording vfs_delete_file host call",
    ) {
        return Ok(-1);
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = read_memory_slice(data, namespace_ptr, namespace_len, "namespace")?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;
    let state = caller.data().clone();
    if !ensure_vfs_capability(&state, FUNCTION_NAME, &VfsOperation::Delete, &namespace) {
        return Ok(-1);
    }

    // Read identifier from memory
    let identifier_bytes = read_memory_slice(data, identifier_ptr, identifier_len, "identifier")?;
    let identifier_json = String::from_utf8(identifier_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in identifier: {}", e)))?;

    let identifier: FileIdentifier = serde_json::from_str(&identifier_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse identifier: {}", e)))?;

    // Execute VFS operation
    let operation_state = state.clone();
    let result = run_vfs_operation(async move {
        let vfs_bridge = {
            let Some(state_guard) =
                lock_host_state(&operation_state, "reading VFS bridge for delete")
            else {
                return Err(oxide_core::vfs::VfsError::IoError {
                    message: "Plugin host state not available".to_string(),
                });
            };
            state_guard.vfs_bridge.clone()
        };
        if let Some(vfs_bridge) = vfs_bridge {
            vfs_bridge.vfs().delete_file(&namespace, identifier).await
        } else {
            Err(oxide_core::vfs::VfsError::IoError {
                message: "VFS not available".to_string(),
            })
        }
    });

    match result {
        Ok(Ok(_)) => store_vfs_result(&mut caller, &state, FUNCTION_NAME, "true".to_string()),
        Ok(Err(e)) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS delete error") {
                state_guard.store_error(FUNCTION_NAME, &e.to_string());
            }
            Ok(-1) // Error
        }
        Err(e) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS delete error") {
                state_guard.store_error(FUNCTION_NAME, &e);
            }
            Ok(-1) // Error
        }
    }
}

/// List files in the VFS
pub fn vfs_list_files(
    mut caller: Caller<'_, HostStateRef>,
    namespace_ptr: i32,
    namespace_len: i32,
    request_ptr: i32,
    request_len: i32,
) -> wasmtime::Result<i64> {
    const FUNCTION_NAME: &str = "vfs_list_files";

    if !begin_vfs_call(
        caller.data(),
        FUNCTION_NAME,
        "recording vfs_list_files host call",
    ) {
        return Ok(-1);
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = read_memory_slice(data, namespace_ptr, namespace_len, "namespace")?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;
    let state = caller.data().clone();
    if !ensure_vfs_capability(&state, FUNCTION_NAME, &VfsOperation::List, &namespace) {
        return Ok(-1);
    }

    // Read request from memory
    let request_bytes = read_memory_slice(data, request_ptr, request_len, "request")?;
    let request_json = String::from_utf8(request_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in request: {}", e)))?;

    let request: FileListRequest = serde_json::from_str(&request_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse request: {}", e)))?;

    // Execute VFS operation
    let operation_state = state.clone();
    let result = run_vfs_operation(async move {
        let vfs_bridge = {
            let Some(state_guard) =
                lock_host_state(&operation_state, "reading VFS bridge for list")
            else {
                return Err(oxide_core::vfs::VfsError::IoError {
                    message: "Plugin host state not available".to_string(),
                });
            };
            state_guard.vfs_bridge.clone()
        };
        if let Some(vfs_bridge) = vfs_bridge {
            vfs_bridge.vfs().list_files(&namespace, request).await
        } else {
            Err(oxide_core::vfs::VfsError::IoError {
                message: "VFS not available".to_string(),
            })
        }
    });

    match result {
        Ok(Ok(response)) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&response)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            store_vfs_result(&mut caller, &state, FUNCTION_NAME, result_json)
        }
        Ok(Err(e)) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS list error") {
                state_guard.store_error(FUNCTION_NAME, &e.to_string());
            }
            Ok(-1) // Error
        }
        Err(e) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS list error") {
                state_guard.store_error(FUNCTION_NAME, &e);
            }
            Ok(-1) // Error
        }
    }
}

/// Get VFS usage statistics
pub fn vfs_get_usage_stats(
    mut caller: Caller<'_, HostStateRef>,
    namespace_ptr: i32,
    namespace_len: i32,
) -> wasmtime::Result<i64> {
    const FUNCTION_NAME: &str = "vfs_get_usage_stats";

    if !begin_vfs_call(
        caller.data(),
        FUNCTION_NAME,
        "recording vfs_get_usage_stats host call",
    ) {
        return Ok(-1);
    }

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = read_memory_slice(data, namespace_ptr, namespace_len, "namespace")?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;
    let state = caller.data().clone();
    if !ensure_vfs_capability(&state, FUNCTION_NAME, &VfsOperation::Usage, &namespace) {
        return Ok(-1);
    }

    // Execute VFS operation
    let operation_state = state.clone();
    let result = run_vfs_operation(async move {
        let vfs_bridge = {
            let Some(state_guard) =
                lock_host_state(&operation_state, "reading VFS bridge for usage stats")
            else {
                return Err(oxide_core::vfs::VfsError::IoError {
                    message: "Plugin host state not available".to_string(),
                });
            };
            state_guard.vfs_bridge.clone()
        };
        if let Some(vfs_bridge) = vfs_bridge {
            vfs_bridge.vfs().get_usage_stats(&namespace).await
        } else {
            Err(oxide_core::vfs::VfsError::IoError {
                message: "VFS not available".to_string(),
            })
        }
    });

    match result {
        Ok(Ok(stats)) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&stats)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            store_vfs_result(&mut caller, &state, FUNCTION_NAME, result_json)
        }
        Ok(Err(e)) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS usage stats error")
            {
                state_guard.store_error(FUNCTION_NAME, &e.to_string());
            }
            Ok(-1) // Error
        }
        Err(e) => {
            if let Some(mut state_guard) = lock_host_state(&state, "storing VFS usage stats error")
            {
                state_guard.store_error(FUNCTION_NAME, &e);
            }
            Ok(-1) // Error
        }
    }
}

/// Register all VFS host functions with the linker
pub fn register_vfs_functions(linker: &mut Linker<HostStateRef>) -> wasmtime::Result<()> {
    linker.func_wrap("env", "vfs_write_file", vfs_write_file)?;
    linker.func_wrap("env", "vfs_read_file", vfs_read_file)?;
    linker.func_wrap("env", "vfs_move_file", vfs_move_file)?;
    linker.func_wrap("env", "vfs_delete_file", vfs_delete_file)?;
    linker.func_wrap("env", "vfs_list_files", vfs_list_files)?;
    linker.func_wrap("env", "vfs_get_usage_stats", vfs_get_usage_stats)?;
    Ok(())
}
