//! OxideDB - A hook-first database with plugin architecture
//!
//! This is the main entry point for OxideDB. It demonstrates the integration
//! of the core architecture components implemented in Milestone 3.

use oxide_api::server::ApiServer;
use oxide_core::{AppError, AuthService, BeforeEventContext, BeforeEventType, EventBus, InMemoryEventBus, register_system_hooks, ApplicationLogger, SecurityAuditor};
use oxide_core::plugin_api::{EventPayload, plugin_exports, PluginRuntime};
use oxide_db::{Db, SqliteDb};
use oxide_logging::{LogService, LogServiceBuilder, LogServiceBridge};
use oxidedb::WasmtimePluginRuntime;
use std::sync::{Arc, Mutex};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tracing::{info, warn, error, Level};
use tracing_subscriber::fmt;
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Security policy levels for plugin execution
#[derive(Debug, Clone, ValueEnum)]
enum SecurityPolicy {
    /// Strict security - only fully trusted plugins allowed
    Strict,
    /// Allow untrusted plugins with limited capabilities
    Untrusted,
    /// Development mode - relaxed security for testing
    Dev,
}

impl From<SecurityPolicy> for oxide_core::plugin_security::SecurityPolicies {
    fn from(policy: SecurityPolicy) -> Self {
        match policy {
            SecurityPolicy::Strict => oxide_core::plugin_security::SecurityPolicies {
                default_trust_level: oxide_core::plugin_security::PluginTrustLevel::FullyTrusted,
                default_resource_limits: oxide_core::plugin_security::ResourceLimits::conservative(),
                max_violations_before_suspension: 3,
                allow_untrusted_plugins: false,
                require_code_signing: true,
            },
            SecurityPolicy::Untrusted => oxide_core::plugin_security::SecurityPolicies {
                default_trust_level: oxide_core::plugin_security::PluginTrustLevel::Untrusted,
                default_resource_limits: oxide_core::plugin_security::ResourceLimits::default(),
                max_violations_before_suspension: 5,
                allow_untrusted_plugins: true,
                require_code_signing: false,
            },
            SecurityPolicy::Dev => oxide_core::plugin_security::SecurityPolicies {
                default_trust_level: oxide_core::plugin_security::PluginTrustLevel::PartiallyTrusted,
                default_resource_limits: oxide_core::plugin_security::ResourceLimits::relaxed(),
                max_violations_before_suspension: 10,
                allow_untrusted_plugins: true,
                require_code_signing: false,
            },
        }
    }
}

/// Log level configuration for the application
#[derive(Debug, Clone, ValueEnum)]
enum LogLevel {
    /// Only error messages
    Error,
    /// Warning and error messages
    Warn,
    /// Info, warning, and error messages
    Info,
    /// Debug and above (verbose)
    Debug,
    /// All messages including trace (very verbose)
    Trace,
}

impl From<LogLevel> for Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => Level::ERROR,
            LogLevel::Warn => Level::WARN,
            LogLevel::Info => Level::INFO,
            LogLevel::Debug => Level::DEBUG,
            LogLevel::Trace => Level::TRACE,
        }
    }
}

/// OxideDB - A hook-first database with plugin architecture
#[derive(Parser)]
#[command(name = "oxidedb")]
#[command(about = "A hook-first database with plugin architecture")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the OxideDB server
    Start(StartArgs),
    /// Register a superuser account
    RegisterSuperuser(RegisterSuperuserArgs),
}

#[derive(Parser)]
struct StartArgs {
    /// Database file path
    #[arg(long, default_value = "oxidedb-sqlite")]
    db_path: PathBuf,

    /// Security policy for plugin execution
    #[arg(long, value_enum, default_value = "untrusted")]
    security_policy: SecurityPolicy,

    /// Plugin folder path
    #[arg(long, default_value = "oxidedb-plugins")]
    plugin_folder: PathBuf,

    /// API server port
    #[arg(long, default_value = "8080")]
    api_port: u16,

