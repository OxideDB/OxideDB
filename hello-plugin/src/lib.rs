//! Hello Plugin - A simple WASM plugin for OxideDB using the Plugin SDK
//!
//! This plugin demonstrates how to use the high-level Plugin SDK to:
//! 1. Handle database events (create, update, delete)
//! 2. Process and validate data
//! 3. Expose HTTP routes for custom business logic
//! 4. Perform CRUD operations on database records
//! 5. Implement security checks and data transformations

use oxide_plugin_sdk::prelude::*;

// Plugin constants (sourced from plugin.toml)
const PLUGIN_NAME: &str = "hello-plugin";
const PLUGIN_VERSION: &str = "1.0.0";
const PLUGIN_AUTHOR: &str = "OxideDB Team <team@oxidedb.com>";
const PLUGIN_DESCRIPTION: &str = "A simple demonstration plugin for OxideDB that showcases basic CRUD operations and HTTP route handling";
const PLUGIN_HOMEPAGE: &str = "https://github.com/oxidedb/hello-plugin";
const SETTINGS_COLLECTION: &str = "_plugins";
const SETTINGS_METADATA_KEY: &str = "demo_settings";
const SETTINGS_SAVED_AT_KEY: &str = "demo_settings_saved_at";
const HELLO_MODES: [&str; 3] = ["Observe", "Transform records", "Block restricted writes"];

/// Hello Plugin - demonstrates the Plugin SDK capabilities
#[derive(Default)]
pub struct HelloPlugin;

impl HelloPlugin {
    // NOTE: Removed metadata() getter and create_metadata() functions
    // All plugin metadata is now sourced from plugin.toml files only

    /// Get current timestamp for consistent date formatting
    fn get_current_timestamp() -> String {
        // In a real plugin, you'd use proper time libraries
        // For demo purposes, we use a fixed timestamp
        "2024-01-01T00:00:00Z".to_string()
    }

    /// Get plugin runtime information
    fn get_runtime_info() -> serde_json::Value {
        serde_json::json!({
            "plugin_name": "hello-plugin",
            "plugin_version": "1.0.0",
            "sdk_version": "1.0.0",
            "runtime": "wasmtime",
            "build_timestamp": "2024-01-01T00:00:00Z"
        })
    }

    /// Get default settings for the packaged admin demo.
    fn default_settings() -> JsonValue {
        json!({
            "greeting": "Hello from a packaged plugin page",
            "mode": HELLO_MODES[0]
        })
    }

    /// Identify system collections that should not receive demo record mutations.
    fn is_system_collection(collection: &str) -> bool {
        collection.starts_with('_')
    }

    /// Validate and normalize settings saved from the admin page.
    fn sanitize_settings(input: &JsonValue) -> Result<JsonValue, HttpResponse> {
        let Some(obj) = input.as_object() else {
            return Err(HttpResponse::error(
                400,
                "Settings payload must be a JSON object",
            ));
        };

        let Some(raw_greeting) = obj.get("greeting").and_then(|value| value.as_str()) else {
            return Err(HttpResponse::error(400, "Greeting is required"));
        };
        let greeting = raw_greeting.trim();
        if greeting.is_empty() {
            return Err(HttpResponse::error(400, "Greeting cannot be empty"));
        }
        if greeting.chars().count() > 120 {
            return Err(HttpResponse::error(
                400,
                "Greeting must be 120 characters or fewer",
            ));
        }

        let Some(mode) = obj.get("mode").and_then(|value| value.as_str()) else {
            return Err(HttpResponse::error(400, "Mode is required"));
        };
        if !HELLO_MODES.contains(&mode) {
            return Err(HttpResponse::error(400, "Mode is not supported"));
        }

        Ok(json!({
            "greeting": greeting,
            "mode": mode
        }))
    }

    /// Find this plugin's persisted configuration record.
    fn find_plugin_record() -> PluginResult<Option<Record>> {
        let records = Database::read_typed(SETTINGS_COLLECTION)?;
        Ok(records.into_iter().find(|record| {
            record.data.get("name").and_then(|value| value.as_str()) == Some(PLUGIN_NAME)
        }))
    }

