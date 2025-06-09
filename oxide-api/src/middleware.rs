//! HTTP middleware for the API server
//!
//! This module contains middleware functions that process HTTP requests
//! and responses. Middleware handles cross-cutting concerns like logging,
//! authentication, CORS, etc.

use oxide_core::{
    event::{Event, EventBus},
    AppError,
};
use std::sync::Arc;
use tracing::{debug, info};

/// Logging middleware that dispatches API events
pub struct LoggingMiddleware {
    event_bus: Arc<dyn EventBus>,
}

impl LoggingMiddleware {
    /// Create a new logging middleware instance
    pub fn new(event_bus: Arc<dyn EventBus>) -> Self {
        Self { event_bus }
    }

    /// Process a request and log it
    pub async fn process_request(
        &self,
        method: String,
        path: String,
        headers: serde_json::Value,
    ) -> Result<(), AppError> {
        debug!("Processing {} {}", method, path);

        // Dispatch BeforeApiRequest event
        self.event_bus
            .dispatch(Event::BeforeApiRequest {
                method: method.clone(),
                path: path.clone(),
                headers,
            })
            .await?;

        info!("Incoming request: {} {}", method, path);
        Ok(())
    }
}
