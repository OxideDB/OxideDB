//! Hello Plugin - A simple WASM plugin for OxideDB using the Plugin SDK
//!
//! This plugin demonstrates how to use the high-level Plugin SDK to:
//! 1. Handle database events (create, update, delete)
//! 2. Process and validate data
//! 3. Expose HTTP routes for custom business logic
//! 4. Perform CRUD operations on database records
//! 5. Implement security checks and data transformations

use oxide_plugin_sdk::prelude::*;

/// Hello Plugin - demonstrates the Plugin SDK capabilities
#[derive(Default)]
pub struct HelloPlugin;

impl PluginEventHandler for HelloPlugin {
    fn on_init(&mut self) -> PluginResult<()> {
        log_info!("Hello Plugin initializing...");
        
        // Register HTTP routes with their specific handler function names
        Http::register_route("GET", "/api/hello/items", "handle_get_items")?;
        Http::register_route("POST", "/api/hello/items", "handle_create_item")?;
        Http::register_route("GET", "/api/hello/items/:id", "handle_get_item")?;
        Http::register_route("POST", "/api/hello/process", "handle_process_data")?;
        
        log_info!("Hello Plugin routes registered successfully!");
        Ok(())
    }

    fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Processing create event for collection: {}", event.collection);

        // Security check: block access to restricted collections
        if event.collection.contains("admin") || event.collection.contains("system") {
            let error_msg = format!("Access denied to restricted collection: {}", event.collection);
            log_warn!("{}", error_msg);
            return Ok(PluginResponse::deny(error_msg));
        }

        // Parse and enhance the data
        let mut data: JsonValue = serde_json::from_str(&event.data)?;
        
        if let Some(obj) = data.as_object_mut() {
            // Add plugin metadata
            obj.insert("plugin_processed_at".to_string(), json!("2024-01-01T00:00:00Z"));
            obj.insert("plugin_name".to_string(), json!("hello-plugin"));
            obj.insert("plugin_version".to_string(), json!("1.0.0"));
            
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
        log_info!("Record created successfully in collection: {}", event.collection);

        // NOTE: We don't create audit logs from the event handler because it can cause
        // circular dependencies (the audit log creation would trigger another event).
        // In a production system, you might use a separate audit logging service
        // or queue the audit log for later processing.
        
        log_info!("Audit log would be created: action=record_created, collection={}", event.collection);

        Ok(PluginResponse::allow())
    }

    fn on_before_update(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Processing update event for collection: {}", event.collection);

        // Parse and validate the update data
        let mut data: JsonValue = serde_json::from_str(&event.data)?;

        if let Some(obj) = data.as_object_mut() {
            // Update the last modified timestamp
            obj.insert("plugin_updated_at".to_string(), json!("2024-01-01T00:00:00Z"));

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
        log_info!("Processing delete event for collection: {}", event.collection);

        // Parse the data to check if it's a protected record
        let data: JsonValue = serde_json::from_str(&event.data)?;
        
        if let Some(protected) = data.get("protected").and_then(|v| v.as_bool()) {
            if protected {
                log_warn!("Attempted to delete protected record");
                return Ok(PluginResponse::deny("Cannot delete protected records"));
            }
        }

        Ok(PluginResponse::allow())
    }

    fn on_cleanup(&mut self) -> PluginResult<()> {
        log_info!("Hello Plugin cleanup completed");
        Ok(())
    }
}

/// HTTP handler implementation using the proper SDK architecture
impl PluginHttpHandler for HelloPlugin {
    fn handle_request(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling HTTP request: {} {}", request.method, request.path);
        
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/api/hello/items") => self.handle_get_items(request),
            ("POST", "/api/hello/items") => self.handle_create_item(request),
            ("GET", path) if path.starts_with("/api/hello/items/") => self.handle_get_item(request),
            ("POST", "/api/hello/process") => self.handle_process_data(request),
            _ => {
                log_warn!("Route not found: {} {}", request.method, request.path);
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
                
                let items: Vec<JsonValue> = records.into_iter()
                    .map(|record| json!({
                        "id": record.id,
                        "data": record.data,
                        "created_at": record.created_at,
                        "updated_at": record.updated_at
                    }))
                    .collect();

                JsonResponseBuilder::new().json(&items)
            }
            Err(e) => {
                log_error!("Failed to read items: {}", e);
                
                // Return mock data for demo purposes
                let mock_items = json!([
                    {
                        "id": "item1",
                        "name": "Hello Item 1",
                        "description": "Created by Hello Plugin",
                        "created_at": "2024-01-01T00:00:00Z"
                    },
                    {
                        "id": "item2", 
                        "name": "Hello Item 2",
                        "description": "Another item from Hello Plugin",
                        "created_at": "2024-01-01T00:01:00Z"
                    }
                ]);
                
                JsonResponseBuilder::new().json(&mock_items)
            }
        }
    }