    /// Admin UI path prefix
    #[arg(long, default_value = "/admin")]
    admin_path: String,

    /// Enable admin UI (set to false to disable in production)
    #[arg(long, default_value = "true")]
    enable_admin: bool,

    /// Admin UI mode: embedded (use built-in UI) or external (serve from filesystem)
    #[arg(long, value_enum, default_value = "embedded")]
    admin_mode: AdminMode,

    /// Path to external admin UI files (only used when admin_mode=external)
    #[arg(long, default_value = "ui/dist")]
    admin_ui_path: PathBuf,

    /// Auto-populate database with sample data
    #[arg(long)]
    auto_populate_db: bool,

    /// Bind address for the API server
    #[arg(long, default_value = "127.0.0.1")]
    bind_address: String,

    /// Log level for the application
    #[arg(long, value_enum, default_value = "info")]
    log_level: LogLevel,

    /// Enable request logging
    #[arg(long, default_value = "true")]
    enable_request_logging: bool,

    /// Enable CORS (Cross-Origin Resource Sharing)
    #[arg(long, default_value = "true")]
    enable_cors: bool,

    /// Enable logging system
    #[arg(long, default_value = "true")]
    enable_logging: bool,

    /// Logging database path
    #[arg(long, default_value = "logs")]
    logging_db_path: PathBuf,
}

/// Admin UI modes
#[derive(Debug, Clone, ValueEnum)]
enum AdminMode {
    /// Use the embedded admin UI (default)
    Embedded,
    /// Serve admin UI from external filesystem path
    External,
    /// Disable admin UI completely
    Disabled,
}

#[derive(Parser)]
struct RegisterSuperuserArgs {
    /// Database file path
    #[arg(long, default_value = "oxidedb-sqlite")]
    db_path: PathBuf,

    /// Email address for the superuser
    #[arg(long)]
    email: String,

    /// Password for the superuser
    #[arg(long)]
    password: String,

    /// Full name for the superuser
    #[arg(long)]
    name: Option<String>,

    /// Log level for the operation
    #[arg(long, value_enum, default_value = "info")]
    log_level: LogLevel,
}

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
    let cli = Cli::parse();

    // Initialize logging based on command-specific log level
    let log_level = match &cli.command {
        Commands::Start(args) => args.log_level.clone(),
        Commands::RegisterSuperuser(args) => args.log_level.clone(),
    };
    
    fmt().with_max_level(Level::from(log_level)).init();

    match cli.command {
        Commands::Start(args) => start_server(args).await,
        Commands::RegisterSuperuser(args) => register_superuser_command(args).await,
    }
}

