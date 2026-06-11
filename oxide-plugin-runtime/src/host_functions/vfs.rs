//! Virtual File System Host Functions
//!
//! These functions allow plugins to interact with the VFS through the host,
//! maintaining security by preventing direct filesystem access.

use crate::host_state::HostStateRef;
use oxide_core::{FileIdentifier, FileListRequest, FileReadRequest, FileWriteRequest};
use wasmtime::{Caller, Linker};

/// Write a file to the VFS
pub fn vfs_write_file(
    mut caller: Caller<'_, HostStateRef>,
    namespace_ptr: i32,
    namespace_len: i32,
    request_ptr: i32,
    request_len: i32,
) -> wasmtime::Result<i64> {
    caller.data().lock().unwrap().record_host_call();

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = data
        .get(namespace_ptr as usize..(namespace_ptr + namespace_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read namespace from memory"))?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;

    // Read request from memory
    let request_bytes = data
        .get(request_ptr as usize..(request_ptr + request_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read request from memory"))?;
    let request_json = String::from_utf8(request_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in request: {}", e)))?;

    let request: FileWriteRequest = serde_json::from_str(&request_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse request: {}", e)))?;

    let state = caller.data().clone();
    let rt = tokio::runtime::Handle::current();

    // Execute VFS operation
    let result = rt.block_on(async {
        let vfs_bridge = {
            let state_guard = state.lock().unwrap();
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
        Ok(metadata) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&metadata)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            let mut state_guard = state.lock().unwrap();
            state_guard.store_result("vfs_write_file", result_json);
            Ok(0) // Success
        }
        Err(e) => {
            let mut state_guard = state.lock().unwrap();
            state_guard.store_error("vfs_write_file", &e.to_string());
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
    caller.data().lock().unwrap().record_host_call();

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = data
        .get(namespace_ptr as usize..(namespace_ptr + namespace_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read namespace from memory"))?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;

    // Read request from memory
    let request_bytes = data
        .get(request_ptr as usize..(request_ptr + request_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read request from memory"))?;
    let request_json = String::from_utf8(request_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in request: {}", e)))?;

    let request: FileReadRequest = serde_json::from_str(&request_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse request: {}", e)))?;

    let state = caller.data().clone();
    let rt = tokio::runtime::Handle::current();

    // Execute VFS operation
    let result = rt.block_on(async {
        let vfs_bridge = {
            let state_guard = state.lock().unwrap();
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
        Ok(response) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&response)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            let mut state_guard = state.lock().unwrap();
            state_guard.store_result("vfs_read_file", result_json);
            Ok(0) // Success
        }
        Err(e) => {
            let mut state_guard = state.lock().unwrap();
            state_guard.store_error("vfs_read_file", &e.to_string());
            Ok(-1) // Error
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
    caller.data().lock().unwrap().record_host_call();

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = data
        .get(namespace_ptr as usize..(namespace_ptr + namespace_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read namespace from memory"))?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;

    // Read identifier from memory
    let identifier_bytes = data
        .get(identifier_ptr as usize..(identifier_ptr + identifier_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read identifier from memory"))?;
    let identifier_json = String::from_utf8(identifier_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in identifier: {}", e)))?;

    let identifier: FileIdentifier = serde_json::from_str(&identifier_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse identifier: {}", e)))?;

    let state = caller.data().clone();
    let rt = tokio::runtime::Handle::current();

    // Execute VFS operation
    let result = rt.block_on(async {
        let vfs_bridge = {
            let state_guard = state.lock().unwrap();
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
        Ok(_) => {
            let mut state_guard = state.lock().unwrap();
            state_guard.store_result("vfs_delete_file", "true".to_string());
            Ok(0) // Success
        }
        Err(e) => {
            let mut state_guard = state.lock().unwrap();
            state_guard.store_error("vfs_delete_file", &e.to_string());
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
    caller.data().lock().unwrap().record_host_call();

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = data
        .get(namespace_ptr as usize..(namespace_ptr + namespace_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read namespace from memory"))?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;

    // Read request from memory
    let request_bytes = data
        .get(request_ptr as usize..(request_ptr + request_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read request from memory"))?;
    let request_json = String::from_utf8(request_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in request: {}", e)))?;

    let request: FileListRequest = serde_json::from_str(&request_json)
        .map_err(|e| wasmtime::Error::msg(format!("failed to parse request: {}", e)))?;

    let state = caller.data().clone();
    let rt = tokio::runtime::Handle::current();

    // Execute VFS operation
    let result = rt.block_on(async {
        let vfs_bridge = {
            let state_guard = state.lock().unwrap();
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
        Ok(response) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&response)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            let mut state_guard = state.lock().unwrap();
            state_guard.store_result("vfs_list_files", result_json);
            Ok(0) // Success
        }
        Err(e) => {
            let mut state_guard = state.lock().unwrap();
            state_guard.store_error("vfs_list_files", &e.to_string());
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
    caller.data().lock().unwrap().record_host_call();

    let memory = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| wasmtime::Error::msg("failed to find host memory"))?;

    let data = memory.data(&caller);

    // Read namespace from memory
    let namespace_bytes = data
        .get(namespace_ptr as usize..(namespace_ptr + namespace_len) as usize)
        .ok_or_else(|| wasmtime::Error::msg("failed to read namespace from memory"))?;
    let namespace = String::from_utf8(namespace_bytes.to_vec())
        .map_err(|e| wasmtime::Error::msg(format!("invalid UTF-8 in namespace: {}", e)))?;

    let state = caller.data().clone();
    let rt = tokio::runtime::Handle::current();

    // Execute VFS operation
    let result = rt.block_on(async {
        let vfs_bridge = {
            let state_guard = state.lock().unwrap();
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
        Ok(stats) => {
            // Store result in plugin state for retrieval
            let result_json = serde_json::to_string(&stats)
                .map_err(|e| wasmtime::Error::msg(format!("failed to serialize result: {}", e)))?;

            let mut state_guard = state.lock().unwrap();
            state_guard.store_result("vfs_get_usage_stats", result_json);
            Ok(0) // Success
        }
        Err(e) => {
            let mut state_guard = state.lock().unwrap();
            state_guard.store_error("vfs_get_usage_stats", &e.to_string());
            Ok(-1) // Error
        }
    }
}

/// Register all VFS host functions with the linker
pub fn register_vfs_functions(linker: &mut Linker<HostStateRef>) -> wasmtime::Result<()> {
    linker.func_wrap("env", "vfs_write_file", vfs_write_file)?;
    linker.func_wrap("env", "vfs_read_file", vfs_read_file)?;
    linker.func_wrap("env", "vfs_delete_file", vfs_delete_file)?;
    linker.func_wrap("env", "vfs_list_files", vfs_list_files)?;
    linker.func_wrap("env", "vfs_get_usage_stats", vfs_get_usage_stats)?;
    Ok(())
}
