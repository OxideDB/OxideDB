//! HTTP server implementation
//!
//! This module contains the core HTTP server implementation for the OxideDB API.
//! The server is built on top of Axum and provides a REST API interface.

use oxide_core::{event::EventBus, AppError, AuthService};
use oxide_db::Db;
use oxide_logging::LogServiceBridge;
use crate::services::LoggingApiService;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, debug};

use crate::routes::{build_router_with_config, build_router_with_config_and_middleware, RouteConfig};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Db>,
    pub event_bus: Arc<dyn EventBus>,
    pub auth_service: Arc<AuthService>,
    pub logging_service: Option<Arc<LogServiceBridge>>,
    pub logging_api_service: Option<Arc<LoggingApiService>>,
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

        let state = AppState {
            db: Arc::clone(&self.db),
            event_bus: Arc::clone(&self.event_bus),
            auth_service: Arc::clone(&self.auth_service),
            logging_service: self.logging_service.clone(),
            logging_api_service: self.logging_api_service.clone(),
        };

        let app = build_router_with_config_and_middleware(config, state);

        let listener = TcpListener::bind(&self.address()).await.map_err(|e| {
            AppError::internal(format!("Failed to bind to {}: {}", self.address(), e))
        })?;

        info!("✅ API server listening on {}", self.address());

        axum::serve(listener, app)
            .await
            .map_err(|e| AppError::internal(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Stop the API server gracefully
    pub async fn stop(&self) -> Result<(), AppError> {
        info!("Stopping API server");

        // TODO: Implement graceful shutdown
        // In a real implementation, this would:
        // 1. Stop accepting new connections
        // 2. Wait for existing requests to complete
        // 3. Close the database connection
        // 4. Shutdown the event bus

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


