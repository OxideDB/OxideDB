//! Application Startup and Service Initialization
//!
//! This module provides centralized initialization logic for all OxideDB services
//! including the event bus, database, authentication, plugins, and HTTP server.

use crate::{OxideDbConfig, Result, sample_data};
use oxide_plugin_runtime::PluginManager;
use oxide_api::{server::ApiServer, services::{DatabasePermissionService, LoggingApiService}};
use oxide_core::{AppError, AuthService, EventBus, InMemoryEventBus, register_system_hooks};
use oxide_db::{Db, SqliteDb};
use oxide_logging::{LogService, LogServiceBuilder, LogServiceBridge};
use oxide_vfs;
use std::sync::Arc;
use tracing::{info, warn, debug};
use std::fmt::Write;

/// Application services container
#[derive(Clone)]
pub struct ApplicationServices {
    pub event_bus: Arc<dyn EventBus>,
    pub database: Arc<SqliteDb>,
    pub auth_service: Arc<AuthService>,
    pub permission_service: Arc<DatabasePermissionService>,
    pub logging_service: Option<Arc<LogServiceBridge>>,
    pub logging_api_service: Option<Arc<LoggingApiService>>,
    pub plugin_manager: Option<Arc<PluginManager>>,
    pub vfs_service: Option<Arc<dyn oxide_core::VirtualFileSystem>>,
}

/// Application bootstrap orchestrator
pub struct ApplicationBootstrap {
    config: OxideDbConfig,
}

impl ApplicationBootstrap {
    /// Create a new application bootstrap with the given configuration
    pub fn new(config: OxideDbConfig) -> Self {
        Self { config }
    }

    /// Initialize all application services in the correct order
    pub async fn initialize(&self) -> Result<ApplicationServices> {
        info!("🚀 Starting OxideDB application initialization");
        
        // Force initialization of the global process start time for accurate uptime tracking
        let _ = &*oxide_db::dashboard_stats_service::PROCESS_START;
        debug!("✅ Process start time initialized for uptime tracking");
        
        self.log_configuration();

        // 1. Initialize event bus - the heart of our hook-first architecture
        let event_bus = self.initialize_event_bus()?;

        // 2. Initialize logging system (optional)
        let logging_service = self.initialize_logging_system().await?;

        // 3. Initialize authentication service
        let auth_service = self.initialize_auth_service()?;

        // 4. Initialize database with dependencies
        let database = self.initialize_database(
            Arc::clone(&event_bus),
            Arc::clone(&auth_service),
        ).await?;

        // 5. Ensure auth system collections exist
        self.ensure_auth_collections_exist(&database).await?;

        // 6. Update auth service with discovered collections
        self.update_auth_collections(&database, &auth_service).await?;

        // 7. Initialize permission service
        let permission_service = self.initialize_permission_service(Arc::clone(&database))?;

        // 8. Register system hooks
        self.register_system_hooks(
            &event_bus,
            Arc::clone(&auth_service),
            Arc::clone(&permission_service),
        ).await?;

        // 9. Initialize VFS service (optional)
        let vfs_service = self.initialize_vfs_system(Arc::clone(&event_bus)).await?;

        // 10. Register dashboard activity listener
        self.register_dashboard_activity_listener(&event_bus, &database, &logging_service, &vfs_service).await?;

        // 11. Initialize plugin system (optional)
        let plugin_manager = self.initialize_plugin_system(
            Arc::clone(&event_bus), 
            Arc::clone(&database) as Arc<dyn oxide_db::Db>
        ).await?;

        // 12. Create logging API service if logging is enabled
        let logging_api_service = self.create_logging_api_service(&logging_service);

        // 13. Populate sample data if requested
        if self.config.database.auto_populate {
            self.populate_sample_data(&database, &auth_service, &logging_service).await?;
        }

        info!("✅ Application initialization completed successfully");

        Ok(ApplicationServices {
            event_bus,
            database,
            auth_service,
            permission_service,
            logging_service,
            logging_api_service,
            plugin_manager,
            vfs_service,
        })
    }