    /// Read persisted demo settings from this plugin's configuration metadata.
    fn read_persisted_settings(record_data: &JsonValue) -> Option<JsonValue> {
        record_data
            .get("metadata")
            .and_then(|metadata| metadata.get(SETTINGS_METADATA_KEY))
            .cloned()
    }

    /// Store demo settings in this plugin's configuration metadata.
    fn write_settings_to_record_data(
        mut record_data: JsonValue,
        settings: JsonValue,
    ) -> Result<JsonValue, HttpResponse> {
        let Some(record_obj) = record_data.as_object_mut() else {
            return Err(HttpResponse::error(
                500,
                "Plugin configuration record is malformed",
            ));
        };

        let metadata_entry = record_obj
            .entry("metadata".to_string())
            .or_insert_with(|| json!({}));
        if !metadata_entry.is_object() {
            *metadata_entry = json!({});
        }

        let Some(metadata_obj) = metadata_entry.as_object_mut() else {
            return Err(HttpResponse::error(
                500,
                "Plugin configuration metadata is malformed",
            ));
        };
        metadata_obj.insert(SETTINGS_METADATA_KEY.to_string(), settings);
        metadata_obj.insert(
            SETTINGS_SAVED_AT_KEY.to_string(),
            json!(Self::get_current_timestamp()),
        );

        Ok(record_data)
    }
}

impl PluginEventHandler for HelloPlugin {
    fn on_init(&mut self) -> PluginResult<()> {
        log_info!("Hello Plugin initializing...");
        log_info!("Plugin: hello-plugin v1.0.0 (metadata sourced from plugin.toml)");
        log_info!("Author: OxideDB Team <team@oxidedb.com>");
        log_info!("Description: A simple demonstration plugin for OxideDB");
        log_info!("Homepage: https://github.com/oxidedb/hello-plugin");

        // NOTE: All plugin metadata is now sourced from plugin.toml during installation

        // Register HTTP routes with their specific handler function names
        Http::register_route("GET", "/api/hello/items", "handle_get_items")?;
        Http::register_route("POST", "/api/hello/items", "handle_create_item")?;
        Http::register_route("GET", "/api/hello/items/:id", "handle_get_item")?;
        Http::register_route("POST", "/api/hello/process", "handle_process_data")?;
        Http::register_route("GET", "/api/hello/settings", "handle_get_settings")?;
        Http::register_route("POST", "/api/hello/settings", "handle_save_settings")?;
        Http::register_route("GET", "/api/hello/metadata", "handle_get_metadata")?;
        Http::register_route("GET", "/api/hello/status", "handle_get_status")?;

        log_info!("Hello Plugin routes registered successfully!");
        log_info!("Features enabled: database_operations, http_routes, data_validation, event_handling, security_checks");
        Ok(())
    }

    fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!(
            "Processing create event for collection: {}",
            event.collection
        );

        if Self::is_system_collection(&event.collection) {
            log_info!(
                "Skipping demo create mutations for system collection: {}",
                event.collection
            );
            return Ok(PluginResponse::allow());
        }

        // Security check: block access to restricted collections
        if event.collection.contains("admin") || event.collection.contains("system") {
            let error_msg = format!(
                "Access denied to restricted collection: {}",
                event.collection
            );
            log_warn!("{}", error_msg);
            return Ok(PluginResponse::deny(error_msg));
        }

        // Parse and enhance the data
        let mut data: JsonValue = serde_json::from_str(&event.data)?;

