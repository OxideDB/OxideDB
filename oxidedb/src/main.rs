//! OxideDB - A hook-first database with plugin architecture
//!
//! This is the main entry point for OxideDB. It demonstrates the integration
//! of the core architecture components implemented in Milestone 3.

use oxide_api::ApiServer;
use oxide_core::{AppError, AuthService, BeforeEventContext, BeforeEventType, EventBus, InMemoryEventBus, register_system_hooks};
use oxide_core::plugin_api::{EventPayload, plugin_exports, PluginRuntime};
use oxide_db::{Db, SqliteDb};
use oxidedb::WasmtimePluginRuntime;
use std::sync::{Arc, Mutex};
use tracing::{info, warn, error, Level};
use tracing_subscriber;

/// Bridge to connect WASM plugins with the EventBus system
struct PluginEventBridge {
    plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>,
}

impl PluginEventBridge {
    fn new(plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>) -> Self {
        Self { plugin_runtime }
    }

    /// Create an event handler that calls the plugin for BeforeRecordCreate events
    fn create_before_create_handler(&self, plugin_name: String) -> Box<dyn Fn(&mut BeforeEventContext) -> Result<(), AppError> + Send + Sync> {
        let runtime = Arc::clone(&self.plugin_runtime);
        
        Box::new(move |context: &mut BeforeEventContext| {
            info!("🔌 Calling plugin '{}' for BeforeRecordCreate event", plugin_name);
            
            // Convert BeforeEventContext to EventPayload
            let payload = EventPayload {
                event_type: "BeforeRecordCreate".to_string(),
                collection: context.collection.clone(),
                data: context.data.to_string(),
                metadata: context.metadata.clone(),
            };

            // Call the plugin
            let mut runtime_guard = runtime.lock().map_err(|_| {
                AppError::internal("Failed to acquire plugin runtime lock")
            })?;

            match runtime_guard.call_plugin_function(&plugin_name, plugin_exports::ON_BEFORE_CREATE, &payload) {
                Ok(response) => {
                    info!("🔌 Plugin '{}' response: allow={}, error={:?}", 
                          plugin_name, response.allow, response.error_message);
                    
                                         // If plugin modified the data, update the context
                     if let Some(ref modified_data) = response.modified_data {
                         match serde_json::from_str(modified_data) {
                             Ok(new_data) => {
                                 context.data = new_data;
                                 info!("🔌 Plugin modified data: {}", modified_data);
                             }
                             Err(e) => {
                                 warn!("🔌 Plugin returned invalid modified data: {}", e);
                             }
                         }
                     }
                    
                    // If plugin doesn't allow the operation, return an error
                    if !response.allow {
                        let error_msg = response.error_message.unwrap_or_else(|| 
                            "Plugin rejected the operation".to_string()
                        );
                        return Err(AppError::plugin(&plugin_name, &error_msg));
                    }
                    
                    Ok(())
                }
                Err(e) => {
                    error!("🔌 Plugin '{}' execution failed: {}", plugin_name, e);
                    Err(AppError::plugin(&plugin_name, &format!("Plugin execution failed: {}", e)))
                }
            }
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting OxideDB - Milestone 4 with Plugin Integration Test");

    // Create the event bus - the heart of our hook-first architecture
    let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
    info!("✅ Event bus initialized");

    // Initialize plugin runtime
    let mut plugin_runtime = WasmtimePluginRuntime::new()
        .map_err(|e| AppError::internal(&format!("Failed to create plugin runtime: {}", e)))?;
    
    // Load the hello-plugin
    let wasm_path = "./target/wasm32-unknown-unknown/release/hello_plugin.wasm";
    let wasm_bytes = std::fs::read(wasm_path)
        .map_err(|e| AppError::internal(&format!("Failed to read WASM file {}: {}", wasm_path, e)))?;
    
    plugin_runtime.load_plugin("hello-plugin", &wasm_bytes)
        .map_err(|e| AppError::internal(&format!("Failed to load hello-plugin: {}", e)))?;
    
    info!("✅ Hello plugin loaded successfully");
    
    // Test plugin initialization
    let test_payload = EventPayload {
        event_type: "plugin_init".to_string(),
        collection: "test".to_string(),
        data: "{}".to_string(),
        metadata: serde_json::json!({}),
    };
    
    let _init_response = plugin_runtime.call_plugin_function(
        "hello-plugin",
        plugin_exports::PLUGIN_INIT,
        &test_payload,
    ).map_err(|e| AppError::internal(&format!("Failed to initialize plugin: {}", e)))?;
    
    info!("✅ Plugin initialized successfully");
    
    // Wrap the plugin runtime in Arc<Mutex<>> for sharing
    let shared_plugin_runtime = Arc::new(Mutex::new(plugin_runtime));
    
    // Create plugin bridge and register plugin with event system
    let plugin_bridge = PluginEventBridge::new(Arc::clone(&shared_plugin_runtime));
    let plugin_handler = plugin_bridge.create_before_create_handler("hello-plugin".to_string());
    
    event_bus.subscribe_before(
        BeforeEventType::RecordCreate.name(),
        plugin_handler,
    )?;
    info!("✅ Hello plugin registered with event system");

    // Register a sample event listener to demonstrate hooking
    event_bus.subscribe_before(
        BeforeEventType::RecordCreate.name(),
        Box::new(|context: &mut BeforeEventContext| {
            info!(
                "🎣 Demo Hook triggered: About to create record in collection '{}' with data: {}",
                context.collection, context.data
            );
            Ok(())
        }),
    )?;
    info!("✅ Demo event listener registered");

    // Initialize AuthService
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev_secret_key_change_in_production".to_string());
    let auth_service = Arc::new(AuthService::new(jwt_secret));
    info!("✅ Authentication service initialized");

    // Register all system hooks using the new centralized system
    register_system_hooks(event_bus.as_ref(), Arc::clone(&auth_service))?;
    info!("✅ All system hooks registered via centralized registry");

    // Initialize the database with event integration
    let database_path = ":memory:"; // Use in-memory SQLite for demo
    let database = Arc::new(SqliteDb::new(
        database_path,
        Arc::clone(&event_bus),
        Arc::clone(&auth_service),
    )?);
    database.initialize().await?;
    info!("✅ SQLite database initialized with event integration and authentication");

    // Create and start the API server
    info!("🚀 Starting API server...");
    let api_server = ApiServer::new(
        Arc::clone(&database) as Arc<dyn Db>,
        Arc::clone(&event_bus),
        "127.0.0.1".to_string(),
        8080,
    );

    // Demonstrate the authentication system by creating a sample user
    info!("🚀 Demonstrating authentication system...");

    // Register a sample superuser
    let superuser_id = database
        .register_user("admin@example.com", "secure_password_123", true)
        .await?;
    info!("✅ Sample superuser registered with ID: {}", superuser_id);

    // Register a regular user
    let user_id = database
        .register_user("user@example.com", "user_password_456", false)
        .await?;
    info!("✅ Sample user registered with ID: {}", user_id);

    // Demonstrate authentication
    let (auth_user_id, token) = database
        .authenticate_user("admin@example.com", "secure_password_123")
        .await?;
    info!(
        "✅ User authenticated. ID: {}, Token starts with: {}...",
        auth_user_id,
        &token[..20]
    );

    // Test the plugin integration by creating a record
    info!("🧪 Testing plugin integration with record creation...");
    
    // This should trigger the plugin and fail because the plugin sets an error
    match database.create_record("test_collection", serde_json::json!({"name": "test", "value": 42})).await {
        Ok(_) => {
            warn!("Expected plugin to reject the operation, but it succeeded!");
        }
        Err(e) => {
            info!("✅ Plugin correctly rejected the operation: {}", e);
        }
    }

    info!("🎉 Milestone 4 startup completed successfully!");
    info!("Architecture summary:");
    info!("  ✅ Cargo workspace with 3 crates (oxide-core, oxide-db, oxide-api)");
    info!("  ✅ Hook-first architecture with EventBus");
    info!("  ✅ Database abstraction with SQLite implementation");
    info!("  ✅ Standardized error handling with AppError");
    info!("  ✅ Full async support with Tokio");
    info!("  ✅ All operations route through events for extensibility");
    info!("  ✅ HTTP API server with health check endpoint");
    info!("  ✅ REST API for collections and records");
    info!("  ✅ WASM Plugin system with hello-plugin integration");
    info!("");
    info!("🌐 API server is running at: http://127.0.0.1:8080");
    info!("📋 Available endpoints:");
    info!("  - GET  /health                              - Health check");
    info!("  - GET  /collections                         - List collections");
    info!("  - POST /collections                         - Create collection");
    info!("  - DEL  /collections/{{collection}}            - Delete collection");
    info!("  - GET  /collections/{{collection}}/stats      - Collection stats");
    info!("  - GET  /collections/{{collection}}/records    - List records");
    info!("  - POST /collections/{{collection}}/records    - Create record");
    info!("  - GET  /collections/{{collection}}/records/{{id}} - Get record");
    info!("  - PUT  /collections/{{collection}}/records/{{id}} - Update record");
    info!("  - DEL  /collections/{{collection}}/records/{{id}} - Delete record");
    info!("");

    // Start the server (this will run until the process is terminated)
    api_server.start().await?;

    Ok(())
}
