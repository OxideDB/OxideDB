//! HTTP server implementation
//!
//! This module contains the core HTTP server implementation for the OxideDB API.
//! The server is built on top of Axum and provides a REST API interface.

use oxide_core::{event::EventBus, logging::ApplicationLogger, AppError, AuthService};
use oxide_db::Db;
use oxide_logging::LogServiceBridge;
use crate::services::{LoggingApiService, DatabasePermissionService, plugin_config_service::PluginConfigService};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::TcpListener;
use tracing::{info, debug, warn};

use crate::routes::{build_router_with_config, build_router_with_config_and_middleware, RouteConfig};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Db>,
    pub event_bus: Arc<dyn EventBus>,
    pub auth_service: Arc<AuthService>,
    pub logging_service: Option<Arc<LogServiceBridge>>,
    pub logging_api_service: Option<Arc<LoggingApiService>>,
    pub plugin_manager: Option<Arc<oxide_plugin_runtime::PluginManager>>,
    pub database_permission_service: Arc<DatabasePermissionService>,
    pub plugin_config_service: Arc<PluginConfigService>,
    pub vfs_service: Option<Arc<dyn oxide_core::VirtualFileSystem>>,
    pub started_at: Instant,
}

/// The API server that handles HTTP requests
///
/// This server provides a REST API for interacting with the OxideDB database.
/// It integrates with the event system and database abstraction layer to
/// provide a complete HTTP interface.
pub struct ApiServer {
    db: Arc<dyn Db>,
    event_bus: Arc<dyn EventBus>,
    auth_service: Arc<AuthService>,
    logging_service: Option<Arc<LogServiceBridge>>,
    logging_api_service: Option<Arc<LoggingApiService>>,
    host: String,
    port: u16,
}

impl AppState {
    /// Update the AppState with a plugin manager
    pub fn with_plugin_manager(mut self, plugin_manager: Arc<oxide_plugin_runtime::PluginManager>) -> Self {
        self.plugin_manager = Some(plugin_manager);
        self
    }

    /// Update the AppState with a VFS service
    pub fn with_vfs_service(mut self, vfs_service: Arc<dyn oxide_core::VirtualFileSystem>) -> Self {
        self.vfs_service = Some(vfs_service);
        self
    }
}

impl ApiServer {
    /// Create a new API server instance
    ///
    /// # Arguments
    /// * `db` - The database implementation to use
    /// * `event_bus` - The event bus for dispatching events
    /// * `auth_service` - The authentication service
    /// * `host` - The host address to bind to
    /// * `port` - The port to listen on
    pub fn new(db: Arc<dyn Db>, event_bus: Arc<dyn EventBus>, auth_service: Arc<AuthService>, host: String, port: u16) -> Self {
        Self {
            db,
            event_bus,
            auth_service,
            logging_service: None,
            logging_api_service: None,
            host,
            port,
        }
    }

    /// Create a new API server instance with logging services
    ///
    /// # Arguments
    /// * `db` - The database implementation to use
    /// * `event_bus` - The event bus for dispatching events
    /// * `auth_service` - The authentication service
    /// * `logging_service` - The logging service bridge
    /// * `logging_api_service` - The logging API service
    /// * `host` - The host address to bind to
    /// * `port` - The port to listen on
    pub fn new_with_logging(
        db: Arc<dyn Db>, 
        event_bus: Arc<dyn EventBus>, 
        auth_service: Arc<AuthService>,
        logging_service: Arc<LogServiceBridge>,
        logging_api_service: Arc<LoggingApiService>,
        host: String, 
        port: u16
    ) -> Self {
        Self {
            db,
            event_bus,
            auth_service,
            logging_service: Some(logging_service),
            logging_api_service: Some(logging_api_service),
            host,
            port,
        }
    }

    /// Start the API server with default configuration
    ///
    /// This method starts the HTTP server and begins listening for requests.
    /// It will run indefinitely until stopped.
    pub async fn start(&self) -> Result<(), AppError> {
        self.start_with_config(RouteConfig::default()).await
    }

    /// Start the API server with custom configuration
    ///
    /// This allows for different configurations in different environments
    /// (e.g., disabling admin UI in production, serving external UI in development).
    pub async fn start_with_config(&self, config: RouteConfig) -> Result<(), AppError> {
        self.start_with_config_plugin_manager_and_optional_vfs(config, None, None).await
    }

    /// Start the API server with custom configuration and plugin manager
    ///
    /// This variant includes plugin support by passing the plugin manager to the app state.
    pub async fn start_with_config_and_plugin_manager(
        &self, 
        config: RouteConfig, 
        plugin_manager: Arc<oxide_plugin_runtime::PluginManager>
    ) -> Result<(), AppError> {
        self.start_with_config_plugin_manager_and_optional_vfs(config, Some(plugin_manager), None).await
    }