    /// Handle POST /api/hello/items - create a new item
    fn handle_create_item(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling POST /api/hello/items");

        if !request.is_json() {
            return Ok(HttpResponse::error(400, "Content-Type must be application/json"));
        }

        let item_data: JsonValue = request.body_json()?;

        // Validate required fields
        if !item_data.get("name").and_then(|v| v.as_str()).map_or(false, |s| !s.is_empty()) {
            return Ok(HttpResponse::error(400, "Name field is required"));
        }

        // Enhance the item with metadata
        let mut enhanced_item = item_data;
        if let Some(obj) = enhanced_item.as_object_mut() {
            obj.insert("created_by".to_string(), json!("hello-plugin"));
            obj.insert("created_at".to_string(), json!("2024-01-01T00:00:00Z"));
            obj.insert("plugin_version".to_string(), json!("1.0.0"));
        }

        // Create the record in the database
        match Database::create_typed("items", &enhanced_item) {
            Ok(record) => {
                log_info!("Item created with ID: {}", record.id);
                
                let response_data = json!({
                    "success": true,
                    "message": "Item created successfully",
                    "data": {
                        "id": record.id,
                        "data": record.data,
                        "created_at": record.created_at
                    }
                });

                JsonResponseBuilder::new().status(201).json(&response_data)
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

        let item_id = request.get_path_param("id")
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
                    "updated_at": record.updated_at
                });

                log_info!("Successfully retrieved item: {}", item_id);
                JsonResponseBuilder::new().json(&item)
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
                    "created_at": "2024-01-01T00:00:00Z",
                    "created_by": "hello-plugin"
                });
                
                JsonResponseBuilder::new().json(&mock_item)
            }
        }
    }

    /// Handle POST /api/hello/process - custom data processing
    fn handle_process_data(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling POST /api/hello/process");

        if !request.is_json() {
            return Ok(HttpResponse::error(400, "Content-Type must be application/json"));
        }

        let input_data: JsonValue = request.body_json()?;

        // Custom business logic - transform the data
        let processed_data = match input_data.clone() {
            JsonValue::Object(mut obj) => {
                // Add processing metadata
                obj.insert("processed_by".to_string(), json!("hello-plugin"));
                obj.insert("processed_at".to_string(), json!("2024-01-01T00:00:00Z"));
                
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
                        obj.insert("processed_by".to_string(), json!("hello-plugin"));
                    }
                }
                JsonValue::Array(arr)
            }
            other => {
                // For primitive values, wrap in an object
                json!({
                    "original_value": other,
                    "processed_by": "hello-plugin",
                    "processed_at": "2024-01-01T00:00:00Z"
                })
            }
        };

        // Optionally save the processed data
        if let Some(save_flag) = request.get_query("save") {
            if save_flag == "true" {
                if let Err(e) = Database::create("processed_data", &processed_data) {
                    log_warn!("Failed to save processed data: {}", e);
                } else {
                    log_info!("Processed data saved to database");
                }
            }
        }

        // Return the processed data
        let response = json!({
            "success": true,
            "message": "Data processed successfully",
            "input": input_data,
            "output": processed_data,
            "processing_info": {
                "plugin": "hello-plugin",
                "version": "1.0.0",
                "timestamp": "2024-01-01T00:00:00Z"
            }
        });

        log_info!("Successfully processed data");
        JsonResponseBuilder::new().json(&response)
    }
}

/// Helper function to capitalize names
fn capitalize_name(name: &str) -> String {
    name.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// Export the plugin with HTTP capabilities using the updated SDK architecture
export_http_plugin!(HelloPlugin);

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
}
