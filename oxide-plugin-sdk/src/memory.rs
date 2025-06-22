//! Memory management for WASM plugins

use std::sync::{Mutex, OnceLock};

/// Global buffer for sharing data with host
static RESPONSE_BUFFER: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

/// Memory manager for WASM plugin memory operations
pub struct MemoryManager;

impl MemoryManager {
    /// Initialize the memory manager
    pub fn init() {
        let _ = RESPONSE_BUFFER.set(Mutex::new(Vec::new()));
    }

    /// Set response data that the host can retrieve
    pub fn set_response(data: &[u8]) {
        let buffer = RESPONSE_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
        if let Ok(mut buf) = buffer.lock() {
            buf.clear();
            buf.extend_from_slice(data);
        }
    }

    /// Get response data length
    pub fn get_response_len() -> usize {
        let buffer = RESPONSE_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
        if let Ok(buf) = buffer.lock() {
            buf.len()
        } else {
            0
        }
    }

    /// Get response data pointer
    pub fn get_response_ptr() -> *const u8 {
        let buffer = RESPONSE_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
        if let Ok(buf) = buffer.lock() {
            buf.as_ptr()
        } else {
            std::ptr::null()
        }
    }

    /// Clear the response buffer
    pub fn clear_response() {
        let buffer = RESPONSE_BUFFER.get_or_init(|| Mutex::new(Vec::new()));
        if let Ok(mut buf) = buffer.lock() {
            buf.clear();
        }
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
    let slice = std::slice::from_raw_parts(ptr, len);
    MemoryManager::set_response(slice);
}

/// Get response data length
#[no_mangle]
pub extern "C" fn get_response_len() -> usize {
    MemoryManager::get_response_len()
}

/// Get response data pointer
#[no_mangle]
pub extern "C" fn get_response_ptr() -> *const u8 {
    MemoryManager::get_response_ptr()
} 