//! Host function interface for calling back to the OxideDB host

use crate::{PluginResult, PluginError, EventPayload, HttpRequestContext, HttpResponse, DatabaseResult, LogLevel};

// Raw host function imports
extern "C" {
    fn get_event_payload() -> i32;
    fn get_result_ptr() -> i32;
    fn get_result_len() -> i32;
    fn log_info(ptr: *const u8, len: usize);
    fn log_error(ptr: *const u8, len: usize);
    fn set_error(ptr: *const u8, len: usize);
    fn register_http_route(
        method_ptr: *const u8, method_len: usize,
        path_ptr: *const u8, path_len: usize,
        handler_ptr: *const u8, handler_len: usize,
    ) -> i32;
    fn create_record(
        collection_ptr: *const u8, collection_len: usize,
        data_ptr: *const u8, data_len: usize,
    ) -> i32;
    fn read_records(
        collection_ptr: *const u8, collection_len: usize,
        filter_ptr: *const u8, filter_len: usize,
    ) -> i32;
    fn update_record(
        collection_ptr: *const u8, collection_len: usize,
        record_id_ptr: *const u8, record_id_len: usize,
        data_ptr: *const u8, data_len: usize,
    ) -> i32;
    fn delete_record(
        collection_ptr: *const u8, collection_len: usize,
        record_id_ptr: *const u8, record_id_len: usize,
    ) -> i32;
    fn get_http_request() -> i32;
    fn set_http_response(
        status_code: i32,
        headers_ptr: *const u8, headers_len: usize,
        body_ptr: *const u8, body_len: usize,
    ) -> i32;
}

/// High-level interface for calling host functions
pub struct Host;

impl Host {
    /// Get the current event payload
    pub fn get_event_payload() -> PluginResult<EventPayload> {
        unsafe {
            let result = get_event_payload();
            if result < 0 {
                return Err(PluginError::HostCallFailed("Failed to get event payload".to_string()));
            }

            let ptr = get_result_ptr();
            let len = get_result_len();

            if ptr == 0 || len == 0 {
                return Err(PluginError::HostCallFailed("Invalid payload pointer or length".to_string()));
            }

            let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            let json_str = std::str::from_utf8(slice)
                .map_err(|e| PluginError::InvalidData(format!("Invalid UTF-8: {}", e)))?;

            serde_json::from_str(json_str)
                .map_err(|e| PluginError::JsonError(e))
        }
    }

    /// Log a message to the host
    pub fn log(level: LogLevel, message: &str) {
        let bytes = message.as_bytes();
        unsafe {
            match level {
                LogLevel::Info | LogLevel::Debug => {
                    log_info(bytes.as_ptr(), bytes.len());
                }
                LogLevel::Error | LogLevel::Warn => {
                    log_error(bytes.as_ptr(), bytes.len());
                }
            }
        }
    }

    /// Log an info message
    pub fn log_info(message: &str) {
        Self::log(LogLevel::Info, message);
    }

    /// Log an error message
    pub fn log_error(message: &str) {
        Self::log(LogLevel::Error, message);
    }

    /// Log a warning message
    pub fn log_warn(message: &str) {
        Self::log(LogLevel::Warn, message);
    }

    /// Log a debug message
    pub fn log_debug(message: &str) {
        Self::log(LogLevel::Debug, message);
    }

    /// Set an error that will prevent the operation from continuing
    pub fn set_error(message: &str) {
        let bytes = message.as_bytes();
        unsafe {
            set_error(bytes.as_ptr(), bytes.len());
        }
    }

    /// Register an HTTP route
    pub fn register_http_route(method: &str, path: &str, handler_function: &str) -> PluginResult<()> {
        let method_bytes = method.as_bytes();
        let path_bytes = path.as_bytes();
        let handler_bytes = handler_function.as_bytes();

        unsafe {
            let result = register_http_route(
                method_bytes.as_ptr(), method_bytes.len(),
                path_bytes.as_ptr(), path_bytes.len(),
                handler_bytes.as_ptr(), handler_bytes.len(),
            );

            if result == 0 {
                Ok(())
            } else {
                Err(PluginError::HostCallFailed(format!("Failed to register route: {} {}", method, path)))
            }
        }
    }

