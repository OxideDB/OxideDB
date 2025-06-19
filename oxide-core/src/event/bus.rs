//! Core EventBus Interface and Utilities
//!
//! This module defines the main EventBus trait that serves as the contract
//! for all event bus implementations. It includes additional utilities for
//! event introspection and management.

use crate::AppError;
use std::collections::HashMap;

use super::context::{BeforeEventContext, AfterEventContext};
use super::handlers::{BeforeEventHandler, AfterEventHandler, HandlerExecutionResult, HandlerMetadata, EventFilter};
use super::metrics::EventMetrics;
use super::types::{BeforeEventType, AfterEventType};

/// The EventBus trait defines the interface for dispatching and subscribing to events.
///
/// This is the core contract that enables the hook-first architecture. All business
/// logic must dispatch events through an EventBus implementation, and plugins/listeners
/// subscribe to relevant events to extend functionality.
///
/// ## Production Features
///
/// - **Async Operations**: All operations are async for high performance
/// - **Event Filtering**: Support for filtering events based on various criteria
/// - **Metrics & Observability**: Comprehensive metrics collection
/// - **Error Recovery**: Graceful handling of handler failures
/// - **Handler Management**: Registration, unregistration, and introspection
#[async_trait::async_trait]
pub trait EventBus: Send + Sync {
    /// Dispatch a Before event to all registered listeners, allowing data modification
    ///
    /// This method delivers the event to all listeners that have subscribed
    /// to this event type. Handlers can modify the context data.
    ///
    /// # Arguments
    /// * `event_type` - The type of Before event to dispatch
    /// * `context` - Mutable context that handlers can modify
    ///
    /// # Returns
    /// * `Ok(())` if all handlers executed successfully
    /// * `Err(AppError)` if any critical handler failed
    async fn dispatch_before(
        &self,
        event_type: BeforeEventType,
        context: &mut BeforeEventContext,
    ) -> Result<Vec<HandlerExecutionResult>, AppError>;

    /// Dispatch an After event to all registered listeners for read-only notification
    ///
    /// This method delivers the event to all listeners that have subscribed
    /// to this event type. Handlers cannot modify the data.
    ///
    /// # Arguments
    /// * `event_type` - The type of After event to dispatch
    /// * `context` - Read-only context for handlers
    ///
    /// # Returns
    /// * `Ok(())` if all handlers executed successfully
    /// * `Err(AppError)` if any critical handler failed
    async fn dispatch_after(
        &self,
        event_type: AfterEventType,
        context: &AfterEventContext,
    ) -> Result<Vec<HandlerExecutionResult>, AppError>;

    /// Subscribe a handler to Before events (can modify data)
    ///
    /// # Arguments
    /// * `event_name` - The name of the event to subscribe to
    /// * `handler` - The handler function to execute
    /// * `metadata` - Metadata about the handler (priority, timeout, etc.)
    ///
    /// # Returns
    /// * `Ok(handler_id)` - Unique ID for the registered handler
    /// * `Err(AppError)` - If registration failed
    async fn subscribe_before(
        &self,
        event_name: &str,
        handler: BeforeEventHandler,
        metadata: HandlerMetadata,
    ) -> Result<String, AppError>;

    /// Subscribe a handler to After events (read-only)
    ///
    /// # Arguments
    /// * `event_name` - The name of the event to subscribe to
    /// * `handler` - The handler function to execute
    /// * `metadata` - Metadata about the handler
    ///
    /// # Returns
    /// * `Ok(handler_id)` - Unique ID for the registered handler
    /// * `Err(AppError)` - If registration failed
    async fn subscribe_after(
        &self,
        event_name: &str,
        handler: AfterEventHandler,
        metadata: HandlerMetadata,
    ) -> Result<String, AppError>;

    /// Unsubscribe a Before event handler
    ///
    /// # Arguments
    /// * `event_name` - The name of the event
    /// * `handler_id` - The ID of the handler to remove
    ///
    /// # Returns
    /// * `Ok(true)` if handler was found and removed
    /// * `Ok(false)` if handler was not found
    /// * `Err(AppError)` if operation failed
    async fn unsubscribe_before(&self, event_name: &str, handler_id: &str) -> Result<bool, AppError>;

    /// Unsubscribe an After event handler
    ///
    /// # Arguments
    /// * `event_name` - The name of the event
    /// * `handler_id` - The ID of the handler to remove
    ///
    /// # Returns
    /// * `Ok(true)` if handler was found and removed
    /// * `Ok(false)` if handler was not found
    /// * `Err(AppError)` if operation failed
    async fn unsubscribe_after(&self, event_name: &str, handler_id: &str) -> Result<bool, AppError>;

