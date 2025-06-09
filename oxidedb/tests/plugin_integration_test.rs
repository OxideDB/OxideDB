//! Integration tests for Wasm Plugin System PoC
//!
//! This test implements the requirements from Milestone 3, Task 3.3:
//! 1. Initialize the Wasm runtime
//! 2. Load the hello-plugin.wasm file
//! 3. Call the on_before_create function within the Wasm module
//! 4. Assert that the host's set_error function was correctly called by the plugin

use oxide_core::{plugin_exports, EventPayload, PluginRuntime};
use oxidedb::WasmtimePluginRuntime;
use std::fs;

/// Test the "Hello Wasm" PoC as specified in Milestone 3
#[test]
fn test_hello_wasm_plugin_poc() {
    // Step 1: Initialize the Wasm runtime
    let mut runtime = WasmtimePluginRuntime::new().expect("Failed to initialize Wasm runtime");

    // Step 2: Load the hello-plugin.wasm file
    let wasm_path = "../target/wasm32-unknown-unknown/release/hello_plugin.wasm";
    let wasm_bytes = fs::read(wasm_path)
        .expect("Failed to read hello_plugin.wasm - make sure it's compiled first");

    runtime
        .load_plugin("hello-plugin", &wasm_bytes)
        .expect("Failed to load hello plugin");

    // Verify the plugin is loaded
    let plugins = runtime.list_plugins();
    assert!(plugins.contains(&"hello-plugin".to_string()));

    // Verify the plugin exports the expected function
    assert!(runtime.has_function("hello-plugin", plugin_exports::ON_BEFORE_CREATE));

    // Step 3: Call the on_before_create function within the Wasm module
    let test_payload = EventPayload {
        event_type: "BeforeRecordCreate".to_string(),
        collection: "test_collection".to_string(),
        data: r#"{"name": "test", "value": 42}"#.to_string(),
        metadata: serde_json::json!({"test": true}),
    };

    let response = runtime
        .call_plugin_function(
            "hello-plugin",
            plugin_exports::ON_BEFORE_CREATE,
            &test_payload,
        )
        .expect("Failed to call plugin function");

    // Step 4: Assert that the host's set_error function was correctly called by the plugin

    // Verify the plugin response indicates an error was set
    assert!(
        !response.allow,
        "Plugin should have set allow to false due to test error"
    );
    assert!(
        response.error_message.is_some(),
        "Plugin should have set an error message"
    );

    if let Some(error_msg) = &response.error_message {
        assert!(
            error_msg.contains("test"),
            "Error message should contain 'test'"
        );
    }

    // Verify the plugin set metadata
    assert!(
        response.metadata.is_object(),
        "Plugin should have set metadata"
    );
    assert_eq!(response.metadata["plugin"], "hello-plugin");

    // Check that the host state captured the error message
    let plugin_error = runtime.get_plugin_error();
    assert!(
        plugin_error.is_some(),
        "Host should have captured the error message from plugin"
    );
    if let Some(host_error) = &plugin_error {
        assert!(
            host_error.contains("Hello Wasm plugin test error message"),
            "Host should have captured the exact error message: {}",
            host_error
        );
    }

    // Verify log messages were captured
    let log_messages = runtime.get_plugin_logs();
    assert!(
        !log_messages.is_empty(),
        "Plugin should have logged messages"
    );

    // Check for expected log messages
    let log_text = log_messages.join(" ");
    assert!(
        log_text.contains("Hello from WASM plugin"),
        "Should contain greeting message"
    );
    assert!(
        log_text.contains("Processing event"),
        "Should contain event processing message"
    );
    assert!(
        log_text.contains("BeforeRecordCreate"),
        "Should log the event type"
    );
    assert!(
        log_text.contains("test_collection"),
        "Should log the collection name"
    );

    println!("✅ Hello Wasm PoC test completed successfully!");
    println!("📋 Test Summary:");
    println!("  - Wasm runtime initialized ✓");
    println!("  - Plugin loaded from .wasm file ✓");
    println!("  - Plugin function called successfully ✓");
    println!("  - Plugin set error message correctly ✓");
    println!("  - Host captured plugin error ✓");
    println!("  - Plugin logging worked ✓");
    println!("  - Event payload processed ✓");
}

/// Test plugin initialization and cleanup functions
#[test]
fn test_plugin_lifecycle() {
    let mut runtime = WasmtimePluginRuntime::new().expect("Failed to initialize Wasm runtime");

    // Load the plugin
    let wasm_path = "../target/wasm32-unknown-unknown/release/hello_plugin.wasm";
    let wasm_bytes = fs::read(wasm_path).expect("Failed to read hello_plugin.wasm");

    runtime
        .load_plugin("hello-plugin", &wasm_bytes)
        .expect("Failed to load hello plugin");

    // Test plugin_init function
    let test_payload = EventPayload {
        event_type: "PluginInit".to_string(),
        collection: "".to_string(),
        data: "{}".to_string(),
        metadata: serde_json::json!({}),
    };

    let _response = runtime
        .call_plugin_function("hello-plugin", plugin_exports::PLUGIN_INIT, &test_payload)
        .expect("Failed to call plugin_init");

    // Test plugin_cleanup function
    let _response = runtime
        .call_plugin_function(
            "hello-plugin",
            plugin_exports::PLUGIN_CLEANUP,
            &test_payload,
        )
        .expect("Failed to call plugin_cleanup");

    // Verify log messages from init and cleanup
    let log_messages = runtime.get_plugin_logs();
    let log_text = log_messages.join(" ");
    assert!(
        log_text.contains("Hello plugin initialized"),
        "Should contain init message"
    );
    assert!(
        log_text.contains("Hello plugin cleaned up"),
        "Should contain cleanup message"
    );

    // Test unloading the plugin
    runtime
        .unload_plugin("hello-plugin")
        .expect("Failed to unload plugin");

    assert!(!runtime.list_plugins().contains(&"hello-plugin".to_string()));
}

/// Test error handling for non-existent plugins and functions
#[test]
fn test_plugin_error_handling() {
    let mut runtime = WasmtimePluginRuntime::new().expect("Failed to initialize Wasm runtime");

    let test_payload = EventPayload {
        event_type: "Test".to_string(),
        collection: "test".to_string(),
        data: "{}".to_string(),
        metadata: serde_json::json!({}),
    };

    // Test calling function on non-existent plugin
    let result =
        runtime.call_plugin_function("non-existent-plugin", "some_function", &test_payload);
    assert!(
        result.is_err(),
        "Should fail when calling non-existent plugin"
    );

    // Load the plugin for further tests
    let wasm_path = "../target/wasm32-unknown-unknown/release/hello_plugin.wasm";
    let wasm_bytes = fs::read(wasm_path).expect("Failed to read hello_plugin.wasm");

    runtime
        .load_plugin("hello-plugin", &wasm_bytes)
        .expect("Failed to load hello plugin");

    // Test calling non-existent function
    let result =
        runtime.call_plugin_function("hello-plugin", "non_existent_function", &test_payload);
    assert!(
        result.is_err(),
        "Should fail when calling non-existent function"
    );

    // Test has_function with non-existent function
    assert!(!runtime.has_function("hello-plugin", "non_existent_function"));
    assert!(runtime.has_function("hello-plugin", plugin_exports::ON_BEFORE_CREATE));
}