    /// Create a record in a collection
    pub fn create_record<T: serde::Serialize>(collection: &str, data: &T) -> PluginResult<DatabaseResult> {
        let data_json = serde_json::to_string(data)?;
        let collection_bytes = collection.as_bytes();
        let data_bytes = data_json.as_bytes();

        unsafe {
            let result = create_record(
                collection_bytes.as_ptr(), collection_bytes.len(),
                data_bytes.as_ptr(), data_bytes.len(),
            );

            if result == 0 {
                // Get the result from the host
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed("Failed to create record".to_string()))
            }
        }
    }

    /// Read records from a collection
    pub fn read_records(collection: &str, filter: Option<&serde_json::Value>) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();
        
        unsafe {
            let result = if let Some(filter) = filter {
                let filter_json = serde_json::to_string(filter)
                    .map_err(|e| PluginError::JsonError(e))?;
                let filter_bytes = filter_json.as_bytes();
                read_records(
                    collection_bytes.as_ptr(), collection_bytes.len(),
                    filter_bytes.as_ptr(), filter_bytes.len(),
                )
            } else {
                read_records(
                    collection_bytes.as_ptr(), collection_bytes.len(),
                    std::ptr::null(), 0,
                )
            };

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed("Failed to read records".to_string()))
            }
        }
    }

    /// Update a record in a collection
    pub fn update_record<T: serde::Serialize>(
        collection: &str, 
        record_id: &str, 
        data: &T
    ) -> PluginResult<DatabaseResult> {
        let data_json = serde_json::to_string(data)?;
        let collection_bytes = collection.as_bytes();
        let record_id_bytes = record_id.as_bytes();
        let data_bytes = data_json.as_bytes();

        unsafe {
            let result = update_record(
                collection_bytes.as_ptr(), collection_bytes.len(),
                record_id_bytes.as_ptr(), record_id_bytes.len(),
                data_bytes.as_ptr(), data_bytes.len(),
            );

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed("Failed to update record".to_string()))
            }
        }
    }

    /// Delete a record from a collection
    pub fn delete_record(collection: &str, record_id: &str) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();
        let record_id_bytes = record_id.as_bytes();

        unsafe {
            let result = delete_record(
                collection_bytes.as_ptr(), collection_bytes.len(),
                record_id_bytes.as_ptr(), record_id_bytes.len(),
            );

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed("Failed to delete record".to_string()))
            }
        }
    }

    /// Get the current HTTP request context
    pub fn get_http_request() -> PluginResult<HttpRequestContext> {
        unsafe {
            let result = get_http_request();
            if result < 0 {
                return Err(PluginError::HostCallFailed("Failed to get HTTP request".to_string()));
            }

            let ptr = get_result_ptr();
            let len = get_result_len();

            if ptr == 0 || len == 0 {
                return Err(PluginError::HostCallFailed("Invalid request pointer or length".to_string()));
            }

            let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            let json_str = std::str::from_utf8(slice)
                .map_err(|e| PluginError::InvalidData(format!("Invalid UTF-8: {}", e)))?;

            serde_json::from_str(json_str)
                .map_err(|e| PluginError::JsonError(e))
        }
    }

    /// Set the HTTP response
    pub fn set_http_response(response: &HttpResponse) -> PluginResult<()> {
        let headers_json = serde_json::to_string(&response.headers)?;
        let headers_bytes = headers_json.as_bytes();
        let body_bytes = response.body.as_bytes();

        unsafe {
            let result = set_http_response(
                response.status_code as i32,
                headers_bytes.as_ptr(), headers_bytes.len(),
                body_bytes.as_ptr(), body_bytes.len(),
            );

            if result == 0 {
                Ok(())
            } else {
                Err(PluginError::HostCallFailed("Failed to set HTTP response".to_string()))
            }
        }
    }

    /// Helper to get database operation result
    fn get_database_result() -> PluginResult<DatabaseResult> {
        // For now, return a success result
        // In a real implementation, this would parse the result from the host
        Ok(DatabaseResult {
            success: true,
            data: None,
            error: None,
        })
    }
}

 