    /// Enable or disable a specific handler
    ///
    /// # Arguments
    /// * `handler_id` - The ID of the handler to enable/disable
    /// * `enabled` - Whether the handler should be enabled
    ///
    /// # Returns
    /// * `Ok(true)` if handler was found and updated
    /// * `Ok(false)` if handler was not found
    /// * `Err(AppError)` if operation failed
    async fn set_handler_enabled(&self, handler_id: &str, enabled: bool) -> Result<bool, AppError>;

    /// Get the number of active Before listeners for an event type
    fn before_listener_count(&self, event_name: &str) -> usize;

    /// Get the number of active After listeners for an event type
    fn after_listener_count(&self, event_name: &str) -> usize;

    /// Get detailed information about all registered handlers
    fn list_handlers(&self) -> HashMap<String, Vec<HandlerMetadata>>;

    /// Get metrics about event processing
    fn metrics(&self) -> EventMetrics;

    /// Add an event filter that will be applied to all events
    async fn add_filter(&self, filter: Box<dyn EventFilter>) -> Result<String, AppError>;

    /// Remove an event filter
    async fn remove_filter(&self, filter_id: &str) -> Result<bool, AppError>;

    /// Get the health status of the event bus
    fn health_status(&self) -> EventBusHealth;

    /// Shutdown the event bus gracefully
    async fn shutdown(&self) -> Result<(), AppError>;
}

/// Health status information for the event bus
#[derive(Debug, Clone)]
pub struct EventBusHealth {
    /// Whether the event bus is healthy
    pub healthy: bool,
    /// Total number of handlers registered
    pub total_handlers: usize,
    /// Number of enabled handlers
    pub enabled_handlers: usize,
    /// Number of disabled handlers
    pub disabled_handlers: usize,
    /// Average event processing time in milliseconds
    pub avg_processing_time_ms: f64,
    /// Number of failed events in the last minute
    pub recent_failures: u64,
    /// Memory usage in bytes (if available)
    pub memory_usage_bytes: Option<u64>,
    /// Last health check timestamp
    pub last_check_timestamp: u64,
    /// Additional diagnostic information
    pub diagnostics: HashMap<String, String>,
}

impl EventBusHealth {
    /// Create a new healthy status
    pub fn healthy() -> Self {
        Self {
            healthy: true,
            total_handlers: 0,
            enabled_handlers: 0,
            disabled_handlers: 0,
            avg_processing_time_ms: 0.0,
            recent_failures: 0,
            memory_usage_bytes: None,
            last_check_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            diagnostics: HashMap::new(),
        }
    }

    /// Create an unhealthy status with reason
    pub fn unhealthy(reason: String) -> Self {
        let mut health = Self::healthy();
        health.healthy = false;
        health.diagnostics.insert("error".to_string(), reason);
        health
    }

    /// Check if the event bus is considered healthy based on metrics
    pub fn is_healthy_by_metrics(&self) -> bool {
        // Consider unhealthy if:
        // - Too many recent failures (>10% of total events)
        // - Average processing time is too high (>5 seconds)
        // - Too many disabled handlers (>50% of total)
        
        if self.recent_failures > 100 {
            return false;
        }

        if self.avg_processing_time_ms > 5000.0 {
            return false;
        }

        if self.total_handlers > 0 && (self.disabled_handlers as f64 / self.total_handlers as f64) > 0.5 {
            return false;
        }

        true
    }
}

/// Event bus configuration for different operational modes
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// Maximum number of concurrent event handlers
    pub max_concurrent_handlers: usize,
    /// Default timeout for event handlers (milliseconds)
    pub default_handler_timeout_ms: u64,
    /// Maximum number of retries for failed handlers
    pub max_handler_retries: u32,
    /// Whether to continue processing if a handler fails
    pub continue_on_handler_failure: bool,
    /// Whether to collect detailed metrics
    pub enable_metrics: bool,
    /// Whether to enable handler filtering
    pub enable_filtering: bool,
    /// Buffer size for event processing
    pub event_buffer_size: usize,
    /// Whether to enable handler middleware (retry, timeout, etc.)
    pub enable_middleware: bool,
    /// Graceful shutdown timeout (milliseconds)
    pub shutdown_timeout_ms: u64,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            max_concurrent_handlers: 100,
            default_handler_timeout_ms: 5000,
            max_handler_retries: 3,
            continue_on_handler_failure: true,
            enable_metrics: true,
            enable_filtering: true,
            event_buffer_size: 1000,
            enable_middleware: true,
            shutdown_timeout_ms: 30000,
        }
    }
}

