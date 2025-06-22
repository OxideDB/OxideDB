//! HTTP Plugin Example using OxideDB Plugin SDK
//! 
//! This example demonstrates how to create a plugin that handles HTTP requests
//! using the high-level SDK. Much simpler than the raw WASM approach!

use oxide_plugin_sdk::prelude::*;

/// An HTTP plugin that provides a REST API for managing items
#[derive(Default)]
pub struct HttpPlugin;

impl PluginEventHandler for HttpPlugin {
    fn on_init(&mut self) -> PluginResult<()> {
        log_info!("HTTP plugin initializing...");
        
        // Register HTTP routes
        Http::register_route("GET", "/api/items", "handle_get_items")?;
        Http::register_route("POST", "/api/items", "handle_create_item")?;
        Http::register_route("GET", "/api/items/:id", "handle_get_item")?;
        Http::register_route("PUT", "/api/items/:id", "handle_update_item")?;
        Http::register_route("DELETE", "/api/items/:id", "handle_delete_item")?;
        Http::register_route("POST", "/api/search", "handle_search_items")?;
        
        log_info!("HTTP routes registered successfully");
        Ok(())
    }

    fn on_cleanup(&mut self) -> PluginResult<()> {
        log_info!("HTTP plugin cleanup completed");
        Ok(())
    }
}

impl PluginHttpHandler for HttpPlugin {
    fn handle_request(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Handling {} {}", request.method, request.path);

        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/api/items") => self.handle_get_items(request),
            ("POST", "/api/items") => self.handle_create_item(request),
            ("GET", path) if path.starts_with("/api/items/") => self.handle_get_item(request),
            ("PUT", path) if path.starts_with("/api/items/") => self.handle_update_item(request),
            ("DELETE", path) if path.starts_with("/api/items/") => self.handle_delete_item(request),
            ("POST", "/api/search") => self.handle_search_items(request),
            _ => error_response!(404, "Route not found"),
        }
    }
}

impl HttpPlugin {
    /// GET /api/items - List all items
    fn handle_get_items(&mut self, _request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Fetching all items");

        match Database::read_typed("items") {
            Ok(records) => {
                let items: Vec<JsonValue> = records
                    .into_iter()
                    .map(|record| json!({
                        "id": record.id,
                        "data": record.data,
                        "created_at": record.created_at,
                        "updated_at": record.updated_at
                    }))
                    .collect();

                json_response!({
                    "success": true,
                    "data": items,
                    "count": items.len()
                })
            }
            Err(e) => {
                log_error!("Failed to fetch items: {}", e);
                error_response!(500, "Failed to fetch items")
            }
        }
    }

    /// POST /api/items - Create a new item
    fn handle_create_item(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Creating new item");

        if !Http::is_json_request(request) {
            return error_response!(400, "Content-Type must be application/json");
        }

        let item_data: JsonValue = Http::parse_json(request)?;

        // Validate required fields
        if !item_data.get("name").and_then(|v| v.as_str()).map_or(false, |s| !s.is_empty()) {
            return error_response!(400, "Name field is required");
        }

        // Enhance the item with metadata
        let mut enhanced_item = item_data;
        if let Some(obj) = enhanced_item.as_object_mut() {
            obj.insert("created_by".to_string(), json!("http-plugin"));
            obj.insert("created_at".to_string(), json!("2024-01-01T00:00:00Z"));
            obj.insert("plugin_version".to_string(), json!("1.0.0"));
        }

        match Database::create_typed("items", &enhanced_item) {
            Ok(record) => {
                log_info!("Item created with ID: {}", record.id);
                
                let response_data = json!({
                    "success": true,
                    "message": "Item created successfully",
                    "data": {
                        "id": record.id,
                        "data": record.data,
                        "created_at": record.created_at,
                        "updated_at": record.updated_at
                    }
                });

                JsonResponseBuilder::new()
                    .status(201)
                    .json(&response_data)
            }
            Err(e) => {
                log_error!("Failed to create item: {}", e);
                error_response!(500, "Failed to create item")
            }
        }
    }

    /// GET /api/items/:id - Get a specific item
    fn handle_get_item(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        let item_id = match request.get_path_param("id") {
            Some(id) => id,
            None => return error_response!(400, "Item ID not found in path"),
        };

        log_info!("Fetching item with ID: {}", item_id);

        // Create a filter for the specific item
        let filter = json!({"id": item_id});
        
        match Database::read_typed_with_filter("items", &filter) {
            Ok(mut records) => {
                if let Some(record) = records.pop() {
                    json_response!({
                        "success": true,
                        "data": {
                            "id": record.id,
                            "data": record.data,
                            "created_at": record.created_at,
                            "updated_at": record.updated_at
                        }
                    })
                } else {
                    error_response!(404, "Item not found")
                }
            }
            Err(e) => {
                log_error!("Failed to fetch item {}: {}", item_id, e);
                error_response!(500, "Failed to fetch item")
            }
        }
    }