    /// Start the API server with custom configuration, optional plugin manager, and optional VFS service
    ///
    /// This is the main startup method that handles all combinations of services.
    pub async fn start_with_config_plugin_manager_and_optional_vfs(
        &self, 
        config: RouteConfig, 
        plugin_manager: Option<Arc<oxide_plugin_runtime::PluginManager>>,
        vfs_service: Option<Arc<dyn oxide_core::VirtualFileSystem>>
    ) -> Result<(), AppError> {
        info!("Starting API server on {}:{}", self.host, self.port);
        debug!("Server configuration: Admin enabled: {}, CORS enabled: {}, Tracing enabled: {}", 
               config.enable_admin, config.enable_cors, config.enable_tracing);

        // Log admin UI configuration details
        if config.enable_admin {
            match &config.admin_mode {
                crate::routes::AdminUiMode::Embedded => {
                    info!("Admin UI mode: Embedded (built-in UI)");
                    
                    // Check if embedded UI is available
                    if crate::handlers::admin::is_admin_ui_available() {
                        info!("✅ Embedded admin UI is available");
                    } else {
                        info!("⚠️  Embedded admin UI is not available (UI files not found)");
                    }
                }
                crate::routes::AdminUiMode::External(path) => {
                    info!("Admin UI mode: External (serving from {:?})", path);
                    
                    // Check if external UI is available
                    if crate::handlers::admin::is_external_admin_ui_available(path).await {
                        info!("✅ External admin UI is available at {:?}", path);
                    } else {
                        info!("⚠️  External admin UI is not available at {:?}", path);
                    }
                }
                crate::routes::AdminUiMode::Disabled => {
                    info!("Admin UI is disabled");
                }
            }
        } else {
            info!("Admin UI is disabled by configuration");
        }

        let database_permission_service = Arc::new(DatabasePermissionService::new(Arc::clone(&self.db)));
        // Default plugins directory
        let plugins_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("oxide-plugins");
        let plugin_config_service = Arc::new(PluginConfigService::new(Arc::clone(&self.db), plugins_dir));
        
        let state = AppState {
            db: Arc::clone(&self.db),
            event_bus: Arc::clone(&self.event_bus),
            auth_service: Arc::clone(&self.auth_service),
            logging_service: self.logging_service.clone(),
            logging_api_service: self.logging_api_service.clone(),
            plugin_manager: plugin_manager,
            database_permission_service,
            plugin_config_service,
            vfs_service: vfs_service,
            started_at: Instant::now(),
        };

        let app = build_router_with_config_and_middleware(config.clone(), state.clone());

        // Log all registered endpoints
        crate::routes::log_registered_endpoints(&state, &config);

        let listener = TcpListener::bind(&self.address()).await.map_err(|e| {
            AppError::internal(format!("Failed to bind to {}: {}", self.address(), e))
        })?;

        info!("✅ API server listening on {}", self.address());

        let server_result = axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await;

        if let Err(e) = server_result {
            return Err(AppError::internal(format!("Server error: {}", e)));
        }

        self.stop().await?;

        Ok(())
    }

    /// Stop the API server gracefully
    pub async fn stop(&self) -> Result<(), AppError> {
        info!("Stopping API server gracefully");

        if let Some(logging_service) = &self.logging_service {
            if let Err(e) = logging_service.flush().await {
                warn!("Failed to flush logs during API server shutdown: {}", e);
            }
        }

        self.db.close().await?;
        self.event_bus.shutdown().await?;

        info!("API server shutdown complete");

        Ok(())
    }

    /// Get the database instance
    pub fn db(&self) -> &Arc<dyn Db> {
        &self.db
    }

    /// Get the event bus instance
    pub fn event_bus(&self) -> &Arc<dyn EventBus> {
        &self.event_bus
    }

    /// Get the authentication service instance
    pub fn auth_service(&self) -> &Arc<AuthService> {
        &self.auth_service
    }

    /// Get the server address
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Get server status information
    pub fn status(&self) -> ServerStatus {
        ServerStatus {
            host: self.host.clone(),
            port: self.port,
            address: self.address(),
        }
    }
}

/// Server status information
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStatus {
    pub host: String,
    pub port: u16,
    pub address: String,
}

/// Create an Axum app with the given state
///
/// This function creates the complete Axum application with all routes
/// and middleware configured. It's useful for testing or when you need
/// more control over the server lifecycle.
pub fn create_app(state: AppState) -> axum::Router {
    build_router_with_config(RouteConfig::default()).with_state(state)
}

/// Create an Axum app with custom configuration
pub fn create_app_with_config(state: AppState, config: RouteConfig) -> axum::Router {
    build_router_with_config(config).with_state(state)
}

async fn shutdown_signal() {
    match tokio::signal::ctrl_c().await {
        Ok(()) => info!("Shutdown signal received"),
        Err(e) => warn!("Failed to listen for shutdown signal: {}", e),
    }
}