/// Start the OxideDB server with the provided configuration
async fn start_server(args: StartArgs) -> Result<(), AppError> {
    info!("Starting OxideDB - Milestone 4 with Plugin Integration Test");
    info!("Configuration:");
    info!("  Database path: {:?}", args.db_path);
    info!("  Security policy: {:?}", args.security_policy);
    info!("  Plugin folder: {:?}", args.plugin_folder);
    info!("  API port: {}", args.api_port);
    info!("  Admin path: {}", args.admin_path);
    info!("  Admin enabled: {}", args.enable_admin);
    info!("  Admin mode: {:?}", args.admin_mode);
    if matches!(args.admin_mode, AdminMode::External) {
        info!("  Admin UI path: {:?}", args.admin_ui_path);
    }
    info!("  Auto-populate DB: {}", args.auto_populate_db);
    info!("  Bind address: {}", args.bind_address);
    info!("  Log level: {:?}", args.log_level);
    info!("  Request logging: {}", args.enable_request_logging);
    info!("  CORS enabled: {}", args.enable_cors);
    info!("  Logging system enabled: {}", args.enable_logging);
    if args.enable_logging {
        info!("  Logging DB path: {:?}", args.logging_db_path);
    }

    // Create the event bus - the heart of our hook-first architecture
    let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
    info!("✅ Event bus initialized");

    // Initialize logging system
    let logging_service = if args.enable_logging {
        // Create logging service configuration
        let logging_config = LogServiceBuilder::new()
            .db_path(args.logging_db_path.join("oxidedb.log.sqlite"))
            .retention_days(90)
            .enable_metrics(true)
            .build();

        // Create the logging service
        let log_service = Arc::new(LogService::with_config(logging_config).await
            .map_err(|e| AppError::internal(format!("Failed to initialize logging service: {}", e)))?);
        
        // Create bridge to oxide-core traits
        let bridge = Arc::new(LogServiceBridge::new(log_service));
        
        info!("✅ Logging system initialized with SQLite backend");
        Some(bridge)
    } else {
        info!("⚠️  Logging system disabled");
        None
    };

    // Initialize plugin runtime with the specified security policy
    let security_policies: oxide_core::plugin_security::SecurityPolicies = args.security_policy.clone().into();
    
    let mut plugin_runtime = WasmtimePluginRuntime::new_with_security_policies(security_policies)
        .map_err(|e| AppError::internal(format!("Failed to create plugin runtime: {}", e)))?;
    
    // Load plugins from the plugin folder
    if args.plugin_folder.exists() {
        load_plugins_from_folder(&mut plugin_runtime, &args.plugin_folder, &event_bus).await?;
    } else {
        info!("Plugin folder {:?} does not exist, skipping plugin loading", args.plugin_folder);
    }
    
    // Wrap the plugin runtime in Arc<Mutex<>> for sharing
    let shared_plugin_runtime = Arc::new(Mutex::new(plugin_runtime));
    
    // Create plugin bridge and register loaded plugins with event system
    let plugin_bridge = PluginEventBridge::new(Arc::clone(&shared_plugin_runtime));
    
    // Get list of loaded plugins and register them with the event system
    let loaded_plugins = {
        let runtime_guard = shared_plugin_runtime.lock().map_err(|_| {
            AppError::internal("Failed to acquire plugin runtime lock")
        })?;
        runtime_guard.list_plugins()
    };
    
    for plugin_name in loaded_plugins {
        let plugin_handler = plugin_bridge.create_before_create_handler(plugin_name.clone());
        event_bus.subscribe_before(
            BeforeEventType::RecordCreate.name(),
            plugin_handler,
        )?;
        info!("✅ Plugin '{}' registered with event system", plugin_name);
    }
    
    // Register a sample event listener to demonstrate hooking if no plugins are loaded
    if shared_plugin_runtime.lock().unwrap().list_plugins().is_empty() {
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
        info!("✅ Demo event listener registered (no plugins loaded)");
    }

    // Initialize AuthService with configuration
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev_secret_key_change_in_production".to_string());
    let auth_config = oxide_core::auth::AuthServiceConfig::new(jwt_secret);
    let auth_service = Arc::new(AuthService::new(auth_config));
    info!("✅ Authentication service initialized");

    // Initialize the database with event integration
    let database_path = if args.db_path.to_string_lossy() == ":memory:" {
        ":memory:".to_string()
    } else {
        // Ensure the database directory exists
        if let Some(parent) = args.db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
        }
        
        // If the path is a directory, append the default database filename
        if args.db_path.is_dir() {
            args.db_path.join("oxidedb.sqlite").to_string_lossy().to_string()
        } else {
            // Ensure parent directory exists for file path
            if let Some(parent) = args.db_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
            }
            args.db_path.to_string_lossy().to_string()
        }
    };
    
    let database = Arc::new(SqliteDb::new(
        &database_path,
        Arc::clone(&event_bus),
        Arc::clone(&auth_service),
    )?);
    database.initialize().await?;
    info!("✅ SQLite database initialized with event integration and authentication");

    // Update auth service configuration with discovered auth collections
    let auth_collections = database.list_auth_collections().await?;
    auth_service.update_auth_collections(&auth_collections);
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

    // Auto-populate database if requested
    if args.auto_populate_db {
        populate_sample_data(&database, &auth_service).await?;
        
        // Also populate some sample log data if logging is enabled
        if let Some(ref logging) = logging_service {
            populate_sample_logs(logging).await?;
        }
    }

    // Determine admin UI configuration based on mode
    let admin_enabled = args.enable_admin && !matches!(args.admin_mode, AdminMode::Disabled);
    
    // Validate external admin UI path if needed
    if matches!(args.admin_mode, AdminMode::External) && admin_enabled {
        if !args.admin_ui_path.exists() {
            warn!("External admin UI path {:?} does not exist, admin UI will be unavailable", args.admin_ui_path);
        } else {
            info!("Using external admin UI from: {:?}", args.admin_ui_path);
        }
    }

    // Create route configuration based on arguments
    let route_config = oxide_api::routes::RouteConfig {
        enable_admin: admin_enabled,
        enable_cors: args.enable_cors,
        enable_tracing: args.enable_request_logging,
        admin_mode: match args.admin_mode {
            AdminMode::Embedded => oxide_api::routes::AdminUiMode::Embedded,
            AdminMode::External => oxide_api::routes::AdminUiMode::External(args.admin_ui_path.clone()),
            AdminMode::Disabled => oxide_api::routes::AdminUiMode::Disabled,
        },
        admin_path: args.admin_path.clone(),
    };

    // Create and start the API server
    info!("🚀 Starting API server...");
    let api_server = if let Some(ref logging_service) = logging_service {
        // Create logging API service
        let log_api_service = oxide_logging::api::LogApiService::new(logging_service.inner().clone());
        let logging_api_service = Arc::new(oxide_api::services::LoggingApiService::new(log_api_service));
        
        ApiServer::new_with_logging(
            Arc::clone(&database) as Arc<dyn Db>,
            Arc::clone(&event_bus),
            Arc::clone(&auth_service),
            Arc::clone(logging_service),
            logging_api_service,
            args.bind_address.clone(),
            args.api_port,
        )
    } else {
        ApiServer::new(
            Arc::clone(&database) as Arc<dyn Db>,
            Arc::clone(&event_bus),
            Arc::clone(&auth_service),
            args.bind_address.clone(),
            args.api_port,
        )
    };

    print_startup_info(&args);

    // Start the server with custom configuration
    api_server.start_with_config(route_config).await?;

    Ok(())
}

