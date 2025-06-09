//! Simple Plugin Test - Basic verification of plugin loading
//!
//! This test verifies that the plugin runtime can be initialized and plugins can be loaded.

use oxide_core::PluginRuntime;
use oxidedb::WasmtimePluginRuntime;
use std::fs;

#[test]
fn test_plugin_runtime_initialization() {
    // Test that we can create a plugin runtime
    let runtime = WasmtimePluginRuntime::new();
    assert!(
        runtime.is_ok(),
        "Plugin runtime should initialize successfully"
    );

    let runtime = runtime.unwrap();

    // Test that initially no plugins are loaded
    assert_eq!(
        runtime.list_plugins().len(),
        0,
        "No plugins should be loaded initially"
    );

    println!("✅ Plugin runtime initialization test passed");
}

#[test]
fn test_plugin_loading() {
    let mut runtime = WasmtimePluginRuntime::new().expect("Failed to initialize runtime");

    // Try to load the hello plugin if it exists
    let wasm_path = "../target/wasm32-unknown-unknown/release/hello_plugin.wasm";

    if let Ok(wasm_bytes) = fs::read(wasm_path) {
        let result = runtime.load_plugin("hello-plugin", &wasm_bytes);
        assert!(result.is_ok(), "Plugin should load successfully");

        // Verify the plugin is in the list
        let plugins = runtime.list_plugins();
        assert!(
            plugins.contains(&"hello-plugin".to_string()),
            "Plugin should be in the list"
        );

        println!("✅ Plugin loading test passed");
    } else {
        println!("⚠️  Skipping plugin loading test - WASM file not found");
        println!(
            "   Run: cd hello-plugin && cargo build --target wasm32-unknown-unknown --release"
        );
    }
}