        if let Some(obj) = data.as_object_mut() {
            // Add comprehensive plugin metadata
            let runtime_info = Self::get_runtime_info();
            obj.insert(
                "plugin_processed_at".to_string(),
                json!(Self::get_current_timestamp()),
            );
            obj.insert("plugin_name".to_string(), json!(PLUGIN_NAME));
            obj.insert("plugin_version".to_string(), json!(PLUGIN_VERSION));
            obj.insert("plugin_author".to_string(), json!(PLUGIN_AUTHOR));
            obj.insert("plugin_runtime_info".to_string(), runtime_info);

            // Validate and enhance name field if present
            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                if name.trim().is_empty() {
                    return Ok(PluginResponse::deny("Name cannot be empty"));
                }

                // Capitalize the name
                obj.insert("name".to_string(), json!(capitalize_name(name)));
            }
        }

        log_info!("Data validation and enhancement completed");
        Ok(PluginResponse::allow_with_data(&data)?)
    }

    fn on_after_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!(
            "Record created successfully in collection: {}",
            event.collection
        );

        // NOTE: We don't create audit logs from the event handler because it can cause
        // circular dependencies (the audit log creation would trigger another event).
        // In a production system, you might use a separate audit logging service
        // or queue the audit log for later processing.

        log_info!(
            "Audit log would be created: action=record_created, collection={}, plugin={} v{}",
            event.collection,
            PLUGIN_NAME,
            PLUGIN_VERSION
        );

        Ok(PluginResponse::allow())
    }

    fn on_before_update(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!(
            "Processing update event for collection: {}",
            event.collection
        );

        if Self::is_system_collection(&event.collection) {
            log_info!(
                "Skipping demo update mutations for system collection: {}",
                event.collection
            );
            return Ok(PluginResponse::allow());
        }

        // Parse and validate the update data
        let mut data: JsonValue = serde_json::from_str(&event.data)?;

        if let Some(obj) = data.as_object_mut() {
            // Update the last modified timestamp with metadata
            obj.insert(
                "plugin_updated_at".to_string(),
                json!(Self::get_current_timestamp()),
            );
            obj.insert(
                "plugin_updated_by".to_string(),
                json!(format!("{} v{}", PLUGIN_NAME, PLUGIN_VERSION)),
            );

            // Validate name field if being updated
            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                if name.trim().is_empty() {
                    return Ok(PluginResponse::deny("Name cannot be empty"));
                }

                obj.insert("name".to_string(), json!(capitalize_name(name)));
            }
        }

        Ok(PluginResponse::allow_with_data(&data)?)
    }

    fn on_before_delete(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!(
            "Processing delete event for collection: {}",
            event.collection
        );

        if Self::is_system_collection(&event.collection) {
            log_info!(
                "Skipping demo delete checks for system collection: {}",
                event.collection
            );
            return Ok(PluginResponse::allow());
        }

        // Parse the data to check if it's a protected record
        let data: JsonValue = serde_json::from_str(&event.data)?;

        if let Some(protected) = data.get("protected").and_then(|v| v.as_bool()) {
            if protected {
                log_warn!(
                    "Attempted to delete protected record (blocked by {} v{})",
                    PLUGIN_NAME,
                    PLUGIN_VERSION
                );
                return Ok(PluginResponse::deny("Cannot delete protected records"));
            }
        }

        Ok(PluginResponse::allow())
    }

    fn on_cleanup(&mut self) -> PluginResult<()> {
        log_info!(
            "Hello Plugin cleanup completed for {} v{}",
            PLUGIN_NAME,
            PLUGIN_VERSION
        );
        Ok(())
    }
}

/// HTTP handler implementation using the proper SDK architecture
impl PluginHttpHandler for HelloPlugin {
    fn handle_request(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!(
            "Handling HTTP request: {} {} (plugin: {} v{})",
            request.method,
            request.path,
            PLUGIN_NAME,
            PLUGIN_VERSION
        );

        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/api/hello/items") => self.handle_get_items(request),
            ("POST", "/api/hello/items") => self.handle_create_item(request),
            ("GET", path) if path.starts_with("/api/hello/items/") => self.handle_get_item(request),
            ("POST", "/api/hello/process") => self.handle_process_data(request),
            ("GET", "/api/hello/settings") => self.handle_get_settings(request),
            ("POST", "/api/hello/settings") => self.handle_save_settings(request),
            ("GET", "/api/hello/metadata") => self.handle_get_metadata(request),
            ("GET", "/api/hello/status") => self.handle_get_status(request),
            _ => {
                log_warn!(
                    "Route not found: {} {} (plugin: {})",
                    request.method,
                    request.path,
                    PLUGIN_NAME
                );
                Ok(HttpResponse::error(404, "Route not found"))
            }
        }
    }
}

