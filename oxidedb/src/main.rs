//! OxideDB - A hook-first database with plugin architecture
//!
//! This is the main entry point for OxideDB. It demonstrates the integration
//! of the core architecture components implemented in Milestone 3.

use oxide_api::server::ApiServer;
use oxide_core::{AppError, AuthService, BeforeEventContext, BeforeEventType, EventBus, InMemoryEventBus, register_system_hooks};
use oxide_core::plugin_api::{EventPayload, plugin_exports, PluginRuntime};
use oxide_db::{Db, SqliteDb};
use oxidedb::WasmtimePluginRuntime;
use std::sync::{Arc, Mutex};
use std::future::Future;
use std::pin::Pin;
use tracing::{info, warn, error, Level};
use tracing_subscriber::fmt;

// Type alias to reduce complexity
type BeforeCreateHandler = Arc<dyn Fn(&mut BeforeEventContext) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>> + Send + Sync>;

/// Bridge to connect WASM plugins with the EventBus system
struct PluginEventBridge {
    plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>,
}

impl PluginEventBridge {
    fn new(plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>) -> Self {
        Self { plugin_runtime }
    }

    /// Create an event handler that calls the plugin for BeforeRecordCreate events
    fn create_before_create_handler(&self, plugin_name: String) -> BeforeCreateHandler {
        let runtime = Arc::clone(&self.plugin_runtime);
        
        Arc::new(move |context: &mut BeforeEventContext| {
            let runtime = Arc::clone(&runtime);
            let plugin_name = plugin_name.clone();
            Box::pin(async move {
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
                    Err(AppError::plugin(plugin_name, format!("Plugin execution failed: {}", e)))
                }
            }
            })
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize logging
    fmt().with_max_level(Level::INFO).init();

    info!("Starting OxideDB - Milestone 4 with Plugin Integration Test");

    // Create the event bus - the heart of our hook-first architecture
    let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
    info!("✅ Event bus initialized");

    // Initialize plugin runtime with security policies that allow untrusted plugins
    let security_policies = oxide_core::plugin_security::SecurityPolicies {
        default_trust_level: oxide_core::plugin_security::PluginTrustLevel::Untrusted,
        default_resource_limits: oxide_core::plugin_security::ResourceLimits::default(),
        max_violations_before_suspension: 5,
        allow_untrusted_plugins: true,  // Allow untrusted plugins
        require_code_signing: false,
    };
    
    let mut plugin_runtime = WasmtimePluginRuntime::new_with_security_policies(security_policies)
        .map_err(|e| AppError::internal(format!("Failed to create plugin runtime: {}", e)))?;
    
    // Load the hello-plugin with proper security registration
    let wasm_path = "./target/wasm32-unknown-unknown/release/hello_plugin.wasm";
    let wasm_bytes = std::fs::read(wasm_path)
        .map_err(|e| AppError::internal(format!("Failed to read WASM file {}: {}", wasm_path, e)))?;
    
    // Load plugin with elevated trust level to allow advanced capabilities
    use oxide_core::plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits};
    use oxide_core::auth::CrudOperation;
    
    // Load plugin with PartiallyTrusted level to enable more capabilities
    plugin_runtime.load_plugin_with_trust(
        "hello-plugin", 
        &wasm_bytes,
        PluginTrustLevel::PartiallyTrusted,
        vec![
            PluginCapability::ReadEventData,
            PluginCapability::ModifyEventData,
            PluginCapability::BlockOperations,
            PluginCapability::AccessCollection {
                collection: "*".to_string(),
                operations: vec![CrudOperation::Create, CrudOperation::Read, CrudOperation::Update, CrudOperation::Delete],
            },
            PluginCapability::LogInfo,
            PluginCapability::LogError,
        ],
        ResourceLimits::default(),
    ).map_err(|e| AppError::internal(format!("Failed to load hello-plugin with capabilities: {}", e)))?;
    