/// Register a superuser account
async fn register_superuser_command(args: RegisterSuperuserArgs) -> Result<(), AppError> {
    info!("Registering superuser account");
    info!("Database path: {:?}", args.db_path);
    info!("Email: {}", args.email);

    // Initialize minimal services needed for user registration
    let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
    
    let jwt_secret = std::env::var("JWT_SECRET")
        .unwrap_or_else(|_| "dev_secret_key_change_in_production".to_string());
    let auth_config = oxide_core::auth::AuthServiceConfig::new(jwt_secret);
    let auth_service = Arc::new(AuthService::new(auth_config));

    // Ensure the database directory exists
    if let Some(parent) = args.db_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
    }

    let database_path = if args.db_path.is_dir() {
        args.db_path.join("oxidedb.sqlite").to_string_lossy().to_string()
    } else {
        // Ensure parent directory exists for file path
        if let Some(parent) = args.db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
        }
        args.db_path.to_string_lossy().to_string()
    };

    let database = Arc::new(SqliteDb::new(
        &database_path,
        Arc::clone(&event_bus),
        Arc::clone(&auth_service),
    )?);
    database.initialize().await?;

    // Update auth service with discovered collections
    let auth_collections = database.list_auth_collections().await?;
    auth_service.update_auth_collections(&auth_collections);

    // Create permission service for authorization hooks
    let permission_service = Arc::new(oxide_api::services::DatabasePermissionService::new(
        Arc::clone(&database) as Arc<dyn oxide_db::Db>
    ));

    // Register all system hooks using the new centralized system
    // This is CRITICAL for password hashing to work properly!
    register_system_hooks(
        event_bus.as_ref(), 
        Arc::clone(&auth_service),
        Arc::clone(&permission_service) as Arc<dyn oxide_core::auth::PermissionService>
    ).await?;
    info!("✅ System hooks registered for password hashing and validation");

    // Find superuser collection or use default
    let superuser_collection = auth_collections.iter()
        .find(|c| c.name == "superusers")
        .map(|c| c.name.as_str())
        .unwrap_or("users");

    if let Some(superuser_config) = auth_service.config().get_auth_collection(superuser_collection) {
        let register_request = oxide_db::db::RegisterRequest {
            collection: superuser_collection.to_string(),
            identifier: args.email,
            credential: args.password,
            additional_data: Some(serde_json::json!({
                "verified": true,
                "name": args.name.unwrap_or_else(|| "System Administrator".to_string()),
                "role": "superuser"
            })),
        };

        match database.register_user(register_request, &superuser_config).await {
            Ok(user_id) => {
                info!("✅ Superuser registered successfully with ID: {} in collection '{}'", user_id, superuser_collection);
            }
            Err(e) => {
                error!("❌ Failed to register superuser: {}", e);
                return Err(e);
            }
        }
    } else {
        return Err(AppError::internal(format!("Auth collection '{}' not found", superuser_collection)));
    }

    Ok(())
}