    /// Create and start the API server with the initialized services
    pub async fn start_server(&self, services: ApplicationServices) -> Result<()> {
        info!("🚀 Starting API server...");

        // Create route configuration
        let route_config = self.create_route_config()?;

        // Create API server based on available services
        let api_server = if let Some(ref logging_service) = services.logging_service {
            ApiServer::new_with_logging(
                Arc::clone(&services.database) as Arc<dyn Db>,
                Arc::clone(&services.event_bus),
                Arc::clone(&services.auth_service),
                Arc::clone(logging_service),
                services.logging_api_service.unwrap(),
                self.config.server.bind_address.clone(),
                self.config.server.api_port,
            )
        } else {
            ApiServer::new(
                Arc::clone(&services.database) as Arc<dyn Db>,
                Arc::clone(&services.event_bus),
                Arc::clone(&services.auth_service),
                self.config.server.bind_address.clone(),
                self.config.server.api_port,
            )
        };

        // Print startup information
        self.print_startup_info();

        // Start the server with services available
        match (services.plugin_manager, services.vfs_service) {
            (Some(plugin_manager), Some(vfs_service)) => {
                // Both plugin manager and VFS service available - need to modify the server method to handle this
                api_server.start_with_config_plugin_manager_and_optional_vfs(route_config, Some(plugin_manager), Some(vfs_service)).await?;
            }
            (Some(plugin_manager), None) => {
                // Only plugin manager available
                api_server.start_with_config_plugin_manager_and_optional_vfs(route_config, Some(plugin_manager), None).await?;
            }
            (None, Some(vfs_service)) => {
                // Only VFS service available - need to create a new method for this
                api_server.start_with_config_plugin_manager_and_optional_vfs(route_config, None, Some(vfs_service)).await?;
            }
            (None, None) => {
                // Neither available
                api_server.start_with_config_plugin_manager_and_optional_vfs(route_config, None, None).await?;
            }
        }

        Ok(())
    }

    /// Initialize the event bus
    fn initialize_event_bus(&self) -> Result<Arc<dyn EventBus>> {
        let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
        info!("✅ Event bus initialized");
        Ok(event_bus)
    }

    /// Initialize the logging system if enabled
    async fn initialize_logging_system(&self) -> Result<Option<Arc<LogServiceBridge>>> {
        if !self.config.logging.enable_logging {
            info!("⚠️ Logging system disabled");
            return Ok(None);
        }

        // Create logging service configuration
        let logging_config = LogServiceBuilder::new()
            .db_path(self.config.logging.logging_db_path.join("oxidedb.log.sqlite"))
            .retention_days(90)
            .enable_metrics(true)
            .build();

        // Create the logging service
        let log_service = Arc::new(LogService::with_config(logging_config).await
            .map_err(|e| AppError::internal(format!("Failed to initialize logging service: {}", e)))?);
        
        // Create bridge to oxide-core traits
        let bridge = Arc::new(LogServiceBridge::new(log_service));
        
        info!("✅ Logging system initialized with SQLite backend");
        Ok(Some(bridge))
    }

    /// Initialize the authentication service
    fn initialize_auth_service(&self) -> Result<Arc<AuthService>> {
        let auth_config = oxide_core::auth::AuthServiceConfig::new(self.config.security.jwt_secret.clone());
        let auth_service = Arc::new(AuthService::new(auth_config));
        info!("✅ Authentication service initialized");
        Ok(auth_service)
    }

    /// Initialize the database
    async fn initialize_database(
        &self,
        event_bus: Arc<dyn EventBus>,
        auth_service: Arc<AuthService>,
    ) -> Result<Arc<SqliteDb>> {
        let database_path = self.config.database.resolved_path()?;
        
        let database = Arc::new(SqliteDb::new(
            &database_path,
            event_bus,
            auth_service,
        )?);
        
        database.initialize().await?;
        info!("✅ SQLite database initialized with event integration and authentication");
        Ok(database)
    }