impl EventBusConfig {
    /// Create a configuration optimized for development
    pub fn development() -> Self {
        Self {
            max_concurrent_handlers: 50,
            default_handler_timeout_ms: 10000, // Longer timeout for debugging
            max_handler_retries: 1,
            continue_on_handler_failure: false, // Fail fast in development
            enable_metrics: true,
            enable_filtering: false,
            event_buffer_size: 100,
            enable_middleware: false,
            shutdown_timeout_ms: 5000,
        }
    }

    /// Create a configuration optimized for production
    pub fn production() -> Self {
        Self {
            max_concurrent_handlers: 200,
            default_handler_timeout_ms: 3000,
            max_handler_retries: 5,
            continue_on_handler_failure: true,
            enable_metrics: true,
            enable_filtering: true,
            event_buffer_size: 2000,
            enable_middleware: true,
            shutdown_timeout_ms: 30000,
        }
    }

    /// Create a configuration optimized for testing
    pub fn testing() -> Self {
        Self {
            max_concurrent_handlers: 10,
            default_handler_timeout_ms: 1000,
            max_handler_retries: 0,
            continue_on_handler_failure: false,
            enable_metrics: false,
            enable_filtering: false,
            event_buffer_size: 10,
            enable_middleware: false,
            shutdown_timeout_ms: 1000,
        }
    }
}

/// Utility trait for event bus implementations to provide common functionality
pub trait EventBusExt: EventBus {
    /// Subscribe to Before events with default metadata
    async fn subscribe_before_simple(
        &self,
        event_name: &str,
        handler: BeforeEventHandler,
    ) -> Result<String, AppError> {
        let metadata = HandlerMetadata::new(
            uuid::Uuid::new_v4().to_string(),
            format!("Handler for {}", event_name),
        );
        self.subscribe_before(event_name, handler, metadata).await
    }

    /// Subscribe to After events with default metadata
    async fn subscribe_after_simple(
        &self,
        event_name: &str,
        handler: AfterEventHandler,
    ) -> Result<String, AppError> {
        let metadata = HandlerMetadata::new(
            uuid::Uuid::new_v4().to_string(),
            format!("Handler for {}", event_name),
        );
        self.subscribe_after(event_name, handler, metadata).await
    }

    /// Dispatch a Before event and return only success/failure
    async fn dispatch_before_simple(
        &self,
        event_type: BeforeEventType,
        context: &mut BeforeEventContext,
    ) -> Result<(), AppError> {
        let results = self.dispatch_before(event_type, context).await?;
        
        // Check if any critical handlers failed
        for result in results {
            if !result.success && !result.skipped {
                return Err(AppError::internal(format!(
                    "Handler {} failed: {}",
                    result.handler_id,
                    result.error.unwrap_or_else(|| "Unknown error".to_string())
                )));
            }
        }
        
        Ok(())
    }

    /// Dispatch an After event and return only success/failure
    async fn dispatch_after_simple(
        &self,
        event_type: AfterEventType,
        context: &AfterEventContext,
    ) -> Result<(), AppError> {
        let results = self.dispatch_after(event_type, context).await?;
        
        // Check if any critical handlers failed
        for result in results {
            if !result.success && !result.skipped {
                return Err(AppError::internal(format!(
                    "Handler {} failed: {}",
                    result.handler_id,
                    result.error.unwrap_or_else(|| "Unknown error".to_string())
                )));
            }
        }
        
        Ok(())
    }
}

// Automatically implement EventBusExt for all EventBus implementations
impl<T: EventBus> EventBusExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_bus_config_presets() {
        let dev_config = EventBusConfig::development();
        assert!(!dev_config.continue_on_handler_failure);
        assert_eq!(dev_config.max_handler_retries, 1);

        let prod_config = EventBusConfig::production();
        assert!(prod_config.continue_on_handler_failure);
        assert_eq!(prod_config.max_handler_retries, 5);

        let test_config = EventBusConfig::testing();
        assert!(!test_config.enable_metrics);
        assert_eq!(test_config.max_handler_retries, 0);
    }

    #[test]
    fn test_event_bus_health() {
        let mut health = EventBusHealth::healthy();
        health.total_handlers = 100;
        health.disabled_handlers = 60; // 60% disabled
        health.avg_processing_time_ms = 6000.0; // 6 seconds

        assert!(!health.is_healthy_by_metrics());

        health.disabled_handlers = 30; // 30% disabled
        health.avg_processing_time_ms = 1000.0; // 1 second
        assert!(health.is_healthy_by_metrics());
    }

    #[test]
    fn test_unhealthy_status() {
        let health = EventBusHealth::unhealthy("Database connection lost".to_string());
        assert!(!health.healthy);
        assert_eq!(health.diagnostics.get("error").unwrap(), "Database connection lost");
    }
} 