impl HelloPlugin {
    /// Handle GET /api/hello/items - retrieve all items
    fn handle_get_items(&mut self, _request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling GET /api/hello/items");

        // Read records from the 'items' collection
        match Database::read_typed("items") {
            Ok(records) => {
                log_info!("Retrieved {} items from database", records.len());

                let items: Vec<JsonValue> = records
                    .into_iter()
                    .map(|record| {
                        json!({
                            "id": record.id,
                            "data": record.data,
                            "created_at": record.created_at,
                            "updated_at": record.updated_at
                        })
                    })
                    .collect();

                JsonResponseBuilder::new()
                    .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
                    .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
                    .json(&items)
            }
            Err(e) => {
                log_error!("Failed to read items: {}", e);

                // Return mock data for demo purposes with metadata
                let mock_items = json!([
                    {
                        "id": "item1",
                        "name": "Hello Item 1",
                        "description": "Created by Hello Plugin",
                        "created_at": Self::get_current_timestamp(),
                        "plugin_info": Self::get_runtime_info()
                    },
                    {
                        "id": "item2",
                        "name": "Hello Item 2",
                        "description": "Another item from Hello Plugin",
                        "created_at": Self::get_current_timestamp(),
                        "plugin_info": Self::get_runtime_info()
                    }
                ]);

                JsonResponseBuilder::new()
                    .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
                    .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
                    .header("X-Data-Source".to_string(), "mock".to_string())
                    .json(&mock_items)
            }
        }
    }

    /// Handle POST /api/hello/items - create a new item
    fn handle_create_item(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling POST /api/hello/items");

        if !request.is_json() {
            return Ok(HttpResponse::error(
                400,
                "Content-Type must be application/json",
            ));
        }

        let item_data: JsonValue = request.body_json()?;

        // Validate required fields
        if item_data
            .get("name")
            .and_then(|v| v.as_str())
            .is_none_or(str::is_empty)
        {
            return Ok(HttpResponse::error(400, "Name field is required"));
        }

        // Enhance the item with comprehensive metadata
        let mut enhanced_item = item_data;
        if let Some(obj) = enhanced_item.as_object_mut() {
            let runtime_info = Self::get_runtime_info();
            obj.insert(
                "created_by".to_string(),
                json!(format!("{} v{}", PLUGIN_NAME, PLUGIN_VERSION)),
            );
            obj.insert(
                "created_at".to_string(),
                json!(Self::get_current_timestamp()),
            );
            obj.insert("plugin_info".to_string(), runtime_info);
            obj.insert("plugin_author".to_string(), json!(PLUGIN_AUTHOR));
            obj.insert("plugin_homepage".to_string(), json!(PLUGIN_HOMEPAGE));
        }

        // Create the record in the database
        match Database::create_typed("items", &enhanced_item) {
            Ok(record) => {
                log_info!(
                    "Item created with ID: {} by plugin {} v{}",
                    record.id,
                    PLUGIN_NAME,
                    PLUGIN_VERSION
                );

                let response_data = json!({
                    "success": true,
                    "message": "Item created successfully",
                    "data": {
                        "id": record.id,
                        "data": record.data,
                        "created_at": record.created_at
                    },
                    "plugin_info": {
                        "name": PLUGIN_NAME,
                        "version": PLUGIN_VERSION,
                        "author": PLUGIN_AUTHOR
                    }
                });

                JsonResponseBuilder::new()
                    .status(201)
                    .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
                    .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
                    .json(&response_data)
            }
            Err(e) => {
                log_error!("Failed to create item: {}", e);
                Ok(HttpResponse::error(500, "Failed to create item"))
            }
        }
    }