    /// Ensure auth system collections exist
    async fn ensure_auth_collections_exist(&self, database: &Arc<SqliteDb>) -> Result<()> {
        info!("🔐 Ensuring auth system collections exist...");

        // Get existing auth collections
        let existing_collections = database.list_auth_collections().await?;
        let existing_names: std::collections::HashSet<String> = existing_collections
            .iter()
            .map(|c| c.name.clone())
            .collect();

        // Check and create _users collection if it doesn't exist
        if !existing_names.contains("_users") {
            info!("📋 Creating _users auth collection...");
            let mut users_schema = oxide_core::CollectionSchema::new(
                "_users".to_string(), 
                oxide_core::CollectionType::Auth
            );
            users_schema.add_field(
                "email".to_string(),
                oxide_core::FieldDefinition::new(oxide_core::FieldType::Email).required().unique(),
            );
            users_schema.add_field(
                "password".to_string(),
                oxide_core::FieldDefinition::new(oxide_core::FieldType::Password).required(),
            );

            match database.create_collection_with_schema(users_schema).await {
                Ok(_) => {
                    info!("✅ _users auth collection created successfully");
                }
                Err(e) if e.to_string().contains("already exists") => {
                    info!("ℹ️ _users auth collection already exists");
                }
                Err(e) => {
                    warn!("❌ Failed to create _users auth collection: {}", e);
                }
            }
        } else {
            info!("ℹ️ _users auth collection already exists");
        }

        // Check and create _superusers collection if it doesn't exist
        if !existing_names.contains("_superusers") {
            info!("📋 Creating _superusers auth collection...");
            let mut superusers_schema = oxide_core::CollectionSchema::new(
                "_superusers".to_string(), 
                oxide_core::CollectionType::Auth
            );
            superusers_schema.add_field(
                "email".to_string(),
                oxide_core::FieldDefinition::new(oxide_core::FieldType::Email).required().unique(),
            );
            superusers_schema.add_field(
                "password".to_string(),
                oxide_core::FieldDefinition::new(oxide_core::FieldType::Password).required(),
            );

            match database.create_collection_with_schema(superusers_schema).await {
                Ok(_) => {
                    info!("✅ _superusers auth collection created successfully");
                }
                Err(e) if e.to_string().contains("already exists") => {
                    info!("ℹ️ _superusers auth collection already exists");
                }
                Err(e) => {
                    warn!("❌ Failed to create _superusers auth collection: {}", e);
                }
            }
        } else {
            info!("ℹ️ _superusers auth collection already exists");
        }

        info!("✅ Auth system collections verification completed");
        Ok(())
    }

    /// Update auth service with discovered collections
    async fn update_auth_collections(
        &self,
        database: &Arc<SqliteDb>,
        auth_service: &Arc<AuthService>,
    ) -> Result<()> {
        let auth_collections = database.list_auth_collections().await?;
        auth_service.update_auth_collections(&auth_collections);
        info!("✅ Auth service updated with {} auth collections", auth_collections.len());
        Ok(())
    }

    /// Initialize the permission service
    fn initialize_permission_service(
        &self,
        database: Arc<SqliteDb>,
    ) -> Result<Arc<DatabasePermissionService>> {
        let permission_service = Arc::new(DatabasePermissionService::new(
            database as Arc<dyn oxide_db::Db>
        ));
        info!("✅ Permission service initialized");
        Ok(permission_service)
    }

    /// Register all system hooks
    async fn register_system_hooks(
        &self,
        event_bus: &Arc<dyn EventBus>,
        auth_service: Arc<AuthService>,
        permission_service: Arc<DatabasePermissionService>,
    ) -> Result<()> {
        register_system_hooks(
            event_bus.as_ref(),
            auth_service,
            permission_service as Arc<dyn oxide_core::auth::PermissionService>,
        ).await?;
        info!("✅ All system hooks registered via centralized registry");
        Ok(())
    }

    /// Register dashboard activity listener
    async fn register_dashboard_activity_listener(
        &self,
        event_bus: &Arc<dyn EventBus>,
        database: &Arc<SqliteDb>,
        logging_service: &Option<Arc<LogServiceBridge>>,
        vfs_service: &Option<Arc<dyn oxide_core::VirtualFileSystem>>,
    ) -> Result<()> {
        use oxide_db::{DatabaseDashboardStatsService, LoggingStatsBridge, VfsStatsBridge, register_dashboard_activity_listener};
        
        // Create logging bridge if logging service is available
        let logging_bridge = logging_service.as_ref().map(|logging| {
            let log_api_service = oxide_logging::api::LogApiService::new(logging.inner().clone());
            Arc::new(LoggingStatsBridge::with_service(Arc::new(log_api_service))) as Arc<dyn oxide_db::LoggingStatsProvider>
        });
        
        // Create VFS bridge if VFS service is available
        let vfs_bridge = vfs_service.as_ref().map(|vfs| {
            Arc::new(VfsStatsBridge::with_service(Arc::clone(vfs))) as Arc<dyn oxide_db::VfsStatsProvider>
        });
        
        // Create dashboard service with full integration for activity recording
        let dashboard_service = Arc::new(DatabaseDashboardStatsService::with_full_integration(
            database.clone() as Arc<dyn oxide_db::Db>,
            logging_bridge,
            vfs_bridge,
        )) as Arc<dyn oxide_core::DashboardStatsService>;
        
        // Register the activity listener
        register_dashboard_activity_listener(event_bus, dashboard_service).await?;
        info!("✅ Dashboard activity listener registered with full service integration");
        Ok(())
    }