    /// PUT /api/items/:id - Update a specific item
    fn handle_update_item(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        let item_id = match request.get_path_param("id") {
            Some(id) => id,
            None => return error_response!(400, "Item ID not found in path"),
        };

        log_info!("Updating item with ID: {}", item_id);

        if !Http::is_json_request(request) {
            return error_response!(400, "Content-Type must be application/json");
        }

        let mut update_data: JsonValue = Http::parse_json(request)?;

        // Enhance the update with metadata
        if let Some(obj) = update_data.as_object_mut() {
            obj.insert("updated_by".to_string(), json!("http-plugin"));
            obj.insert("updated_at".to_string(), json!("2024-01-01T00:00:00Z"));
        }

        match Database::update_typed("items", item_id, &update_data) {
            Ok(record) => {
                log_info!("Item {} updated successfully", item_id);
                
                json_response!({
                    "success": true,
                    "message": "Item updated successfully",
                    "data": {
                        "id": record.id,
                        "data": record.data,
                        "created_at": record.created_at,
                        "updated_at": record.updated_at
                    }
                })
            }
            Err(e) => {
                log_error!("Failed to update item {}: {}", item_id, e);
                error_response!(500, "Failed to update item")
            }
        }
    }

    /// DELETE /api/items/:id - Delete a specific item
    fn handle_delete_item(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        let item_id = match request.get_path_param("id") {
            Some(id) => id,
            None => return error_response!(400, "Item ID not found in path"),
        };

        log_info!("Deleting item with ID: {}", item_id);

        match Database::delete_typed("items", item_id) {
            Ok(record) => {
                log_info!("Item {} deleted successfully", item_id);
                
                json_response!({
                    "success": true,
                    "message": "Item deleted successfully",
                    "data": {
                        "id": record.id,
                        "data": record.data
                    }
                })
            }
            Err(e) => {
                log_error!("Failed to delete item {}: {}", item_id, e);
                error_response!(500, "Failed to delete item")
            }
        }
    }

    /// POST /api/search - Search items
    fn handle_search_items(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        log_info!("Searching items");

        if !Http::is_json_request(request) {
            return error_response!(400, "Content-Type must be application/json");
        }

        let search_params: JsonValue = Http::parse_json(request)?;

        // Build a filter from search parameters
        let mut filter = json!({});
        
        if let Some(search_obj) = search_params.as_object() {
            for (key, value) in search_obj {
                if key == "name" && value.is_string() {
                    // Simple name search (in a real implementation, you might want regex or fuzzy search)
                    filter[key] = value.clone();
                } else if key == "category" && value.is_string() {
                    filter[key] = value.clone();
                }
            }
        }

        match Database::read_typed_with_filter("items", &filter) {
            Ok(records) => {
                let items: Vec<JsonValue> = records
                    .into_iter()
                    .map(|record| json!({
                        "id": record.id,
                        "data": record.data,
                        "created_at": record.created_at,
                        "updated_at": record.updated_at
                    }))
                    .collect();

                json_response!({
                    "success": true,
                    "data": items,
                    "count": items.len(),
                    "search_params": search_params
                })
            }
            Err(e) => {
                log_error!("Failed to search items: {}", e);
                error_response!(500, "Failed to search items")
            }
        }
    }
}

// Export the HTTP plugin - this generates all the WASM exports automatically
export_http_plugin!(HttpPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_initialization() {
        let mut plugin = HttpPlugin::default();
        assert!(plugin.on_init().is_ok());
    }

    #[test]
    fn test_create_item_validation() {
        let mut plugin = HttpPlugin::default();
        
        // Test valid request
        let valid_request = HttpRequestContext {
            method: "POST".to_string(),
            path: "/api/items".to_string(),
            query_params: std::collections::HashMap::new(),
            headers: {
                let mut headers = std::collections::HashMap::new();
                headers.insert("content-type".to_string(), "application/json".to_string());
                headers
            },
            body: Some(r#"{"name": "Test Item", "category": "test"}"#.to_string()),
            path_params: std::collections::HashMap::new(),
            user: None,
        };

        // This would normally work with the actual database, but in tests we can't
        // easily mock the database calls, so we just test the request parsing
        assert!(Http::parse_json::<JsonValue>(&valid_request).is_ok());
    }

    #[test]
    fn test_invalid_json_request() {
        let invalid_request = HttpRequestContext {
            method: "POST".to_string(),
            path: "/api/items".to_string(),
            query_params: std::collections::HashMap::new(),
            headers: {
                let mut headers = std::collections::HashMap::new();
                headers.insert("content-type".to_string(), "application/json".to_string());
                headers
            },
            body: Some(r#"invalid json"#.to_string()),
            path_params: std::collections::HashMap::new(),
            user: None,
        };

        assert!(Http::parse_json::<JsonValue>(&invalid_request).is_err());
    }
} 