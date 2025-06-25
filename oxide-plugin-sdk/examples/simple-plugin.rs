//! Simple Plugin Example using OxideDB Plugin SDK
//! 
//! This example demonstrates how to create a simple plugin using the high-level SDK.
//! Compare this to the raw WASM plugin implementation to see the difference in complexity.

use oxide_plugin_sdk::prelude::*;

/// A simple plugin that validates and enhances data
#[derive(Default)]
pub struct SimplePlugin;

impl PluginEventHandler for SimplePlugin {
    fn on_init(&mut self) -> PluginResult<()> {
        log_info!("Simple plugin initialized!");
        Ok(())
    }

    fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Validating creation in collection: {}", event.collection);

        // Parse the incoming data
        let mut data: JsonValue = serde_json::from_str(&event.data)?;

        // Security check: block admin collections
        if event.collection.contains("admin") || event.collection.contains("system") {
            log_warn!("Attempted access to restricted collection: {}", event.collection);
            return Ok(PluginResponse::deny("Access denied to restricted collection"));
        }

        // Validation: ensure required fields exist
        if let Some(obj) = data.as_object_mut() {
            // Add plugin metadata
            obj.insert("plugin_processed_at".to_string(), json!("2024-01-01T00:00:00Z"));
            obj.insert("plugin_name".to_string(), json!("simple-plugin"));
            obj.insert("plugin_version".to_string(), json!("1.0.0"));

            // Validate name field if present
            if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
                if name.trim().is_empty() {
                    return Ok(PluginResponse::deny("Name cannot be empty"));
                }
                
                // Capitalize the name
                obj.insert("name".to_string(), json!(capitalize_name(name)));
            }
        }

        log_info!("Validation passed, data enhanced");
        Ok(PluginResponse::allow_with_data(&data)?)
    }

    fn on_after_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Record created successfully in collection: {}", event.collection);

        // Log the creation for audit purposes
        let audit_entry = json!({
            "action": "record_created",
            "collection": event.collection,
            "timestamp": "2024-01-01T00:00:00Z",
            "plugin": "simple-plugin"
        });

        // Try to create an audit log entry (this might fail if collection doesn't exist)
        if let Err(e) = Database::create("_audit_log", &audit_entry) {
            log_warn!("Failed to create audit log: {}", e);
            // Don't fail the main operation because of audit logging
        }

        Ok(PluginResponse::allow())
    }

    fn on_before_update(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Validating update in collection: {}", event.collection);

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
        log_info!("Validating deletion in collection: {}", event.collection);

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
        log_info!("Simple plugin cleanup completed");
        Ok(())
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

// Export the plugin - this generates all the WASM exports automatically
oxide_plugin_sdk::export_plugin!(SimplePlugin);

/// This main function is just for compilation - plugins are normally compiled to WASM
fn main() {
    println!("This is a plugin example. Compile to WASM for actual use.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capitalize_name() {
        assert_eq!(capitalize_name("john doe"), "John Doe");
        assert_eq!(capitalize_name("JANE SMITH"), "Jane Smith");
        assert_eq!(capitalize_name("alice"), "Alice");
    }

    #[test]
    fn test_validation_logic() {
        let mut plugin = SimplePlugin::default();
        
        let event = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "users".to_string(),
            data: r#"{"name": "john doe", "email": "john@example.com"}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_create(&event).unwrap();
        assert!(response.allow);
        
        // Check that the response contains enhanced data
        let enhanced_data: JsonValue = serde_json::from_str(
            response.modified_data.as_ref().unwrap()
        ).unwrap();
        
        assert_eq!(enhanced_data["name"], "John Doe");
        assert_eq!(enhanced_data["plugin_name"], "simple-plugin");
    }

    #[test]
    fn test_empty_name_validation() {
        let mut plugin = SimplePlugin::default();
        
        let event = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "users".to_string(),
            data: r#"{"name": "", "email": "test@example.com"}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_create(&event).unwrap();
        assert!(!response.allow);
        assert!(response.error_message.unwrap().contains("Name cannot be empty"));
    }

    #[test]
    fn test_admin_collection_blocked() {
        let mut plugin = SimplePlugin::default();
        
        let event = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "admin_users".to_string(),
            data: r#"{"name": "Admin User"}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_create(&event).unwrap();
        assert!(!response.allow);
        assert!(response.error_message.unwrap().contains("Access denied"));
    }

    #[test]
    fn test_protected_record_deletion() {
        let mut plugin = SimplePlugin::default();
        
        let event = EventPayload {
            event_type: "BeforeRecordDelete".to_string(),
            collection: "users".to_string(),
            data: r#"{"id": "123", "name": "Protected User", "protected": true}"#.to_string(),
            metadata: json!({}),
        };

        let response = plugin.on_before_delete(&event).unwrap();
        assert!(!response.allow);
        assert!(response.error_message.unwrap().contains("Cannot delete protected"));
    }
} 