    /// Initialize the plugin system if enabled
    async fn initialize_plugin_system(
        &self,
        event_bus: Arc<dyn EventBus>,
        database: Arc<dyn oxide_db::Db>,
    ) -> Result<Option<Arc<PluginManager>>> {
        let mut plugin_manager = PluginManager::new(
            database.clone(),
            self.config.plugins.security_policy.clone().into(),
        )?;

        // First, load plugins from database (persistent plugins)
        let plugins_dir = self.config.plugins.plugin_folder.clone();
        let plugin_config_service = oxide_api::services::PluginConfigService::new(database.clone(), plugins_dir.clone());
        let enabled_plugins = plugin_config_service.get_enabled_plugins().await?;
        
        if !enabled_plugins.is_empty() {
            info!("🔌 Loading {} enabled plugins from database", enabled_plugins.len());
            
            for config in enabled_plugins {
                info!("📦 Loading plugin from database: {} (v{})", config.name, config.version);
                info!("🔍 Plugin capabilities from database: {} capabilities", config.capabilities.len());
                
                // Get WASM file path from configuration
                let wasm_path = if let Some(ref path) = config.wasm_path {
                    plugins_dir.join(path)
                } else {
                    warn!("❌ Plugin '{}' has no WASM path in configuration, skipping", config.name);
                    let _ = plugin_config_service.update_plugin_status(&config.name, oxide_core::plugin_config::PluginStatus::Error).await;
                    continue;
                };
                
                // Use the new load_plugin_with_config method that automatically uses database capabilities
                if let Err(e) = plugin_manager.load_plugin_with_config(&config.name, &wasm_path, &config).await {
                    warn!("❌ Failed to load plugin '{}' from database: {}", config.name, e);
                    // Update status to error
                    let _ = plugin_config_service.update_plugin_status(&config.name, oxide_core::plugin_config::PluginStatus::Error).await;
                } else {
                    info!("✅ Successfully loaded plugin from database with database capabilities: {}", config.name);
                }
            }
        } else {
            info!("📭 No enabled plugins found in database");
        }

        // Then, load plugins from folder (legacy support)
        if self.config.plugins.plugin_folder.exists() {
            info!("🔌 Also loading plugins from folder: {:?}", self.config.plugins.plugin_folder);
            
            plugin_manager.load_plugins_from_folder(
                &self.config.plugins.plugin_folder,
                &event_bus,
            ).await?;
        } else {
            debug!("Plugin folder {:?} does not exist, skipping folder loading", 
                  self.config.plugins.plugin_folder);
        }

        plugin_manager.register_with_event_system(&event_bus).await?;

        let plugin_manager = Arc::new(plugin_manager);
        info!("✅ Plugin system initialized with database persistence");
        Ok(Some(plugin_manager))
    }

