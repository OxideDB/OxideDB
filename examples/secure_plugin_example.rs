//! Example demonstrating capability-based security for OxideDB plugins
//!
//! This example shows how to:
//! 1. Load plugins with different trust levels
//! 2. Grant and revoke capabilities
//! 3. Handle security violations
//! 4. Monitor plugin execution

use oxide_core::{
    plugin_api::{EventPayload, PluginRuntime},
    plugin_security::{
        PluginCapability, PluginTrustLevel, ResourceLimits, SecurityPolicies,
        SecurityViolation,
    },
};
use oxide_plugin_runtime::WasmtimePluginRuntime;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    
    println!("🔒 OxideDB Secure Plugin Runtime Example");
    println!("========================================\n");

    // Create security policies for production environment
    let security_policies = SecurityPolicies {
        default_trust_level: PluginTrustLevel::Untrusted,
        max_violations_before_suspension: 3,
        violation_window: Duration::from_secs(300), // 5 minutes
        require_explicit_capabilities: true,
        audit_all_operations: true,
    };

    // Initialize runtime with security policies
    let mut runtime = WasmtimePluginRuntime::new_with_security_policies(security_policies)?;
    println!("✅ Initialized secure plugin runtime\n");

    // Example 1: Load an untrusted plugin with minimal capabilities
    println!("📦 Example 1: Loading untrusted plugin with minimal capabilities");
    let untrusted_plugin_wasm = include_bytes!("../plugins/demo_plugin.wasm");
    
    runtime.load_plugin_with_trust(
        "untrusted_logger",
        untrusted_plugin_wasm,
        PluginTrustLevel::Untrusted,
        vec![
            PluginCapability::LogInfo,
            PluginCapability::LogError,
        ],
        ResourceLimits {
            max_memory_mb: 1,
            max_execution_time_ms: 100,
            max_host_function_calls: 10,
        },
    )?;
    println!("   ✅ Loaded 'untrusted_logger' with logging capabilities only\n");

    // Example 2: Load a trusted plugin with broader capabilities
    println!("📦 Example 2: Loading trusted plugin with broader capabilities");
    let trusted_plugin_wasm = include_bytes!("../plugins/demo_plugin.wasm");
    
    runtime.load_plugin_with_trust(
        "trusted_processor",
        trusted_plugin_wasm,
        PluginTrustLevel::Trusted,
        vec![
            PluginCapability::LogInfo,
            PluginCapability::LogError,
            PluginCapability::AccessCollection("users".to_string()),
            PluginCapability::AccessCollection("posts".to_string()),
            PluginCapability::HttpRequest("https://api.example.com".to_string()),
        ],
        ResourceLimits {
            max_memory_mb: 10,
            max_execution_time_ms: 1000,
            max_host_function_calls: 100,
        },
    )?;
    println!("   ✅ Loaded 'trusted_processor' with collection and HTTP access\n");

    // Example 3: Demonstrate capability checking
    println!("🔍 Example 3: Checking plugin capabilities");
    let has_logging = runtime.plugin_has_capability(
        "untrusted_logger",
        &PluginCapability::LogInfo,
    );
    println!("   untrusted_logger has LogInfo capability: {}", has_logging);
    
    let has_collection_access = runtime.plugin_has_capability(
        "untrusted_logger",
        &PluginCapability::AccessCollection("users".to_string()),
    );
    println!("   untrusted_logger has collection access: {}", has_collection_access);
    println!();

    // Example 4: Execute plugin functions with security validation
    println!("⚡ Example 4: Executing plugin functions with security validation");
    
    // Test basic event processing
    let test_payload = oxide_core::plugin_api::EventPayload {
        event_type: "BeforeRecordCreate".to_string(),
        collection: "posts".to_string(),
        data: r#"{"title": "Test Post", "content": "Hello World"}"#.to_string(),
        metadata: serde_json::json!({"user_id": "test_user"}),
    };

    match runtime.call_plugin_function("trusted_processor", "on_before_create", &test_payload) {
        Ok(response) => {
            println!("   ✅ Plugin executed successfully:");
            println!("      Allow: {}", response.allow);
            if let Some(modified_data) = response.modified_data {
                println!("      Modified data: {}", modified_data);
            }
        }
        Err(e) => {
            println!("   ❌ Plugin execution failed: {}", e);
        }
    }
    println!();

    // Example 5: Demonstrate HTTP route registration and handling
    println!("🌐 Example 5: HTTP route registration and CRUD operations");
    
    // Load plugin with HTTP and CRUD capabilities
    println!("   Loading plugin with HTTP and CRUD capabilities...");
    let hello_plugin_wasm = include_bytes!("../hello-plugin/hello-plugin.wasm");
    
    runtime.load_plugin_with_trust(
        "hello-plugin",
        hello_plugin_wasm,
        PluginTrustLevel::FullyTrusted,
        vec![
            PluginCapability::LogInfo,
            PluginCapability::LogError,
            PluginCapability::ReadEventData,
            PluginCapability::RegisterHttpRoutes {
                path_patterns: vec!["/api/hello/*".to_string()],
                methods: vec!["GET".to_string(), "POST".to_string()],
            },
            PluginCapability::HandleHttpRequests,
            PluginCapability::CreateRecords {
                collections: vec!["items".to_string(), "processed_data".to_string()],
            },
            PluginCapability::ReadRecords {
                collections: vec!["items".to_string(), "processed_data".to_string()],
            },
        ],
        ResourceLimits {
            max_memory_mb: 16,
            max_execution_time_ms: 5000,
            max_host_function_calls: 500,
        },
    )?;
    println!("   ✅ Hello plugin loaded with HTTP and CRUD capabilities");

    // Initialize the plugin (this registers HTTP routes)
    match runtime.call_plugin_function("hello-plugin", "plugin_init", &test_payload) {
        Ok(_) => println!("   ✅ Plugin initialized and routes registered"),
        Err(e) => println!("   ❌ Plugin initialization failed: {}", e),
    }

    // Get registered routes
    let routes = runtime.get_registered_routes();
    println!("   📍 Registered HTTP routes:");
    for route in &routes {
        println!("      {} {} -> {}", route.method, route.path, route.handler_function);
    }
    println!();

    // Example 6: Simulate HTTP requests to plugin endpoints
    println!("🔧 Example 6: Simulating HTTP requests to plugin endpoints");
    
    // Test GET /api/hello/items
    let get_request = oxide_core::plugin_api::HttpRequestContext {
        method: "GET".to_string(),
        path: "/api/hello/items".to_string(),
        query_params: std::collections::HashMap::new(),
        headers: std::collections::HashMap::new(),
        body: None,
        path_params: std::collections::HashMap::new(),
        user: Some(serde_json::json!({"id": "test_user", "role": "admin"})),
    };

    match runtime.handle_http_request("hello-plugin", "handle_get_items", &get_request) {
        Ok(response) => {
            println!("   ✅ GET /api/hello/items:");
            println!("      Status: {}", response.status_code);
            println!("      Body: {}", response.body);
        }
        Err(e) => println!("   ❌ GET request failed: {}", e),
    }

    // Test POST /api/hello/items
    let create_request = oxide_core::plugin_api::HttpRequestContext {
        method: "POST".to_string(),
        path: "/api/hello/items".to_string(),
        query_params: std::collections::HashMap::new(),
        headers: {
            let mut headers = std::collections::HashMap::new();
            headers.insert("Content-Type".to_string(), "application/json".to_string());
            headers
        },
        body: Some(r#"{"name": "New Item", "description": "Created via HTTP API"}"#.to_string()),
        path_params: std::collections::HashMap::new(),
        user: Some(serde_json::json!({"id": "test_user", "role": "user"})),
    };

    match runtime.handle_http_request("hello-plugin", "handle_create_item", &create_request) {
        Ok(response) => {
            println!("   ✅ POST /api/hello/items:");
            println!("      Status: {}", response.status_code);
            println!("      Body: {}", response.body);
        }
        Err(e) => println!("   ❌ POST request failed: {}", e),
    }

    // Test GET /api/hello/items/:id
    let get_item_request = oxide_core::plugin_api::HttpRequestContext {
        method: "GET".to_string(),
        path: "/api/hello/items/123".to_string(),
        query_params: std::collections::HashMap::new(),
        headers: std::collections::HashMap::new(),
        body: None,
        path_params: {
            let mut params = std::collections::HashMap::new();
            params.insert("id".to_string(), "123".to_string());
            params
        },
        user: Some(serde_json::json!({"id": "test_user", "role": "user"})),
    };

    match runtime.handle_http_request("hello-plugin", "handle_get_item", &get_item_request) {
        Ok(response) => {
            println!("   ✅ GET /api/hello/items/123:");
            println!("      Status: {}", response.status_code);
            println!("      Body: {}", response.body);
        }
        Err(e) => println!("   ❌ GET item request failed: {}", e),
    }

    // Test POST /api/hello/process (custom business logic)
    let process_request = oxide_core::plugin_api::HttpRequestContext {
        method: "POST".to_string(),
        path: "/api/hello/process".to_string(),
        query_params: {
            let mut params = std::collections::HashMap::new();
            params.insert("save".to_string(), "true".to_string());
            params
        },
        headers: {
            let mut headers = std::collections::HashMap::new();
            headers.insert("Content-Type".to_string(), "application/json".to_string());
            headers
        },
        body: Some(r#"{"message": "hello world", "count": 42, "active": true}"#.to_string()),
        path_params: std::collections::HashMap::new(),
        user: Some(serde_json::json!({"id": "test_user", "role": "user"})),
    };

    match runtime.handle_http_request("hello-plugin", "handle_process_data", &process_request) {
        Ok(response) => {
            println!("   ✅ POST /api/hello/process:");
            println!("      Status: {}", response.status_code);
            println!("      Body: {}", response.body);
        }
        Err(e) => println!("   ❌ Process request failed: {}", e),
    }
    println!();

    // Example 7: Security violation handling with HTTP capabilities
    println!("🚨 Example 7: Security violation handling with HTTP capabilities");
    
    // Try to grant dangerous capability to untrusted plugin
    match runtime.grant_plugin_capability(
        "untrusted_logger",
        PluginCapability::RegisterHttpRoutes {
            path_patterns: vec!["/*".to_string()],
            methods: vec!["*".to_string()],
        },
    ) {
        Ok(_) => println!("   ❌ Unexpected: Dangerous capability granted to untrusted plugin"),
        Err(e) => println!("   ✅ Correctly blocked dangerous capability: {}", e),
    }

    // Example 8: Clean up and monitoring
    println!("🧹 Example 8: Plugin monitoring and cleanup");
    
    // Get plugin statistics
    let plugins = runtime.list_plugins();
    println!("   📊 Loaded plugins: {:?}", plugins);
    
    for plugin_name in &plugins {
        if let Some(stats) = runtime.get_plugin_stats(plugin_name) {
            println!("   📈 Stats for {}: {:?}", plugin_name, stats);
        }
        
        let audit_log = runtime.get_plugin_audit_log(plugin_name);
        if !audit_log.is_empty() {
            println!("   📋 Audit log for {} ({} entries)", plugin_name, audit_log.len());
            for entry in audit_log.iter().take(3) {
                println!("      {:?}: {:?}", entry.event_type, entry.details);
            }
        }
    }

    // Cleanup
    runtime.clear_state();
    println!("   ✅ Runtime state cleared");
    println!();

    println!("🎉 All examples completed successfully!");
    println!("🔒 Security features demonstrated:");
    println!("   • Capability-based access control");
    println!("   • Trust level enforcement");
    println!("   • HTTP route registration and handling");
    println!("   • CRUD operation security");
    println!("   • Custom business logic execution");
    println!("   • Security violation detection");
    println!("   • Audit logging and monitoring");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_policies_creation() {
        let policies = SecurityPolicies {
            default_trust_level: PluginTrustLevel::Sandboxed,
            max_violations_before_suspension: 5,
            violation_window: Duration::from_secs(600),
            require_explicit_capabilities: true,
            audit_all_operations: true,
        };
        
        assert_eq!(policies.default_trust_level, PluginTrustLevel::Sandboxed);
        assert_eq!(policies.max_violations_before_suspension, 5);
        assert!(policies.require_explicit_capabilities);
        assert!(policies.audit_all_operations);
    }

    #[test]
    fn test_resource_limits() {
        let limits = ResourceLimits {
            max_memory_mb: 5,
            max_execution_time_ms: 500,
            max_host_function_calls: 50,
        };
        
        assert_eq!(limits.max_memory_mb, 5);
        assert_eq!(limits.max_execution_time_ms, 500);
        assert_eq!(limits.max_host_function_calls, 50);
    }

    #[test]
    fn test_capability_types() {
        let log_capability = PluginCapability::LogInfo;
        let collection_capability = PluginCapability::AccessCollection("test".to_string());
        let http_capability = PluginCapability::HttpRequest("https://api.test.com".to_string());
        
        // Test that capabilities can be created and compared
        assert_ne!(log_capability, collection_capability);
        assert_ne!(collection_capability, http_capability);
    }
}