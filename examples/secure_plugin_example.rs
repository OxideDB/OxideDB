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
use oxidedb::plugin_runtime::WasmtimePluginRuntime;
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
    let payload = EventPayload {
        collection: "users".to_string(),
        operation: "create".to_string(),
        data: serde_json::json!({
            "name": "John Doe",
            "email": "john@example.com"
        }),
        metadata: std::collections::HashMap::new(),
    };

    // This should succeed for trusted plugin
    match runtime.call_plugin_function("trusted_processor", "on_before_create", &payload) {
        Ok(response) => println!("   ✅ trusted_processor executed successfully: {:?}", response),
        Err(e) => println!("   ❌ trusted_processor failed: {}", e),
    }

    // This should fail for untrusted plugin (lacks collection access)
    match runtime.call_plugin_function("untrusted_logger", "on_before_create", &payload) {
        Ok(response) => println!("   ⚠️  untrusted_logger unexpectedly succeeded: {:?}", response),
        Err(e) => println!("   ✅ untrusted_logger correctly blocked: {}", e),
    }
    println!();

    // Example 5: Grant additional capabilities at runtime
    println!("🔑 Example 5: Granting additional capabilities at runtime");
    runtime.grant_plugin_capability(
        "untrusted_logger",
        PluginCapability::AccessCollection("logs".to_string()),
    )?;
    println!("   ✅ Granted 'logs' collection access to untrusted_logger");
    
    let now_has_logs_access = runtime.plugin_has_capability(
        "untrusted_logger",
        &PluginCapability::AccessCollection("logs".to_string()),
    );
    println!("   untrusted_logger now has logs access: {}", now_has_logs_access);
    println!();

    // Example 6: Revoke capabilities
    println!("🚫 Example 6: Revoking capabilities");
    runtime.revoke_plugin_capability(
        "untrusted_logger",
        &PluginCapability::LogError,
    )?;
    println!("   ✅ Revoked LogError capability from untrusted_logger");
    
    let still_has_error_logging = runtime.plugin_has_capability(
        "untrusted_logger",
        &PluginCapability::LogError,
    );
    println!("   untrusted_logger still has LogError: {}", still_has_error_logging);
    println!();

    // Example 7: Demonstrate plugin suspension
    println!("⏸️  Example 7: Suspending a plugin");
    runtime.suspend_plugin(
        "untrusted_logger",
        "Too many security violations detected".to_string(),
    )?;
    println!("   ✅ Suspended untrusted_logger");
    
    // Try to execute suspended plugin
    match runtime.call_plugin_function("untrusted_logger", "on_before_create", &payload) {
        Ok(_) => println!("   ⚠️  Suspended plugin unexpectedly executed"),
        Err(e) => println!("   ✅ Suspended plugin correctly blocked: {}", e),
    }
    println!();

    // Example 8: Resume plugin
    println!("▶️  Example 8: Resuming a plugin");
    runtime.resume_plugin("untrusted_logger")?;
    println!("   ✅ Resumed untrusted_logger");
    println!();

    // Example 9: Check execution statistics
    println!("📊 Example 9: Checking execution statistics");
    if let Some(stats) = runtime.get_plugin_stats("trusted_processor") {
        println!("   trusted_processor stats:");
        println!("     - Total calls: {}", stats.total_calls);
        println!("     - Failed calls: {}", stats.failed_calls);
        println!("     - Total execution time: {:?}", stats.total_execution_time);
        println!("     - Memory usage: {} bytes", stats.memory_usage_bytes);
    }
    println!();

    // Example 10: View audit log
    println!("📋 Example 10: Viewing security audit log");
    let audit_log = runtime.get_plugin_audit_log("untrusted_logger");
    println!("   untrusted_logger audit log ({} entries):", audit_log.len());
    for (i, entry) in audit_log.iter().take(5).enumerate() {
        println!("     {}. [{:?}] {:?} at {:?}", 
                i + 1, entry.event_type, entry.details, entry.timestamp);
    }
    if audit_log.len() > 5 {
        println!("     ... and {} more entries", audit_log.len() - 5);
    }
    println!();

    // Example 11: Clean up
    println!("🧹 Example 11: Cleaning up plugins");
    runtime.unload_plugin("untrusted_logger")?;
    runtime.unload_plugin("trusted_processor")?;
    println!("   ✅ All plugins unloaded and security contexts cleared\n");

    println!("🎉 Secure plugin runtime example completed successfully!");
    println!("\n💡 Key Security Features Demonstrated:");
    println!("   • Capability-based access control");
    println!("   • Trust level management");
    println!("   • Resource limits enforcement");
    println!("   • Runtime capability management");
    println!("   • Security violation handling");
    println!("   • Plugin suspension/resumption");
    println!("   • Comprehensive audit logging");
    println!("   • Execution statistics monitoring");

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