    /// Initialize VFS service if enabled
    async fn initialize_vfs_system(
        &self,
        event_bus: Arc<dyn EventBus>,
    ) -> Result<Option<Arc<dyn oxide_core::VirtualFileSystem>>> {
        // For now, VFS is always enabled but we can add a config option later
        // if !self.config.vfs.enable_vfs {
        //     info!("⚠️ VFS system disabled");
        //     return Ok(None);
        // }

        // Create VFS and backup directories
        let vfs_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("vfs");
        let backup_path = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("vfs_backups");

        // Initialize VFS system using the oxide-vfs crate
        let vfs_service = oxide_vfs::initialize_vfs_system(
            vfs_path,
            backup_path,
            Some(event_bus)
        ).await.map_err(|e| AppError::internal(format!("Failed to initialize VFS system: {}", e)))?;

        let vfs_service: Arc<dyn oxide_core::VirtualFileSystem> = Arc::new(vfs_service);

        // ---------------------------------------------------------------------------------
        // Ensure the built-in "default" namespace exists so that health probes succeed.
        // If it does not exist yet, create it with a permissive configuration.
        // ---------------------------------------------------------------------------------
        let default_ns = "default".to_string();
        match vfs_service.get_namespace_config(&default_ns).await {
            Ok(_) => {
                debug!("ℹ️ Default VFS namespace already exists");
            }
            Err(oxide_core::vfs::VfsError::AccessDenied { .. }) => {
                use oxide_core::vfs::VfsNamespaceConfig;

                let config = VfsNamespaceConfig {
                    namespace: default_ns.clone(),
                    quota_bytes: None,
                    enable_compression: true,
                    allowed_mime_types: None,
                    max_file_size: None,
                    enable_deduplication: true,
                    backup_config: None,
                };

                match vfs_service.create_namespace(config).await {
                    Ok(_) => info!("✅ Created default VFS namespace"),
                    Err(e) => warn!("❌ Failed to create default VFS namespace: {}", e),
                }
            }
            Err(e) => {
                // Other unexpected error (e.g., IO error). Log and continue startup.
                warn!("⚠️ Unable to verify default VFS namespace: {}", e);
            }
        }

        info!("✅ VFS system initialized with file storage and backup support");
        Ok(Some(vfs_service))
    }

    /// Create logging API service if logging is enabled
    fn create_logging_api_service(
        &self,
        logging_service: &Option<Arc<LogServiceBridge>>,
    ) -> Option<Arc<LoggingApiService>> {
        logging_service.as_ref().map(|logging| {
            let log_api_service = oxide_logging::api::LogApiService::new(logging.inner().clone());
            Arc::new(LoggingApiService::new(log_api_service))
        })
    }

    /// Populate sample data if requested
    async fn populate_sample_data(
        &self,
        database: &Arc<SqliteDb>,
        auth_service: &Arc<AuthService>,
        logging_service: &Option<Arc<LogServiceBridge>>,
    ) -> Result<()> {
        sample_data::populate_database_samples(database, auth_service).await?;
        
        if let Some(ref logging) = logging_service {
            sample_data::populate_logging_samples(logging).await?;
        }
        
        Ok(())
    }

    /// Create route configuration
    fn create_route_config(&self) -> Result<oxide_api::routes::RouteConfig> {
        let admin_enabled = self.config.server.enable_admin && 
            !matches!(self.config.server.admin_mode, crate::AdminMode::Disabled);
        
        // Validate external admin UI path if needed
        if matches!(self.config.server.admin_mode, crate::AdminMode::External) && admin_enabled {
            if !self.config.server.admin_ui_path.exists() {
                warn!("External admin UI path {:?} does not exist, admin UI will be unavailable", 
                      self.config.server.admin_ui_path);
            } else {
                info!("Using external admin UI from: {:?}", self.config.server.admin_ui_path);
            }
        }

        Ok(oxide_api::routes::RouteConfig {
            enable_admin: admin_enabled,
            enable_cors: self.config.server.enable_cors,
            enable_tracing: self.config.server.enable_request_logging,
            admin_mode: match &self.config.server.admin_mode {
                crate::AdminMode::Embedded => oxide_api::routes::AdminUiMode::Embedded,
                crate::AdminMode::External => oxide_api::routes::AdminUiMode::External(
                    self.config.server.admin_ui_path.clone()
                ),
                crate::AdminMode::Disabled => oxide_api::routes::AdminUiMode::Disabled,
            },
            admin_path: self.config.server.admin_path.clone(),
        })
    }

