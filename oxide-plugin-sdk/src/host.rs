//! Host function interface for calling back to the OxideDB host

use crate::{
    DatabaseResult, EventPayload, FileIdentifier, FileListRequest, FileListResponse, FileMetadata,
    FileMoveRequest, FileReadRequest, FileReadResponse, FileWriteRequest, HttpRequestContext,
    HttpResponse, LogLevel, PluginError, PluginResult, VfsUsageStats,
};

// Raw host function imports
#[cfg(target_arch = "wasm32")]
extern "C" {
    // Event system functions
    fn get_event_payload() -> i32;
    fn get_result_ptr() -> i32;
    fn get_result_len() -> i32;

    // Logging functions
    fn log_info(ptr: *const u8, len: usize);
    fn log_error(ptr: *const u8, len: usize);
    fn set_error(ptr: *const u8, len: usize);

    // HTTP functions
    fn register_http_route(
        method_ptr: *const u8,
        method_len: usize,
        path_ptr: *const u8,
        path_len: usize,
        handler_ptr: *const u8,
        handler_len: usize,
    ) -> i32;

    // NOTE: Metadata functions removed - using TOML-only approach
    // Removed: set_plugin_metadata, get_plugin_metadata

    // Database functions
    fn create_record(
        collection_ptr: *const u8,
        collection_len: usize,
        data_ptr: *const u8,
        data_len: usize,
    ) -> i32;
    fn read_records(
        collection_ptr: *const u8,
        collection_len: usize,
        filter_ptr: *const u8,
        filter_len: usize,
    ) -> i32;
    fn update_record(
        collection_ptr: *const u8,
        collection_len: usize,
        record_id_ptr: *const u8,
        record_id_len: usize,
        data_ptr: *const u8,
        data_len: usize,
    ) -> i32;
    fn delete_record(
        collection_ptr: *const u8,
        collection_len: usize,
        record_id_ptr: *const u8,
        record_id_len: usize,
    ) -> i32;
    fn create_collection(schema_ptr: *const u8, schema_len: usize) -> i32;
    fn list_collections() -> i32;
    fn get_collection_schema(collection_ptr: *const u8, collection_len: usize) -> i32;
    fn update_collection_schema(
        collection_ptr: *const u8,
        collection_len: usize,
        schema_ptr: *const u8,
        schema_len: usize,
    ) -> i32;
    fn delete_collection(collection_ptr: *const u8, collection_len: usize) -> i32;
    fn collection_exists(collection_ptr: *const u8, collection_len: usize) -> i32;
    fn get_collection_stats(collection_ptr: *const u8, collection_len: usize) -> i32;

    // VFS functions
    fn vfs_write_file(
        namespace_ptr: *const u8,
        namespace_len: usize,
        request_ptr: *const u8,
        request_len: usize,
    ) -> i64;
    fn vfs_read_file(
        namespace_ptr: *const u8,
        namespace_len: usize,
        request_ptr: *const u8,
        request_len: usize,
    ) -> i64;
    fn vfs_move_file(
        namespace_ptr: *const u8,
        namespace_len: usize,
        request_ptr: *const u8,
        request_len: usize,
    ) -> i64;
    fn vfs_delete_file(
        namespace_ptr: *const u8,
        namespace_len: usize,
        identifier_ptr: *const u8,
        identifier_len: usize,
    ) -> i64;
    fn vfs_list_files(
        namespace_ptr: *const u8,
        namespace_len: usize,
        request_ptr: *const u8,
        request_len: usize,
    ) -> i64;
    fn vfs_get_usage_stats(namespace_ptr: *const u8, namespace_len: usize) -> i64;

    fn get_http_request() -> i32;
    fn set_http_response(
        status_code: i32,
        headers_ptr: *const u8,
        headers_len: usize,
        body_ptr: *const u8,
        body_len: usize,
    ) -> i32;
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn get_event_payload() -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn get_result_ptr() -> i32 {
    0
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn get_result_len() -> i32 {
    0
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn log_info(_ptr: *const u8, _len: usize) {}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn log_error(_ptr: *const u8, _len: usize) {}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn set_error(_ptr: *const u8, _len: usize) {}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn register_http_route(
    _method_ptr: *const u8,
    _method_len: usize,
    _path_ptr: *const u8,
    _path_len: usize,
    _handler_ptr: *const u8,
    _handler_len: usize,
) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn create_record(
    _collection_ptr: *const u8,
    _collection_len: usize,
    _data_ptr: *const u8,
    _data_len: usize,
) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn read_records(
    _collection_ptr: *const u8,
    _collection_len: usize,
    _filter_ptr: *const u8,
    _filter_len: usize,
) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn update_record(
    _collection_ptr: *const u8,
    _collection_len: usize,
    _record_id_ptr: *const u8,
    _record_id_len: usize,
    _data_ptr: *const u8,
    _data_len: usize,
) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn delete_record(
    _collection_ptr: *const u8,
    _collection_len: usize,
    _record_id_ptr: *const u8,
    _record_id_len: usize,
) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn create_collection(_schema_ptr: *const u8, _schema_len: usize) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn list_collections() -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn get_collection_schema(_collection_ptr: *const u8, _collection_len: usize) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn update_collection_schema(
    _collection_ptr: *const u8,
    _collection_len: usize,
    _schema_ptr: *const u8,
    _schema_len: usize,
) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn delete_collection(_collection_ptr: *const u8, _collection_len: usize) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn collection_exists(_collection_ptr: *const u8, _collection_len: usize) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn get_collection_stats(_collection_ptr: *const u8, _collection_len: usize) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn vfs_write_file(
    _namespace_ptr: *const u8,
    _namespace_len: usize,
    _request_ptr: *const u8,
    _request_len: usize,
) -> i64 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn vfs_read_file(
    _namespace_ptr: *const u8,
    _namespace_len: usize,
    _request_ptr: *const u8,
    _request_len: usize,
) -> i64 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn vfs_move_file(
    _namespace_ptr: *const u8,
    _namespace_len: usize,
    _request_ptr: *const u8,
    _request_len: usize,
) -> i64 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn vfs_delete_file(
    _namespace_ptr: *const u8,
    _namespace_len: usize,
    _identifier_ptr: *const u8,
    _identifier_len: usize,
) -> i64 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn vfs_list_files(
    _namespace_ptr: *const u8,
    _namespace_len: usize,
    _request_ptr: *const u8,
    _request_len: usize,
) -> i64 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn vfs_get_usage_stats(_namespace_ptr: *const u8, _namespace_len: usize) -> i64 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn get_http_request() -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn set_http_response(
    _status_code: i32,
    _headers_ptr: *const u8,
    _headers_len: usize,
    _body_ptr: *const u8,
    _body_len: usize,
) -> i32 {
    -1
}

/// High-level interface for calling host functions
pub struct Host;

impl Host {
    /// Get the current event payload
    pub fn get_event_payload() -> PluginResult<EventPayload> {
        unsafe {
            let result = get_event_payload();
            if result < 0 {
                return Err(PluginError::HostCallFailed(
                    "Failed to get event payload".to_string(),
                ));
            }

            let ptr = get_result_ptr();
            let len = get_result_len();

            if ptr == 0 || len == 0 {
                return Err(PluginError::HostCallFailed(
                    "Invalid payload pointer or length".to_string(),
                ));
            }

            let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            let json_str = std::str::from_utf8(slice)
                .map_err(|e| PluginError::InvalidData(format!("Invalid UTF-8: {}", e)))?;

            serde_json::from_str(json_str).map_err(PluginError::JsonError)
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

    /// Register an HTTP route with the specified method, path, and handler function name
    pub fn register_http_route(
        method: &str,
        path: &str,
        handler_function: &str,
    ) -> PluginResult<()> {
        let method_bytes = method.as_bytes();
        let path_bytes = path.as_bytes();
        let handler_bytes = handler_function.as_bytes();

        unsafe {
            let result = register_http_route(
                method_bytes.as_ptr(),
                method_bytes.len(),
                path_bytes.as_ptr(),
                path_bytes.len(),
                handler_bytes.as_ptr(),
                handler_bytes.len(),
            );

            if result == 0 {
                Ok(())
            } else {
                Err(PluginError::HostCallFailed(format!(
                    "Failed to register route: {} {}",
                    method, path
                )))
            }
        }
    }

    /// Create a record in a collection
    pub fn create_record<T: serde::Serialize>(
        collection: &str,
        data: &T,
    ) -> PluginResult<DatabaseResult> {
        let data_json = serde_json::to_string(data)?;
        let collection_bytes = collection.as_bytes();
        let data_bytes = data_json.as_bytes();

        unsafe {
            let result = create_record(
                collection_bytes.as_ptr(),
                collection_bytes.len(),
                data_bytes.as_ptr(),
                data_bytes.len(),
            );

            if result == 0 {
                // Get the result from the host
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to create record".to_string(),
                ))
            }
        }
    }

    /// Read records from a collection
    pub fn read_records(
        collection: &str,
        filter: Option<&serde_json::Value>,
    ) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();

        unsafe {
            let result = if let Some(filter) = filter {
                let filter_json = serde_json::to_string(filter).map_err(PluginError::JsonError)?;
                let filter_bytes = filter_json.as_bytes();
                read_records(
                    collection_bytes.as_ptr(),
                    collection_bytes.len(),
                    filter_bytes.as_ptr(),
                    filter_bytes.len(),
                )
            } else {
                read_records(
                    collection_bytes.as_ptr(),
                    collection_bytes.len(),
                    std::ptr::null(),
                    0,
                )
            };

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to read records".to_string(),
                ))
            }
        }
    }

    /// Update a record in a collection
    pub fn update_record<T: serde::Serialize>(
        collection: &str,
        record_id: &str,
        data: &T,
    ) -> PluginResult<DatabaseResult> {
        let data_json = serde_json::to_string(data)?;
        let collection_bytes = collection.as_bytes();
        let record_id_bytes = record_id.as_bytes();
        let data_bytes = data_json.as_bytes();

        unsafe {
            let result = update_record(
                collection_bytes.as_ptr(),
                collection_bytes.len(),
                record_id_bytes.as_ptr(),
                record_id_bytes.len(),
                data_bytes.as_ptr(),
                data_bytes.len(),
            );

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to update record".to_string(),
                ))
            }
        }
    }

    /// Delete a record from a collection
    pub fn delete_record(collection: &str, record_id: &str) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();
        let record_id_bytes = record_id.as_bytes();

        unsafe {
            let result = delete_record(
                collection_bytes.as_ptr(),
                collection_bytes.len(),
                record_id_bytes.as_ptr(),
                record_id_bytes.len(),
            );

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to delete record".to_string(),
                ))
            }
        }
    }

    /// Create a collection from a serialized collection schema.
    pub fn create_collection<T: serde::Serialize>(schema: &T) -> PluginResult<DatabaseResult> {
        let schema_json = serde_json::to_string(schema)?;
        let schema_bytes = schema_json.as_bytes();

        unsafe {
            let result = create_collection(schema_bytes.as_ptr(), schema_bytes.len());

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to create collection".to_string(),
                ))
            }
        }
    }

    /// List collection schemas visible to the plugin.
    pub fn list_collections() -> PluginResult<DatabaseResult> {
        unsafe {
            let result = list_collections();

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to list collections".to_string(),
                ))
            }
        }
    }

    /// Read the schema for a collection.
    pub fn get_collection_schema(collection: &str) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();

        unsafe {
            let result = get_collection_schema(collection_bytes.as_ptr(), collection_bytes.len());

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to get collection schema".to_string(),
                ))
            }
        }
    }

    /// Update the schema for an existing collection.
    pub fn update_collection_schema<T: serde::Serialize>(
        collection: &str,
        schema: &T,
    ) -> PluginResult<DatabaseResult> {
        let schema_json = serde_json::to_string(schema)?;
        let collection_bytes = collection.as_bytes();
        let schema_bytes = schema_json.as_bytes();

        unsafe {
            let result = update_collection_schema(
                collection_bytes.as_ptr(),
                collection_bytes.len(),
                schema_bytes.as_ptr(),
                schema_bytes.len(),
            );

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to update collection schema".to_string(),
                ))
            }
        }
    }

    /// Delete a collection and its records.
    pub fn delete_collection(collection: &str) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();

        unsafe {
            let result = delete_collection(collection_bytes.as_ptr(), collection_bytes.len());

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to delete collection".to_string(),
                ))
            }
        }
    }

    /// Check whether a collection exists.
    pub fn collection_exists(collection: &str) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();

        unsafe {
            let result = collection_exists(collection_bytes.as_ptr(), collection_bytes.len());

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to check collection existence".to_string(),
                ))
            }
        }
    }

    /// Read collection statistics.
    pub fn get_collection_stats(collection: &str) -> PluginResult<DatabaseResult> {
        let collection_bytes = collection.as_bytes();

        unsafe {
            let result = get_collection_stats(collection_bytes.as_ptr(), collection_bytes.len());

            if result == 0 {
                Self::get_database_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to get collection stats".to_string(),
                ))
            }
        }
    }

    /// Write a file to a VFS namespace.
    pub fn vfs_write_file(
        namespace: &str,
        request: &FileWriteRequest,
    ) -> PluginResult<FileMetadata> {
        let request_json = serde_json::to_string(request)?;
        let namespace_bytes = namespace.as_bytes();
        let request_bytes = request_json.as_bytes();

        unsafe {
            let result = vfs_write_file(
                namespace_bytes.as_ptr(),
                namespace_bytes.len(),
                request_bytes.as_ptr(),
                request_bytes.len(),
            );

            if result == 0 {
                Self::get_json_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to write VFS file".to_string(),
                ))
            }
        }
    }

    /// Read a file from a VFS namespace.
    pub fn vfs_read_file(
        namespace: &str,
        request: &FileReadRequest,
    ) -> PluginResult<FileReadResponse> {
        let request_json = serde_json::to_string(request)?;
        let namespace_bytes = namespace.as_bytes();
        let request_bytes = request_json.as_bytes();

        unsafe {
            let result = vfs_read_file(
                namespace_bytes.as_ptr(),
                namespace_bytes.len(),
                request_bytes.as_ptr(),
                request_bytes.len(),
            );

            if result == 0 {
                Self::get_json_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to read VFS file".to_string(),
                ))
            }
        }
    }

    /// Move or rename a file in a VFS namespace.
    pub fn vfs_move_file(namespace: &str, request: &FileMoveRequest) -> PluginResult<FileMetadata> {
        let request_json = serde_json::to_string(request)?;
        let namespace_bytes = namespace.as_bytes();
        let request_bytes = request_json.as_bytes();

        unsafe {
            let result = vfs_move_file(
                namespace_bytes.as_ptr(),
                namespace_bytes.len(),
                request_bytes.as_ptr(),
                request_bytes.len(),
            );

            if result == 0 {
                Self::get_json_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to move VFS file".to_string(),
                ))
            }
        }
    }

    /// Delete a file from a VFS namespace.
    pub fn vfs_delete_file(namespace: &str, identifier: &FileIdentifier) -> PluginResult<()> {
        let identifier_json = serde_json::to_string(identifier)?;
        let namespace_bytes = namespace.as_bytes();
        let identifier_bytes = identifier_json.as_bytes();

        unsafe {
            let result = vfs_delete_file(
                namespace_bytes.as_ptr(),
                namespace_bytes.len(),
                identifier_bytes.as_ptr(),
                identifier_bytes.len(),
            );

            if result == 0 {
                let _: bool = Self::get_json_result()?;
                Ok(())
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to delete VFS file".to_string(),
                ))
            }
        }
    }

    /// List files in a VFS namespace.
    pub fn vfs_list_files(
        namespace: &str,
        request: &FileListRequest,
    ) -> PluginResult<FileListResponse> {
        let request_json = serde_json::to_string(request)?;
        let namespace_bytes = namespace.as_bytes();
        let request_bytes = request_json.as_bytes();

        unsafe {
            let result = vfs_list_files(
                namespace_bytes.as_ptr(),
                namespace_bytes.len(),
                request_bytes.as_ptr(),
                request_bytes.len(),
            );

            if result == 0 {
                Self::get_json_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to list VFS files".to_string(),
                ))
            }
        }
    }

    /// Get usage statistics for a VFS namespace.
    pub fn vfs_get_usage_stats(namespace: &str) -> PluginResult<VfsUsageStats> {
        let namespace_bytes = namespace.as_bytes();

        unsafe {
            let result = vfs_get_usage_stats(namespace_bytes.as_ptr(), namespace_bytes.len());

            if result == 0 {
                Self::get_json_result()
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to get VFS usage stats".to_string(),
                ))
            }
        }
    }

    /// Get the current HTTP request context
    pub fn get_http_request() -> PluginResult<HttpRequestContext> {
        unsafe {
            let result = get_http_request();
            if result < 0 {
                return Err(PluginError::HostCallFailed(
                    "Failed to get HTTP request".to_string(),
                ));
            }

            let ptr = get_result_ptr();
            let len = get_result_len();

            if ptr == 0 || len == 0 {
                return Err(PluginError::HostCallFailed(
                    "Invalid request pointer or length".to_string(),
                ));
            }

            let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            let json_str = std::str::from_utf8(slice)
                .map_err(|e| PluginError::InvalidData(format!("Invalid UTF-8: {}", e)))?;

            serde_json::from_str(json_str).map_err(PluginError::JsonError)
        }
    }

    /// Set the HTTP response
    pub fn set_http_response(response: &HttpResponse) -> PluginResult<()> {
        let headers_json =
            serde_json::to_string(&response.headers).map_err(PluginError::JsonError)?;
        let headers_bytes = headers_json.as_bytes();
        let body_bytes = response.body.as_bytes();

        unsafe {
            let result = set_http_response(
                response.status_code as i32,
                headers_bytes.as_ptr(),
                headers_bytes.len(),
                body_bytes.as_ptr(),
                body_bytes.len(),
            );

            if result == 0 {
                Ok(())
            } else {
                Err(PluginError::HostCallFailed(
                    "Failed to set HTTP response".to_string(),
                ))
            }
        }
    }

    /// Get database operation result from host
    fn get_database_result() -> PluginResult<DatabaseResult> {
        unsafe {
            let ptr = get_result_ptr();
            let len = get_result_len();

            if ptr == 0 || len == 0 {
                return Ok(DatabaseResult {
                    success: false,
                    data: None,
                    error: Some("No result data".to_string()),
                });
            }

            let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            let json_str = std::str::from_utf8(slice)
                .map_err(|e| PluginError::InvalidData(format!("Invalid UTF-8: {}", e)))?;

            serde_json::from_str(json_str).map_err(PluginError::JsonError)
        }
    }

    /// Get a JSON operation result from the host result buffer.
    fn get_json_result<T: serde::de::DeserializeOwned>() -> PluginResult<T> {
        unsafe {
            let ptr = get_result_ptr();
            let len = get_result_len();

            if ptr == 0 || len == 0 {
                return Err(PluginError::HostCallFailed("No result data".to_string()));
            }

            let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
            let json_str = std::str::from_utf8(slice)
                .map_err(|e| PluginError::InvalidData(format!("Invalid UTF-8: {}", e)))?;

            serde_json::from_str(json_str).map_err(PluginError::JsonError)
        }
    }
}