/// Load plugins from the specified folder
async fn load_plugins_from_folder(
    plugin_runtime: &mut WasmtimePluginRuntime,
    plugin_folder: &PathBuf,
    event_bus: &Arc<dyn EventBus>,
) -> Result<(), AppError> {
    use std::fs;
    
    info!("Loading plugins from folder: {:?}", plugin_folder);
    
    let entries = fs::read_dir(plugin_folder)
        .map_err(|e| AppError::internal(format!("Failed to read plugin folder: {}", e)))?;

    let mut loaded_plugins = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| AppError::internal(format!("Failed to read directory entry: {}", e)))?;
        let path = entry.path();
        
        if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
            let plugin_name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown");
            
            info!("Loading plugin: {} from {:?}", plugin_name, path);
            
            let wasm_bytes = fs::read(&path)
                .map_err(|e| AppError::internal(format!("Failed to read WASM file {:?}: {}", path, e)))?;
            
            // Load plugin with development-friendly settings
            use oxide_core::plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits};
            use oxide_core::auth::CrudOperation;
            
            plugin_runtime.load_plugin_with_trust(
                plugin_name,
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
            ).map_err(|e| AppError::internal(format!("Failed to load plugin {}: {}", plugin_name, e)))?;
            
            info!("✅ Plugin '{}' loaded successfully", plugin_name);
            
            // Test plugin initialization
            let test_payload = EventPayload {
                event_type: "plugin_init".to_string(),
                collection: "test".to_string(),
                data: "{}".to_string(),
                metadata: serde_json::json!({}),
            };
            
            let _init_response = plugin_runtime.call_plugin_function(
                plugin_name,
                plugin_exports::PLUGIN_INIT,
                &test_payload,
            ).map_err(|e| AppError::internal(format!("Failed to initialize plugin {}: {}", plugin_name, e)))?;
            
            info!("✅ Plugin '{}' initialized successfully", plugin_name);
            loaded_plugins.push(plugin_name.to_string());
        }
    }
    
    info!("✅ Loaded {} plugins from folder", loaded_plugins.len());
    Ok(())
}