    /// Handle GET /api/hello/items/:id - retrieve a specific item
    fn handle_get_item(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling GET /api/hello/items/:id");

        let item_id = request
            .get_path_param("id")
            .ok_or_else(|| PluginError::InvalidData("Item ID not found in path".to_string()))?;

        // Query for the specific item
        let filter = json!({"id": item_id});
        match Database::read_typed_with_filter("items", &filter) {
            Ok(mut records) if !records.is_empty() => {
                let record = records.remove(0);
                let item = json!({
                    "id": record.id,
                    "data": record.data,
                    "created_at": record.created_at,
                    "updated_at": record.updated_at,
                    "retrieved_by": format!("{} v{}", PLUGIN_NAME, PLUGIN_VERSION),
                    "retrieved_at": Self::get_current_timestamp()
                });

                log_info!(
                    "Successfully retrieved item: {} via plugin {}",
                    item_id,
                    PLUGIN_NAME
                );
                JsonResponseBuilder::new()
                    .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
                    .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
                    .json(&item)
            }
            Ok(_) => {
                log_warn!("Item not found: {}", item_id);
                Ok(HttpResponse::error(404, "Item not found"))
            }
            Err(e) => {
                log_error!("Failed to read item {}: {}", item_id, e);

                // Return mock data for demo purposes
                let mock_item = json!({
                    "id": item_id,
                    "name": format!("Hello Item {}", item_id),
                    "description": "Specific item retrieved by Hello Plugin",
                    "created_at": Self::get_current_timestamp(),
                    "created_by": format!("{} v{}", PLUGIN_NAME, PLUGIN_VERSION),
                    "plugin_info": Self::get_runtime_info()
                });

                JsonResponseBuilder::new()
                    .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
                    .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
                    .header("X-Data-Source".to_string(), "mock".to_string())
                    .json(&mock_item)
            }
        }
    }

    /// Handle POST /api/hello/process - custom data processing
    fn handle_process_data(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling POST /api/hello/process");

        if !request.is_json() {
            return Ok(HttpResponse::error(
                400,
                "Content-Type must be application/json",
            ));
        }

        let input_data: JsonValue = request.body_json()?;

        // Custom business logic - transform the data
        let processed_data = match input_data.clone() {
            JsonValue::Object(mut obj) => {
                // Add comprehensive processing metadata
                let runtime_info = Self::get_runtime_info();
                obj.insert(
                    "processed_by".to_string(),
                    json!(format!("{} v{}", PLUGIN_NAME, PLUGIN_VERSION)),
                );
                obj.insert(
                    "processed_at".to_string(),
                    json!(Self::get_current_timestamp()),
                );
                obj.insert("processor_info".to_string(), runtime_info);
                obj.insert("processor_author".to_string(), json!(PLUGIN_AUTHOR));

                // Example transformation: uppercase all string values
                for (_key, value) in obj.iter_mut() {
                    if let Some(string_val) = value.as_str() {
                        *value = json!(string_val.to_uppercase());
                    }
                }

                JsonValue::Object(obj)
            }
            JsonValue::Array(mut arr) => {
                // Process each item in the array
                for item in arr.iter_mut() {
                    if let Some(obj) = item.as_object_mut() {
                        obj.insert(
                            "processed_by".to_string(),
                            json!(format!("{} v{}", PLUGIN_NAME, PLUGIN_VERSION)),
                        );
                        obj.insert(
                            "processed_at".to_string(),
                            json!(Self::get_current_timestamp()),
                        );
                    }
                }
                JsonValue::Array(arr)
            }
            other => {
                // For primitive values, wrap in an object
                json!({
                    "original_value": other,
                    "processed_by": format!("{} v{}", PLUGIN_NAME, PLUGIN_VERSION),
                    "processed_at": Self::get_current_timestamp(),
                    "processor_info": Self::get_runtime_info()
                })
            }
        };

        // Optionally save the processed data
        if let Some(save_flag) = request.get_query("save") {
            if save_flag == "true" {
                if let Err(e) = Database::create("processed_data", &processed_data) {
                    log_warn!("Failed to save processed data: {}", e);
                } else {
                    log_info!(
                        "Processed data saved to database by {} v{}",
                        PLUGIN_NAME,
                        PLUGIN_VERSION
                    );
                }
            }
        }

        // Return the processed data with metadata
        let response = json!({
            "success": true,
            "message": "Data processed successfully",
            "input": input_data,
            "output": processed_data,
            "processing_info": {
                "plugin": PLUGIN_NAME,
                "version": PLUGIN_VERSION,
                "author": PLUGIN_AUTHOR,
                "timestamp": Self::get_current_timestamp(),
                                 "features": vec!["database_operations", "http_routes", "data_validation", "event_handling", "security_checks"]
            }
        });

        log_info!(
            "Successfully processed data with {} v{}",
            PLUGIN_NAME,
            PLUGIN_VERSION
        );
        JsonResponseBuilder::new()
            .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
            .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
            .json(&response)
    }