    /// Log the current configuration
    fn log_configuration(&self) {
        use std::fmt::Write;

        // Build a single, multi-line summary string so we emit only one log record.
        let mut msg = String::from("\n⚙️  Runtime configuration\n");

        // Core configuration
        let _ = writeln!(msg, "  Database path        : {:?}", self.config.database.db_path);
        let _ = writeln!(msg, "  Security policy      : {:?}", self.config.plugins.security_policy);
        let _ = writeln!(msg, "  Plugin folder        : {:?}", self.config.plugins.plugin_folder);
        let _ = writeln!(msg, "  API port             : {}",  self.config.server.api_port);
        let _ = writeln!(msg, "  Bind address         : {}",  self.config.server.bind_address);

        // Admin UI
        let _ = writeln!(msg, "  Admin enabled        : {}",  self.config.server.enable_admin);
        let _ = writeln!(msg, "  Admin mode           : {:?}", self.config.server.admin_mode);
        if matches!(self.config.server.admin_mode, crate::AdminMode::External) {
            let _ = writeln!(msg, "  Admin UI path        : {:?}", self.config.server.admin_ui_path);
        }

        // Misc.
        let _ = writeln!(msg, "  Auto-populate DB     : {}",  self.config.database.auto_populate);
        let _ = writeln!(msg, "  Log level            : {:?}", self.config.logging.log_level);
        let _ = writeln!(msg, "  Request logging      : {}",  self.config.server.enable_request_logging);
        let _ = writeln!(msg, "  CORS enabled         : {}",  self.config.server.enable_cors);
        let _ = writeln!(msg, "  Logging system       : {}",  self.config.logging.enable_logging);
        if self.config.logging.enable_logging {
            let _ = writeln!(msg, "  Logging DB path      : {:?}", self.config.logging.logging_db_path);
        }

        // Emit as DEBUG to keep INFO channel cleaner; users can opt-in via RUST_LOG=debug.
        debug!("{}", msg);
    }

    /// Print a concise startup banner.
    fn print_startup_info(&self) {
        use std::fmt::Write;

        let mut banner = String::new();

        // Header
        banner.push_str("🎉 OxideDB startup completed successfully!\n\n");

        // Architecture highlights (static content)
        banner.push_str("Architecture highlights:\n");
        banner.push_str("  • Modular workspace with strict crate boundaries\n");
        banner.push_str("  • EventBus-centric, hook-first design\n");
        banner.push_str("  • Async (Tokio) & SQLite backend\n");
        banner.push_str("  • WASM plugin runtime & audit logging\n\n");

        // Runtime URLs
        let _ = writeln!(banner, "🌐 API server : http://{}:{}", self.config.server.bind_address, self.config.server.api_port);

        let admin_enabled = self.config.server.enable_admin &&
            !matches!(self.config.server.admin_mode, crate::AdminMode::Disabled);

        if admin_enabled {
            let _ = writeln!(banner, "🎨 Admin UI   : http://{}:{}{}", self.config.server.bind_address, self.config.server.api_port, self.config.server.admin_path);
            match &self.config.server.admin_mode {
                crate::AdminMode::Embedded => banner.push_str("   Mode        : Embedded (built-in UI)\n"),
                crate::AdminMode::External => {
                    let _ = writeln!(banner, "   Mode        : External (serving from {:?})", self.config.server.admin_ui_path);
                }
                crate::AdminMode::Disabled => {}
            }
        } else {
            banner.push_str("🚫 Admin UI   : disabled\n");
        }

        banner.push_str("\nTip: run with RUST_LOG=debug to see full endpoint list and configuration details.\n");

        // Emit banner in a single INFO record.
        info!("{}", banner);

        // Detailed endpoints (debug level only)
        self.print_available_endpoints();
    }

    /// Print available API endpoints at DEBUG level.
    fn print_available_endpoints(&self) {
        debug!("📋 Available endpoints:");
        debug!("  • GET  /health                               – Health check");

        let admin_enabled = self.config.server.enable_admin &&
            !matches!(self.config.server.admin_mode, crate::AdminMode::Disabled);
        if admin_enabled {
            debug!("  • GET  {}                                – Admin UI", self.config.server.admin_path);
        }

        debug!("  • GET  /collections                          – List collections");
        debug!("  • POST /collections                          – Create collection");
        debug!("  • DEL  /collections/{{collection}}             – Delete collection");
        debug!("  • GET  /collections/{{collection}}/stats       – Collection stats");
        debug!("  • GET  /collections/{{collection}}/records     – List records");
        debug!("  • POST /collections/{{collection}}/records     – Create record");
        debug!("  • GET  /collections/{{collection}}/records/{{id}} – Get record");
        debug!("  • PUT  /collections/{{collection}}/records/{{id}} – Update record");
        debug!("  • DEL  /collections/{{collection}}/records/{{id}} – Delete record");

        if self.config.logging.enable_logging {
            debug!("  📊 Logging endpoints:");
            debug!("  • GET  /api/logs                             – Query logs");
            debug!("  • GET  /api/audit                            – Query audit events");
            debug!("  • GET  /api/logs/metrics                     – Logging metrics");
            debug!("  • GET  /api/logs/health                      – Logging health");
        }
    }
}