/// Populate the database with sample data
async fn populate_sample_data(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
) -> Result<(), AppError> {
    info!("🚀 Auto-populating database with sample data...");

    // Get auth service configuration to determine available collections
    let auth_collections = database.list_auth_collections().await?;
    
    if auth_collections.is_empty() {
        warn!("No auth collections found. Auth system may not be properly configured.");
        return Ok(());
    }

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

            match database.register_user(register_request, &superuser_config).await {
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

            match database.register_user(register_request, &user_config).await {
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

        match database.authenticate_user(auth_request, &superuser_config).await {
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

    Ok(())
}

/// Populate sample log data for demonstration
async fn populate_sample_logs(
    logging_service: &LogServiceBridge,
) -> Result<(), AppError> {
    use oxide_core::LogContext;
    
    info!("🚀 Populating sample log data...");

    // Create some sample log entries
    let context = LogContext::new()
        .with_user_id("admin@example.com")
        .with_client_ip("127.0.0.1")
        .with_user_agent("OxideDB-Demo/1.0");

    // Log system startup
    logging_service.log_system_event(
        "system_startup".to_string(),
        "OxideDB system started successfully".to_string(),
        context.clone(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log system event: {}", e)))?;

    // Log some authentication events
    logging_service.log_authentication(
        "admin@example.com".to_string(),
        "login".to_string(),
        "success".to_string(),
        context.clone(),
        Some(10), // Low risk
    ).await.map_err(|e| AppError::internal(format!("Failed to log auth event: {}", e)))?;

    // Log data access
    logging_service.log_data_access(
        "admin@example.com".to_string(),
        "users".to_string(),
        "read".to_string(),
        context.clone().with_collection("users"),
    ).await.map_err(|e| AppError::internal(format!("Failed to log data access: {}", e)))?;

    // Log configuration change
    logging_service.log_configuration_change(
        "admin@example.com".to_string(),
        "auth_settings".to_string(),
        "update_retention_policy".to_string(),
        context.clone(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log config change: {}", e)))?;

    // Log some informational messages
    logging_service.info(
        "Sample data population completed successfully".to_string(),
        "system".to_string(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log info: {}", e)))?;

    logging_service.warn(
        "This is a demonstration warning message".to_string(),
        "demo".to_string(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log warning: {}", e)))?;

    // Force flush all pending logs to ensure they're written to database
    logging_service.flush().await.map_err(|e| AppError::internal(format!("Failed to flush logs: {}", e)))?;

    info!("✅ Sample log data populated successfully");
    Ok(())
}

/// Print startup information
fn print_startup_info(args: &StartArgs) {
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
    info!("  ✅ WASM Plugin system with configurable security policies");
    info!("");
    info!("🌐 API server is running at: http://{}:{}", args.bind_address, args.api_port);
    
    let admin_enabled = args.enable_admin && !matches!(args.admin_mode, AdminMode::Disabled);
    if admin_enabled {
        info!("🎨 Admin UI is available at: http://{}:{}{}", args.bind_address, args.api_port, args.admin_path);
        match args.admin_mode {
            AdminMode::Embedded => info!("   Mode: Embedded (built-in UI)"),
            AdminMode::External => info!("   Mode: External (serving from {:?})", args.admin_ui_path),
            AdminMode::Disabled => {} // This case is handled above
        }
    } else {
        info!("🚫 Admin UI is disabled");
    }
    
    info!("📋 Available endpoints:");
    info!("  - GET  /health                              - Health check");
    if admin_enabled {
        info!("  - GET  {}                               - Admin UI", args.admin_path);
    }
    info!("  - GET  /collections                         - List collections");
    info!("  - POST /collections                         - Create collection");
    info!("  - DEL  /collections/{{collection}}            - Delete collection");
    info!("  - GET  /collections/{{collection}}/stats      - Collection stats");
    info!("  - GET  /collections/{{collection}}/records    - List records");
    info!("  - POST /collections/{{collection}}/records    - Create record");
    info!("  - GET  /collections/{{collection}}/records/{{id}} - Get record");
    info!("  - PUT  /collections/{{collection}}/records/{{id}} - Update record");
    info!("  - DEL  /collections/{{collection}}/records/{{id}} - Delete record");
    if args.enable_logging {
        info!("  📊 Logging endpoints:");
        info!("  - GET  /api/logs                            - Query logs");
        info!("  - GET  /api/audit                           - Query audit events");
        info!("  - GET  /api/logs/metrics                    - Logging metrics");
        info!("  - GET  /api/logs/health                     - Logging health");
    }
    info!("");
}