    /// Handle GET /api/hello/settings - read persisted admin page settings
    fn handle_get_settings(&mut self, _request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling GET /api/hello/settings");

        let (settings, persisted) = match Self::find_plugin_record() {
            Ok(Some(record)) => match Self::read_persisted_settings(&record.data) {
                Some(settings) => (settings, true),
                None => (Self::default_settings(), false),
            },
            Ok(None) => {
                log_warn!(
                    "Plugin configuration record for {} was not found; returning defaults",
                    PLUGIN_NAME
                );
                (Self::default_settings(), false)
            }
            Err(e) => {
                log_error!("Failed to read plugin settings: {}", e);
                return Ok(HttpResponse::error(500, "Failed to read plugin settings"));
            }
        };

        let response = json!({
            "success": true,
            "settings": settings,
            "persisted": persisted,
            "plugin": {
                "name": PLUGIN_NAME,
                "version": PLUGIN_VERSION
            }
        });

        JsonResponseBuilder::new()
            .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
            .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
            .json(&response)
    }

    /// Handle POST /api/hello/settings - persist admin page settings
    fn handle_save_settings(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling POST /api/hello/settings");

        if !request.is_json() {
            return Ok(HttpResponse::error(
                400,
                "Content-Type must be application/json",
            ));
        }

        let input_data: JsonValue = request.body_json()?;
        let settings = match Self::sanitize_settings(&input_data) {
            Ok(settings) => settings,
            Err(response) => return Ok(response),
        };

        let record = match Self::find_plugin_record() {
            Ok(Some(record)) => record,
            Ok(None) => {
                log_error!(
                    "Cannot save settings because plugin configuration record for {} was not found",
                    PLUGIN_NAME
                );
                return Ok(HttpResponse::error(
                    404,
                    "Plugin configuration record not found",
                ));
            }
            Err(e) => {
                log_error!("Failed to read plugin settings before save: {}", e);
                return Ok(HttpResponse::error(500, "Failed to read plugin settings"));
            }
        };

        let record_id = record.id.clone();
        let updated_data = match Self::write_settings_to_record_data(record.data, settings.clone())
        {
            Ok(data) => data,
            Err(response) => return Ok(response),
        };

        match Database::update_typed(SETTINGS_COLLECTION, &record_id, &updated_data) {
            Ok(_) => {
                log_info!("Persisted admin page settings for {}", PLUGIN_NAME);
                let response = json!({
                    "success": true,
                    "message": "Settings saved",
                    "settings": settings,
                    "persisted": true,
                    "saved_at": Self::get_current_timestamp()
                });

                JsonResponseBuilder::new()
                    .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
                    .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
                    .json(&response)
            }
            Err(e) => {
                log_error!("Failed to save plugin settings: {}", e);
                Ok(HttpResponse::error(500, "Failed to save plugin settings"))
            }
        }
    }

    /// Handle GET /api/hello/metadata - get plugin metadata  
    fn handle_get_metadata(&mut self, _request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling GET /api/hello/metadata");

        let metadata_response = json!({
            "plugin_metadata": {
                "name": PLUGIN_NAME,
                "version": PLUGIN_VERSION,
                "author": PLUGIN_AUTHOR,
                "description": PLUGIN_DESCRIPTION,
                "homepage": PLUGIN_HOMEPAGE
            },
            "runtime_info": Self::get_runtime_info(),
            "capabilities": [
                "database_operations",
                "http_routes",
                "data_validation",
                "event_handling",
                "security_checks"
            ],
            "endpoints": [
                {"method": "GET", "path": "/api/hello/items", "description": "List all items"},
                {"method": "POST", "path": "/api/hello/items", "description": "Create a new item"},
                {"method": "GET", "path": "/api/hello/items/:id", "description": "Get specific item"},
                {"method": "POST", "path": "/api/hello/process", "description": "Process data"},
                {"method": "GET", "path": "/api/hello/settings", "description": "Read admin page settings"},
                {"method": "POST", "path": "/api/hello/settings", "description": "Save admin page settings"},
                {"method": "GET", "path": "/api/hello/metadata", "description": "Get plugin metadata"},
                {"method": "GET", "path": "/api/hello/status", "description": "Get plugin status"}
            ]
        });

        JsonResponseBuilder::new()
            .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
            .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
            .json(&metadata_response)
    }

