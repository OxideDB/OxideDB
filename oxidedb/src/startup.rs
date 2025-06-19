//! Application Startup and Service Initialization
//!
//! This module provides centralized initialization logic for all OxideDB services
//! including the event bus, database, authentication, plugins, and HTTP server.

use crate::{OxideDbConfig, PluginManager, Result, sample_data};
use oxide_api::{server::ApiServer, services::{DatabasePermissionService, LoggingApiService}};
use oxide_core::{AppError, AuthService, EventBus, InMemoryEventBus, register_system_hooks};
use oxide_db::{Db, SqliteDb};
use oxide_logging::{LogService, LogServiceBuilder, LogServiceBridge};
use std::sync::Arc;
use tracing::{info, warn};

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

        // 5. Update auth service with discovered collections
        self.update_auth_collections(&database, &auth_service).await?;

        // 6. Initialize permission service
        let permission_service = self.initialize_permission_service(Arc::clone(&database))?;

        // 7. Register system hooks
        self.register_system_hooks(
            &event_bus,
            Arc::clone(&auth_service),
            Arc::clone(&permission_service),
        ).await?;

        // 8. Initialize plugin system (optional)
        let plugin_manager = self.initialize_plugin_system(Arc::clone(&event_bus)).await?;

        // 9. Create logging API service if logging is enabled
        let logging_api_service = self.create_logging_api_service(&logging_service);

        // 10. Populate sample data if requested
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

        // Start the server
        api_server.start_with_config(route_config).await?;

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

    /// Initialize the plugin system if enabled
    async fn initialize_plugin_system(
        &self,
        event_bus: Arc<dyn EventBus>,
    ) -> Result<Option<Arc<PluginManager>>> {
        if !self.config.plugins.plugin_folder.exists() {
            info!("Plugin folder {:?} does not exist, skipping plugin loading", 
                  self.config.plugins.plugin_folder);
            return Ok(None);
        }

        let mut plugin_manager = PluginManager::new(
            self.config.plugins.security_policy.clone().into(),
        )?;

        plugin_manager.load_plugins_from_folder(
            &self.config.plugins.plugin_folder,
            &event_bus,
        ).await?;

        plugin_manager.register_with_event_system(&event_bus).await?;

        let plugin_manager = Arc::new(plugin_manager);
        info!("✅ Plugin system initialized");
        Ok(Some(plugin_manager))
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
        info!("Configuration:");
        info!("  Database path: {:?}", self.config.database.db_path);
        info!("  Security policy: {:?}", self.config.plugins.security_policy);
        info!("  Plugin folder: {:?}", self.config.plugins.plugin_folder);
        info!("  API port: {}", self.config.server.api_port);
        info!("  Admin path: {}", self.config.server.admin_path);
        info!("  Admin enabled: {}", self.config.server.enable_admin);
        info!("  Admin mode: {:?}", self.config.server.admin_mode);
        if matches!(self.config.server.admin_mode, crate::AdminMode::External) {
            info!("  Admin UI path: {:?}", self.config.server.admin_ui_path);
        }
        info!("  Auto-populate DB: {}", self.config.database.auto_populate);
        info!("  Bind address: {}", self.config.server.bind_address);
        info!("  Log level: {:?}", self.config.logging.log_level);
        info!("  Request logging: {}", self.config.server.enable_request_logging);
        info!("  CORS enabled: {}", self.config.server.enable_cors);
        info!("  Logging system enabled: {}", self.config.logging.enable_logging);
        if self.config.logging.enable_logging {
            info!("  Logging DB path: {:?}", self.config.logging.logging_db_path);
        }
    }

    /// Print startup information
    fn print_startup_info(&self) {
        info!("🎉 OxideDB startup completed successfully!");
        info!("Architecture summary:");
        info!("  ✅ Cargo workspace with modular crate architecture");
        info!("  ✅ Hook-first architecture with EventBus");
        info!("  ✅ Database abstraction with SQLite implementation");
        info!("  ✅ Standardized error handling with AppError");
        info!("  ✅ Full async support with Tokio");
        info!("  ✅ All operations route through events for extensibility");
        info!("  ✅ HTTP API server with health check endpoint");
        info!("  ✅ REST API for collections and records");
        info!("  ✅ WASM Plugin system with configurable security policies");
        info!("  ✅ Comprehensive logging and audit system");
        info!("");
        info!("🌐 API server is running at: http://{}:{}", 
              self.config.server.bind_address, self.config.server.api_port);
        
        let admin_enabled = self.config.server.enable_admin && 
            !matches!(self.config.server.admin_mode, crate::AdminMode::Disabled);
        if admin_enabled {
            info!("🎨 Admin UI is available at: http://{}:{}{}", 
                  self.config.server.bind_address, self.config.server.api_port, self.config.server.admin_path);
            match &self.config.server.admin_mode {
                crate::AdminMode::Embedded => info!("   Mode: Embedded (built-in UI)"),
                crate::AdminMode::External => info!("   Mode: External (serving from {:?})", 
                                                    self.config.server.admin_ui_path),
                crate::AdminMode::Disabled => {} // This case is handled above
            }
        } else {
            info!("🚫 Admin UI is disabled");
        }
        
        self.print_available_endpoints();
    }

    /// Print available API endpoints
    fn print_available_endpoints(&self) {
        info!("📋 Available endpoints:");
        info!("  - GET  /health                              - Health check");
        
        let admin_enabled = self.config.server.enable_admin && 
            !matches!(self.config.server.admin_mode, crate::AdminMode::Disabled);
        if admin_enabled {
            info!("  - GET  {}                               - Admin UI", self.config.server.admin_path);
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
        
        if self.config.logging.enable_logging {
            info!("  📊 Logging endpoints:");
            info!("  - GET  /api/logs                            - Query logs");
            info!("  - GET  /api/audit                           - Query audit events");
            info!("  - GET  /api/logs/metrics                    - Logging metrics");
            info!("  - GET  /api/logs/health                     - Logging health");
        }
        info!("");
    }
} 