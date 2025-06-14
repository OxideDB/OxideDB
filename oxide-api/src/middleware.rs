//! HTTP middleware for the API server
//!
//! This module contains middleware functions that process HTTP requests
//! and responses. Middleware handles cross-cutting concerns like logging,
//! authentication, CORS, rate limiting, etc.
//!
//! ## Available Middleware
//!
//! - [`LoggingMiddleware`] - Request/response logging with event dispatch
//! - [`RequestIdMiddleware`] - Request ID generation and tracking
//! - [`TimingMiddleware`] - Request timing and performance metrics

use oxide_core::{BeforeEventContext, BeforeEventType, EventBus};
use std::sync::Arc;
use tracing::{debug, info};

use crate::errors::ApiError;

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
    ) -> Result<(), ApiError> {
        debug!("Processing {} {}", method, path);

        // Create context for BeforeApiRequest event
        let mut context = BeforeEventContext {
            collection: "api".to_string(),
            data: serde_json::json!({
                "method": method,
                "path": path,
                "headers": headers
            }),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };

        // Dispatch BeforeApiRequest event
        self.event_bus
            .dispatch_before(BeforeEventType::ApiRequest, &mut context)
            .await?;

        info!("Incoming request: {} {}", 
            context.data.get("method").and_then(|m| m.as_str()).unwrap_or("UNKNOWN"),
            context.data.get("path").and_then(|p| p.as_str()).unwrap_or("/")
        );
        Ok(())
    }
}
