//! HTTP server implementation
//!
//! This module contains the core HTTP server implementation for the OxideDB API.
//! The server is built on top of Axum and provides a REST API interface.

use oxide_core::{event::EventBus, AppError};
use oxide_db::Db;
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::info;

use crate::routes::{build_router_with_config, RouteConfig};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Db>,
    pub event_bus: Arc<dyn EventBus>,
}

/// The API server that handles HTTP requests
///
/// This server provides a REST API for interacting with the OxideDB database.
/// It integrates with the event system and database abstraction layer to
/// provide a complete HTTP interface.
pub struct ApiServer {
    db: Arc<dyn Db>,
    event_bus: Arc<dyn EventBus>,
    host: String,
    port: u16,
}

impl ApiServer {
    /// Create a new API server instance
    ///
    /// # Arguments
    /// * `db` - The database implementation to use
    /// * `event_bus` - The event bus for dispatching events
    /// * `host` - The host address to bind to
    /// * `port` - The port to listen on
    pub fn new(db: Arc<dyn Db>, event_bus: Arc<dyn EventBus>, host: String, port: u16) -> Self {
        Self {
            db,
            event_bus,
            host,
            port,
        }
    }

    /// Start the API server
    ///
    /// This method starts the HTTP server and begins listening for requests.
    /// It will run indefinitely until stopped.
    pub async fn start(&self) -> Result<(), AppError> {
        self.start_with_config(RouteConfig::default()).await
    }

    /// Start the API server with custom configuration
    ///
    /// This allows for different configurations in different environments
    /// (e.g., disabling admin UI in production).
    pub async fn start_with_config(&self, config: RouteConfig) -> Result<(), AppError> {
        info!("Starting API server on {}:{}", self.host, self.port);

        let state = AppState {
            db: Arc::clone(&self.db),
            event_bus: Arc::clone(&self.event_bus),
        };

        let app = build_router_with_config(config).with_state(state);

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

    /// Get the server address
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}


