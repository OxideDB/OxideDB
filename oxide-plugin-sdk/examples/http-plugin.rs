//! HTTP Plugin Example using OxideDB Plugin SDK
//! 
//! This example demonstrates how to create a plugin that handles database operations
//! using the high-level SDK. This is a simplified version that focuses on data operations.

use oxide_plugin_sdk::prelude::*;

/// A plugin that provides enhanced data management with validation and audit logging
#[derive(Default)]
pub struct HttpPlugin;

impl PluginEventHandler for HttpPlugin {
    fn on_init(&mut self) -> PluginResult<()> {
        log_info!("HTTP plugin initializing...");
        log_info!("Data management plugin ready for enhanced operations");
        Ok(())
    }

    fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Validating creation in collection: {}", event.collection);

        // Parse the incoming data
        let mut data: JsonValue = serde_json::from_str(&event.data)?;

        // Add metadata to track processing
        if let Some(obj) = data.as_object_mut() {
            obj.insert("processed_by".to_string(), json!("http-plugin"));
            obj.insert("processed_at".to_string(), json!("2024-01-01T00:00:00Z"));
            obj.insert("plugin_version".to_string(), json!("1.0.0"));

            // Validate required fields for 'items' collection
            if event.collection == "items" {
                if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                    if name.trim().is_empty() {
                        return Ok(PluginResponse::deny("Name field is required for items"));
                    }
                } else {
                    return Ok(PluginResponse::deny("Name field is required for items"));
                }
            }
        }

        log_info!("Data validation passed, metadata added");
        Ok(PluginResponse::allow_with_data(&data)?)
    }

    fn on_after_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Record created successfully in collection: {}", event.collection);

        // Create audit log entry
        let audit_entry = json!({
            "action": "create",
            "collection": event.collection,
            "timestamp": "2024-01-01T00:00:00Z",
            "plugin": "http-plugin",
            "data_size": event.data.len()
        });

        // Try to create audit log (ignore failures to not affect main operation)
        if let Err(e) = Database::create("_audit_log", &audit_entry) {
            log_warn!("Failed to create audit log: {}", e);
        } else {
            log_info!("Audit log created for {} operation", event.collection);
        }

        Ok(PluginResponse::allow())
    }

    fn on_before_update(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Validating update in collection: {}", event.collection);

        let mut data: JsonValue = serde_json::from_str(&event.data)?;

        if let Some(obj) = data.as_object_mut() {
            obj.insert("updated_by".to_string(), json!("http-plugin"));
            obj.insert("updated_at".to_string(), json!("2024-01-01T00:00:00Z"));

            // Validate name field if being updated in items collection
            if event.collection == "items" {
                if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                    if name.trim().is_empty() {
                        return Ok(PluginResponse::deny("Name cannot be empty"));
                    }
                }
            }
        }

        log_info!("Update validation passed");
        Ok(PluginResponse::allow_with_data(&data)?)
    }

    fn on_before_delete(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Validating deletion in collection: {}", event.collection);

        // Parse the data to check for protection flags
        let data: JsonValue = serde_json::from_str(&event.data)?;
        
        if let Some(protected) = data.get("protected").and_then(|v| v.as_bool()) {
            if protected {
                log_warn!("Attempted to delete protected record");
                return Ok(PluginResponse::deny("Cannot delete protected records"));
            }
        }

        // Don't allow deletion of system collections
        if event.collection.starts_with("_system") {
            log_warn!("Attempted to delete from system collection: {}", event.collection);
            return Ok(PluginResponse::deny("System collections cannot be modified"));
        }

        Ok(PluginResponse::allow())
    }

    fn on_after_delete(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Record deleted from collection: {}", event.collection);

        // Create deletion audit log
        let audit_entry = json!({
            "action": "delete",
            "collection": event.collection,
            "timestamp": "2024-01-01T00:00:00Z",
            "plugin": "http-plugin"
        });

        if let Err(e) = Database::create("_audit_log", &audit_entry) {
            log_warn!("Failed to create deletion audit log: {}", e);
        }

        Ok(PluginResponse::allow())
    }

    fn on_cleanup(&mut self) -> PluginResult<()> {
        log_info!("HTTP plugin cleanup completed");
        Ok(())
    }
}

// Export the plugin - this generates all the WASM exports automatically
oxide_plugin_sdk::export_plugin!(HttpPlugin);

/// This main function is just for compilation - plugins are normally compiled to WASM
fn main() {
    println!("This is a plugin example. Compile to WASM for actual use.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_initialization() {
        let mut plugin = HttpPlugin::default();
        assert!(plugin.on_init().is_ok());
    }

    #[test]
    fn test_item_validation() {
        let mut plugin = HttpPlugin::default();
        
        // Test valid item creation
        let event = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "items".to_string(),
            data: r#"{"name": "Test Item", "category": "test"}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_create(&event).unwrap();
        assert!(response.allow);
        
        // Verify metadata was added
        let enhanced_data: JsonValue = serde_json::from_str(
            response.modified_data.as_ref().unwrap()
        ).unwrap();
        assert_eq!(enhanced_data["processed_by"], "http-plugin");
    }

    #[test]
    fn test_empty_name_validation() {
        let mut plugin = HttpPlugin::default();
        
        let event = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "items".to_string(),
            data: r#"{"name": "", "category": "test"}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_create(&event).unwrap();
        assert!(!response.allow);
        assert!(response.error_message.unwrap().contains("Name field is required"));
    }

    #[test]
    fn test_protected_record_deletion() {
        let mut plugin = HttpPlugin::default();
        
        let event = EventPayload {
            event_type: "BeforeRecordDelete".to_string(),
            collection: "items".to_string(),
            data: r#"{"id": "123", "name": "Protected Item", "protected": true}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_delete(&event).unwrap();
        assert!(!response.allow);
        assert!(response.error_message.unwrap().contains("Cannot delete protected"));
    }

    #[test]
    fn test_system_collection_protection() {
        let mut plugin = HttpPlugin::default();
        
        let event = EventPayload {
            event_type: "BeforeRecordDelete".to_string(),
            collection: "_system_config".to_string(),
            data: r#"{"id": "123", "name": "System Config"}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_delete(&event).unwrap();
        assert!(!response.allow);
        assert!(response.error_message.unwrap().contains("System collections"));
    }
} 