    /// Handle GET /api/hello/status - get plugin status
    fn handle_get_status(&mut self, _request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling GET /api/hello/status");

        let status_response = json!({
            "status": "active",
            "health": "healthy",
            "plugin": {
                "name": PLUGIN_NAME,
                "version": PLUGIN_VERSION,
                "author": PLUGIN_AUTHOR,
                "description": PLUGIN_DESCRIPTION
            },
            "capabilities": {
                "database_access": true,
                "http_routes": true,
                "event_handling": true,
                "data_processing": true
            },
            "statistics": {
                "requests_handled": 0, // Would be tracked in real implementation
                "errors": 0,
                "uptime": "unknown"
            },
            "last_check": Self::get_current_timestamp()
        });

        JsonResponseBuilder::new()
            .header("X-Plugin-Name".to_string(), PLUGIN_NAME.to_string())
            .header("X-Plugin-Version".to_string(), PLUGIN_VERSION.to_string())
            .json(&status_response)
    }
}

/// Helper function to capitalize names
fn capitalize_name(name: &str) -> String {
    name.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// Export the plugin with all event and HTTP capabilities
oxide_plugin_sdk::export_http_plugin!(HelloPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capitalize_name() {
        assert_eq!(capitalize_name("hello world"), "Hello World");
        assert_eq!(capitalize_name("JOHN DOE"), "John Doe");
        assert_eq!(capitalize_name("mary jane"), "Mary Jane");
    }

    #[test]
    fn test_plugin_response_creation() {
        let response = PluginResponse::allow();
        assert!(response.allow);
        assert!(response.error_message.is_none());

        let deny_response = PluginResponse::deny("Test error");
        assert!(!deny_response.allow);
        assert_eq!(deny_response.error_message, Some("Test error".to_string()));
    }

    #[test]
    fn test_event_payload_parsing() {
        let payload = EventPayload {
            event_type: "BeforeCreate".to_string(),
            collection: "users".to_string(),
            data: r#"{"name": "John Doe"}"#.to_string(),
            metadata: json!({}),
        };

        let data: JsonValue = serde_json::from_str(&payload.data).unwrap();
        assert_eq!(data["name"], "John Doe");
    }

    #[test]
    fn test_runtime_info() {
        let runtime_info = HelloPlugin::get_runtime_info();
        assert_eq!(runtime_info["plugin_name"], "hello-plugin");
        assert_eq!(runtime_info["plugin_version"], "1.0.0");
        assert_eq!(runtime_info["runtime"], "wasmtime");
    }

    #[test]
    fn test_sanitize_settings_trims_greeting() {
        let settings = HelloPlugin::sanitize_settings(&json!({
            "greeting": "  Hello admin  ",
            "mode": "Observe"
        }))
        .expect("settings should be valid");

        assert_eq!(settings["greeting"], "Hello admin");
        assert_eq!(settings["mode"], "Observe");
    }

    #[test]
    fn test_sanitize_settings_rejects_unknown_mode() {
        let error = HelloPlugin::sanitize_settings(&json!({
            "greeting": "Hello admin",
            "mode": "Unknown"
        }))
        .expect_err("unknown mode should be rejected");

        assert_eq!(error.status_code, 400);
    }

    #[test]
    fn test_system_collections_are_skipped() {
        assert!(HelloPlugin::is_system_collection("_plugins"));
        assert!(!HelloPlugin::is_system_collection("items"));
    }

    #[test]
    fn test_write_settings_to_record_data_updates_metadata() {
        let updated = HelloPlugin::write_settings_to_record_data(
            json!({
                "name": PLUGIN_NAME,
                "metadata": {
                    "manifest": {"plugin": {"name": PLUGIN_NAME}}
                }
            }),
            json!({
                "greeting": "Saved",
                "mode": "Observe"
            }),
        )
        .expect("record data should update");

        assert_eq!(
            updated["metadata"][SETTINGS_METADATA_KEY],
            json!({
                "greeting": "Saved",
                "mode": "Observe"
            })
        );
        assert_eq!(
            updated["metadata"]["manifest"]["plugin"]["name"],
            PLUGIN_NAME
        );
        assert!(updated["metadata"][SETTINGS_SAVED_AT_KEY].is_string());
    }
}
