//! Macros for simplifying plugin development

/// Macro to export the main plugin functions with minimal boilerplate
/// 
/// This macro generates all the required WASM exports for the plugin,
/// handling the low-level FFI and calling the appropriate methods on your plugin.
/// 
/// # Example
/// 
/// ```rust
/// use oxide_plugin_sdk::prelude::*;
/// 
/// struct MyPlugin;
/// 
/// impl PluginEventHandler for MyPlugin {
///     fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
///         log_info!("Creating record in collection: {}", event.collection);
///         Ok(PluginResponse::allow())
///     }
/// }
/// 
/// export_plugin!(MyPlugin);
/// ```
#[macro_export]
macro_rules! export_plugin {
    ($plugin_type:ty) => {
        // Initialize memory manager
        #[no_mangle]
        pub extern "C" fn plugin_init() -> i32 {
            use $crate::memory::MemoryManager;
            use $crate::{init_plugin, PluginEventHandler};
            
            MemoryManager::init();
            
            let plugin = <$plugin_type>::default();
            init_plugin(plugin);
            
            // Call the plugin's on_init method
            match $crate::with_plugin(|p| p.on_init()) {
                Ok(()) => {
                    $crate::log_info!("Plugin initialized successfully");
                    0
                }
                Err(e) => {
                    $crate::log_error!("Plugin initialization failed: {}", e);
                    1
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn plugin_cleanup() -> i32 {
            match $crate::with_plugin(|p| p.on_cleanup()) {
                Ok(()) => {
                    $crate::log_info!("Plugin cleanup completed");
                    0
                }
                Err(e) => {
                    $crate::log_error!("Plugin cleanup failed: {}", e);
                    1
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn on_before_create() -> i32 {
            $crate::handle_event_with_response(|plugin, event| plugin.on_before_create(event))
        }

        #[no_mangle]
        pub extern "C" fn on_after_create() -> i32 {
            $crate::handle_event_with_response(|plugin, event| plugin.on_after_create(event))
        }

        #[no_mangle]
        pub extern "C" fn on_before_update() -> i32 {
            $crate::handle_event_with_response(|plugin, event| plugin.on_before_update(event))
        }

        #[no_mangle]
        pub extern "C" fn on_after_update() -> i32 {
            $crate::handle_event_with_response(|plugin, event| plugin.on_after_update(event))
        }

        #[no_mangle]
        pub extern "C" fn on_before_delete() -> i32 {
            $crate::handle_event_with_response(|plugin, event| plugin.on_before_delete(event))
        }

        #[no_mangle]
        pub extern "C" fn on_after_delete() -> i32 {
            $crate::handle_event_with_response(|plugin, event| plugin.on_after_delete(event))
        }
    };
}

/// Macro to export a plugin with HTTP handling capabilities
/// 
/// This macro is like `export_plugin!` but also generates HTTP handler exports.
/// Your plugin struct must implement both `PluginEventHandler` and `PluginHttpHandler`.
/// 
/// # Example
/// 
/// ```rust
/// use oxide_plugin_sdk::prelude::*;
/// 
/// struct MyPlugin;
/// 
/// impl PluginEventHandler for MyPlugin {
///     // ... event handler methods
/// }
/// 
/// impl PluginHttpHandler for MyPlugin {
///     fn handle_request(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
///         HttpResponse::json(&serde_json::json!({"message": "Hello World"}))
///     }
/// }
/// 
/// export_http_plugin!(MyPlugin);
/// ```
#[cfg(feature = "http")]
#[macro_export]
macro_rules! export_http_plugin {
    ($plugin_type:ty) => {
        $crate::export_plugin!($plugin_type);

        // HTTP handler registration
        #[no_mangle]
        pub extern "C" fn register_routes() -> i32 {
            // This should be implemented by the plugin to register its routes
            // For now, return success
            0
        }

        // Generic HTTP request handler
        #[no_mangle]
        pub extern "C" fn handle_http_request() -> i32 {
            $crate::handle_http_request_impl()
        }
    };
}

/// Macro to easily register HTTP routes during plugin initialization
/// 
/// # Example
/// 
/// ```rust
/// register_routes! {
///     GET "/api/items" => handle_get_items,
///     POST "/api/items" => handle_create_item,
///     GET "/api/items/:id" => handle_get_item,
/// }
/// ```
#[macro_export]
macro_rules! register_routes {
    ($($method:ident $path:literal => $handler:ident),* $(,)?) => {
        pub fn register_plugin_routes() -> $crate::PluginResult<()> {
            $(
                $crate::host::Host::register_http_route(
                    stringify!($method),
                    $path,
                    stringify!($handler),
                )?;
            )*
            Ok(())
        }
    };
}

/// Macro for easy JSON responses in HTTP handlers
/// 
/// # Example
/// 
/// ```rust
/// fn handle_get_items(&mut self, _request: &HttpRequestContext) -> PluginResult<HttpResponse> {
///     let items = vec!["item1", "item2", "item3"];
///     json_response!(items)
/// }
/// ```
#[macro_export]
macro_rules! json_response {
    ($data:expr) => {
        $crate::HttpResponse::json(&$data).map_err(|e| $crate::PluginError::JsonError(e))
    };
}

/// Macro for easy error responses in HTTP handlers
/// 
/// # Example
/// 
/// ```rust
/// fn handle_request(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
///     if request.method != "GET" {
///         return error_response!(405, "Method not allowed");
///     }
///     // ... handle GET request
/// }
/// ```
#[macro_export]
macro_rules! error_response {
    ($status:expr, $message:expr) => {
        Ok($crate::HttpResponse::error($status, $message))
    };
}

/// Macro to create a simple text response
#[macro_export]
macro_rules! text_response {
    ($text:expr) => {
        Ok($crate::HttpResponse::text($text))
    };
} 