    info!("✅ Hello plugin loaded successfully with security context and capabilities");
    
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
    ).map_err(|e| AppError::internal(format!("Failed to initialize plugin: {}", e)))?;
    
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
        Arc::new(|context: &mut BeforeEventContext| {
            Box::pin(async move {
                info!(
                    "🎣 Demo Hook triggered: About to create record in collection '{}' with data: {}",
                    context.collection, context.data
                );
                Ok(())
            })
        }),
    )?;
    info!("✅ Demo event listener registered");

    // Initialize AuthService with configuration
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev_secret_key_change_in_production".to_string());
    let auth_config = oxide_core::auth::AuthServiceConfig::new(jwt_secret);
    let mut auth_service = AuthService::new(auth_config);
    info!("✅ Authentication service initialized");

    // Initialize the database with event integration
    let database_path = ":memory:"; // Use in-memory SQLite for demo
    let database = Arc::new(SqliteDb::new(
        database_path,
        Arc::clone(&event_bus),
        Arc::new(auth_service.clone()), // Clone for database initialization
    )?);
    database.initialize().await?;
    info!("✅ SQLite database initialized with event integration and authentication");

    // Update auth service configuration with discovered auth collections
    let auth_collections = database.list_auth_collections().await?;
    auth_service.update_auth_collections(&auth_collections);
    let auth_service = Arc::new(auth_service);
    info!("✅ Auth service updated with {} auth collections", auth_collections.len());

    // Create permission service for authorization hooks
    let permission_service = Arc::new(oxide_api::services::DatabasePermissionService::new(
        Arc::clone(&database) as Arc<dyn oxide_db::Db>
    ));
    info!("✅ Permission service initialized");

    // Register all system hooks using the new centralized system
    register_system_hooks(
        event_bus.as_ref(), 
        Arc::clone(&auth_service),
        Arc::clone(&permission_service) as Arc<dyn oxide_core::auth::PermissionService>
    ).await?;
    info!("✅ All system hooks registered via centralized registry");

    // Create and start the API server
    info!("🚀 Starting API server...");
    let api_server = ApiServer::new(
        Arc::clone(&database) as Arc<dyn Db>,
        Arc::clone(&event_bus),
        Arc::clone(&auth_service),
        "127.0.0.1".to_string(),
        8080,
    );

    // Demonstrate the authentication system by creating a sample user
    info!("🚀 Demonstrating authentication system...");

    // Get auth service configuration to determine available collections
    let auth_collections = database.list_auth_collections().await?;
    
    if auth_collections.is_empty() {
        warn!("No auth collections found. Auth system may not be properly configured.");
    } else {
        info!("Found {} auth collections: {:?}", 
            auth_collections.len(), 
            auth_collections.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        // Use the new auth collection system to register users
        // Try to register a superuser in the "superusers" collection if it exists
        let superuser_collection = auth_collections.iter()
            .find(|c| c.name == "superusers")
            .map(|c| c.name.as_str())
            .unwrap_or("users"); // Fallback to users if superusers doesn't exist

        let user_collection = auth_collections.iter()
            .find(|c| c.name == "users")
            .map(|c| c.name.as_str())
            .unwrap_or(superuser_collection); // Use the first available collection

        // Get auth configurations for the collections
        if let Some(superuser_config) = auth_service.config().get_auth_collection(superuser_collection) {
            if superuser_config.registration_enabled {
                let register_request = oxide_db::db::RegisterRequest {
                    collection: superuser_collection.to_string(),
                    identifier: "admin@example.com".to_string(),
                    credential: "secure_password_123".to_string(),
                    additional_data: Some(serde_json::json!({
                        "verified": true,
                        "name": "System Administrator"
                    })),
                };

                match database.register_user(register_request, superuser_config).await {
                    Ok(superuser_id) => {
                        info!("✅ Sample superuser registered with ID: {} in collection '{}'", superuser_id, superuser_collection);
                    }
                    Err(e) => {
                        warn!("Failed to register superuser: {} (this may be expected if user already exists)", e);
                    }
                }
            } else {
                info!("Registration is disabled for collection '{}'", superuser_collection);
            }
        }

        if let Some(user_config) = auth_service.config().get_auth_collection(user_collection) {
            if user_config.registration_enabled && user_collection != superuser_collection {
                let register_request = oxide_db::db::RegisterRequest {
                    collection: user_collection.to_string(),
                    identifier: "user@example.com".to_string(),
                    credential: "user_password_456".to_string(),
                    additional_data: Some(serde_json::json!({
                        "verified": false,
                        "name": "Regular User"
                    })),
                };

                match database.register_user(register_request, user_config).await {
                    Ok(user_id) => {
                        info!("✅ Sample user registered with ID: {} in collection '{}'", user_id, user_collection);
                    }
                    Err(e) => {
                        warn!("Failed to register user: {} (this may be expected if user already exists)", e);
                    }
                }
            }
        }

        // Demonstrate authentication using the new system
        if let Some(superuser_config) = auth_service.config().get_auth_collection(superuser_collection) {
            let auth_request = oxide_db::db::AuthRequest {
                collection: superuser_collection.to_string(),
                identifier: "admin@example.com".to_string(),
                credential: "secure_password_123".to_string(),
            };

            match database.authenticate_user(auth_request, superuser_config).await {
                Ok(auth_response) => {
                    info!(
                        "✅ User authenticated. ID: {}, Collection: {}, Token starts with: {}...",
                        auth_response.user_id,
                        auth_response.auth_collection,
                        &auth_response.token[..20]
                    );
                }
                Err(e) => {
                    warn!("Authentication failed: {}", e);
                }
            }
        }
    }

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
    info!("🎨 Admin UI is available at: http://127.0.0.1:8080/admin");
    info!("📋 Available endpoints:");
    info!("  - GET  /health                              - Health check");
    info!("  - GET  /admin                               - Admin UI");
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
