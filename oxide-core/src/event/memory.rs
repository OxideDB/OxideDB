//! Production-Ready In-Memory Event Bus Implementation
//!
//! This module provides a comprehensive, production-ready implementation of
//! the EventBus trait with features like metrics, filtering, middleware,
//! error recovery, and performance optimizations.

use crate::AppError;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock as AsyncRwLock, Semaphore};
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::bus::{EventBus, EventBusConfig, EventBusHealth};
use super::context::{AfterEventContext, BeforeEventContext};
use super::handlers::{
    AfterEventHandler, BeforeEventHandler, EventFilter, HandlerExecutionResult, HandlerMetadata,
    ManagedAfterHandler, ManagedBeforeHandler,
};
use super::metrics::{EventMetrics, EventMetricsCollector};
use super::middleware::{
    AfterHandlerMiddleware, BeforeHandlerMiddleware, CircuitBreakerMiddleware,
    CompositeAfterMiddleware, CompositeBeforeMiddleware, RetryMiddleware, TimeoutMiddleware,
};
use super::types::{AfterEventType, BeforeEventType};

/// Production-ready in-memory implementation of EventBus
pub struct InMemoryEventBus {
    /// Configuration for the event bus
    config: EventBusConfig,
    /// Before event handlers organized by event name
    before_handlers: Arc<AsyncRwLock<HashMap<String, Vec<ManagedBeforeHandler>>>>,
    /// After event handlers organized by event name
    after_handlers: Arc<AsyncRwLock<HashMap<String, Vec<ManagedAfterHandler>>>>,
    /// Event filters that apply to all events
    filters: Arc<AsyncRwLock<HashMap<String, Box<dyn EventFilter>>>>,
    /// Metrics collector
    metrics: Arc<EventMetricsCollector>,
    /// Middleware for Before handlers
    before_middleware: Arc<CompositeBeforeMiddleware>,
    /// Middleware for After handlers
    after_middleware: Arc<CompositeAfterMiddleware>,
    /// Semaphore to limit concurrent handler executions
    execution_semaphore: Arc<Semaphore>,
    /// Shutdown flag
    shutdown: Arc<std::sync::atomic::AtomicBool>,
    /// Start time for uptime calculation
    start_time: Instant,
}

impl InMemoryEventBus {
    /// Create a new InMemoryEventBus with default configuration
    pub fn new() -> Self {
        Self::with_config(EventBusConfig::default())
    }

    /// Create a new InMemoryEventBus with custom configuration
    pub fn with_config(config: EventBusConfig) -> Self {
        let execution_semaphore = Arc::new(Semaphore::new(config.max_concurrent_handlers));

        // Setup middleware based on configuration
        let before_middleware = Arc::new(Self::create_before_middleware(&config));
        let after_middleware = Arc::new(Self::create_after_middleware(&config));

        Self {
            config,
            before_handlers: Arc::new(AsyncRwLock::new(HashMap::new())),
            after_handlers: Arc::new(AsyncRwLock::new(HashMap::new())),
            filters: Arc::new(AsyncRwLock::new(HashMap::new())),
            metrics: Arc::new(EventMetricsCollector::new()),
            before_middleware,
            after_middleware,
            execution_semaphore,
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            start_time: Instant::now(),
        }
    }

    /// Create a development-optimized event bus
    pub fn development() -> Self {
        Self::with_config(EventBusConfig::development())
    }

    /// Create a production-optimized event bus
    pub fn production() -> Self {
        Self::with_config(EventBusConfig::production())
    }

    /// Create a testing-optimized event bus
    pub fn testing() -> Self {
        Self::with_config(EventBusConfig::testing())
    }

    fn create_before_middleware(config: &EventBusConfig) -> CompositeBeforeMiddleware {
        let mut middleware = CompositeBeforeMiddleware::new();

        // Add timeout middleware
        middleware = middleware.with_middleware(Box::new(TimeoutMiddleware::new(
            Duration::from_millis(config.default_handler_timeout_ms),
        )));

        // Add retry middleware if retries are enabled
        if config.max_handler_retries > 0 {
            middleware = middleware.with_middleware(Box::new(
                RetryMiddleware::exponential_backoff(config.max_handler_retries),
            ));
        }

        // Add circuit breaker in production environments
        if config.max_concurrent_handlers > 50 {
            middleware = middleware.with_middleware(Box::new(CircuitBreakerMiddleware::default()));
        }

        middleware
    }

    fn create_after_middleware(config: &EventBusConfig) -> CompositeAfterMiddleware {
        let mut middleware = CompositeAfterMiddleware::new();

        // Add timeout middleware
        middleware = middleware.with_middleware(Box::new(TimeoutMiddleware::new(
            Duration::from_millis(config.default_handler_timeout_ms),
        )));

        // Add retry middleware if retries are enabled
        if config.max_handler_retries > 0 {
            middleware = middleware.with_middleware(Box::new(
                RetryMiddleware::exponential_backoff(config.max_handler_retries),
            ));
        }

        middleware
    }

    async fn execute_before_handlers(
        &self,
        event_type: &BeforeEventType,
        context: &mut BeforeEventContext,
        handlers: Vec<ManagedBeforeHandler>,
    ) -> Result<Vec<HandlerExecutionResult>, AppError> {
        let mut results = Vec::new();
        let semaphore = &self.execution_semaphore;

        for handler in handlers {
            if self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            // Check if handler is enabled
            if !handler.metadata.enabled {
                results.push(HandlerExecutionResult::skipped(handler.metadata.id.clone()));
                continue;
            }

            // Apply filters
            if self.config.enable_filtering {
                let filters = self.filters.read().await;
                let should_execute = filters
                    .values()
                    .all(|filter| filter.should_execute_before(&handler.metadata, context));

                if !should_execute {
                    results.push(HandlerExecutionResult::skipped(handler.metadata.id.clone()));
                    continue;
                }
            }

            // Acquire semaphore permit for concurrency control
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|_| AppError::internal("Failed to acquire execution permit"))?;

            let start_time = Instant::now();
            let handler_id = handler.metadata.id.clone();
            let handler_name = handler.metadata.name.clone();

            // Apply middleware and execute handler
            let wrapped_handler = self
                .before_middleware
                .wrap(handler.handler.clone(), handler_id.clone());

            match wrapped_handler(context).await {
                Ok(()) => {
                    let execution_time = start_time.elapsed().as_millis() as f64;

                    results.push(HandlerExecutionResult::success(
                        handler_id.clone(),
                        execution_time as u64,
                    ));

                    self.metrics.record_handler_execution(
                        &handler_id,
                        &handler_name,
                        execution_time,
                        true,
                        false,
                        0,
                    );

                    debug!(
                        "Handler {} executed successfully in {:.2}ms",
                        handler_id, execution_time
                    );
                }
                Err(err) => {
                    let execution_time = start_time.elapsed().as_millis() as f64;

                    results.push(HandlerExecutionResult::failure(
                        handler_id.clone(),
                        execution_time as u64,
                        err.to_string(),
                    ));

                    self.metrics.record_handler_execution(
                        &handler_id,
                        &handler_name,
                        execution_time,
                        false,
                        false,
                        0,
                    );

                    self.metrics.record_error(
                        "handler_execution_error".to_string(),
                        err.to_string(),
                        Some(handler_id.clone()),
                        Some(event_type.name().to_string()),
                    );

                    warn!(
                        "Handler {} failed after {:.2}ms: {}",
                        handler_id, execution_time, err
                    );

                    // Check if we should continue on failure
                    if !self.config.continue_on_handler_failure {
                        return Err(err);
                    }
                }
            }
        }

        Ok(results)
    }

    async fn execute_after_handlers(
        &self,
        event_type: &AfterEventType,
        context: &AfterEventContext,
        handlers: Vec<ManagedAfterHandler>,
    ) -> Result<Vec<HandlerExecutionResult>, AppError> {
        let mut results = Vec::new();
        let semaphore = &self.execution_semaphore;

        for handler in handlers {
            if self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            // Check if handler is enabled
            if !handler.metadata.enabled {
                results.push(HandlerExecutionResult::skipped(handler.metadata.id.clone()));
                continue;
            }

            // Apply filters
            if self.config.enable_filtering {
                let filters = self.filters.read().await;
                let should_execute = filters
                    .values()
                    .all(|filter| filter.should_execute_after(&handler.metadata, context));

                if !should_execute {
                    results.push(HandlerExecutionResult::skipped(handler.metadata.id.clone()));
                    continue;
                }
            }

            // Acquire semaphore permit for concurrency control
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|_| AppError::internal("Failed to acquire execution permit"))?;

            let start_time = Instant::now();
            let handler_id = handler.metadata.id.clone();
            let handler_name = handler.metadata.name.clone();

            // Apply middleware and execute handler
            let wrapped_handler = self
                .after_middleware
                .wrap(handler.handler.clone(), handler_id.clone());

            match wrapped_handler(context).await {
                Ok(()) => {
                    let execution_time = start_time.elapsed().as_millis() as f64;

                    results.push(HandlerExecutionResult::success(
                        handler_id.clone(),
                        execution_time as u64,
                    ));

                    self.metrics.record_handler_execution(
                        &handler_id,
                        &handler_name,
                        execution_time,
                        true,
                        false,
                        0,
                    );

                    debug!(
                        "Handler {} executed successfully in {:.2}ms",
                        handler_id, execution_time
                    );
                }
                Err(err) => {
                    let execution_time = start_time.elapsed().as_millis() as f64;

                    results.push(HandlerExecutionResult::failure(
                        handler_id.clone(),
                        execution_time as u64,
                        err.to_string(),
                    ));

                    self.metrics.record_handler_execution(
                        &handler_id,
                        &handler_name,
                        execution_time,
                        false,
                        false,
                        0,
                    );

                    self.metrics.record_error(
                        "handler_execution_error".to_string(),
                        err.to_string(),
                        Some(handler_id.clone()),
                        Some(event_type.name().to_string()),
                    );

                    warn!(
                        "Handler {} failed after {:.2}ms: {}",
                        handler_id, execution_time, err
                    );

                    // Check if we should continue on failure
                    if !self.config.continue_on_handler_failure {
                        return Err(err);
                    }
                }
            }
        }

        Ok(results)
    }

    /// Get the current configuration
    pub fn config(&self) -> &EventBusConfig {
        &self.config
    }

    /// Check if the event bus is shutting down
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventBus for InMemoryEventBus {
    async fn dispatch_before(
        &self,
        event_type: BeforeEventType,
        context: &mut BeforeEventContext,
    ) -> Result<Vec<HandlerExecutionResult>, AppError> {
        if self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(AppError::internal("Event bus is shutting down"));
        }

        let event_name = event_type.name();
        let start_time = Instant::now();

        debug!(
            "Dispatching Before event: {} for collection: {}",
            event_name, context.collection
        );

        // Get handlers (clone them to avoid holding the lock during execution)
        let handlers = {
            let handlers_map = self.before_handlers.read().await;
            handlers_map.get(event_name).cloned().unwrap_or_default()
        };

        if handlers.is_empty() {
            debug!("No handlers registered for event: {}", event_name);
            return Ok(Vec::new());
        }

        // Handlers are kept pre-sorted by priority at subscription time, so we
        // execute the cloned snapshot directly without re-sorting on the hot path.
        let results = self
            .execute_before_handlers(&event_type, context, handlers)
            .await?;

        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as f64;
        let success = results.iter().all(|r| r.success || r.skipped);
        let skipped = results.iter().all(|r| r.skipped);

        self.metrics
            .record_before_event(&event_type, execution_time, success, skipped);

        // Update system metrics
        let active_handlers =
            self.before_listener_count(event_name) + self.after_listener_count(event_name);
        self.metrics
            .update_system_metrics(active_handlers as u64, results.len() as u64);

        Ok(results)
    }

    async fn dispatch_after(
        &self,
        event_type: AfterEventType,
        context: &AfterEventContext,
    ) -> Result<Vec<HandlerExecutionResult>, AppError> {
        if self.shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(AppError::internal("Event bus is shutting down"));
        }

        let event_name = event_type.name();
        let start_time = Instant::now();

        debug!("Dispatching After event: {}", event_name);

        // Get handlers (clone them to avoid holding the lock during execution)
        let handlers = {
            let handlers_map = self.after_handlers.read().await;
            handlers_map.get(event_name).cloned().unwrap_or_default()
        };

        if handlers.is_empty() {
            debug!("No handlers registered for event: {}", event_name);
            return Ok(Vec::new());
        }

        // Handlers are kept pre-sorted by priority at subscription time, so we
        // execute the cloned snapshot directly without re-sorting on the hot path.
        let results = self
            .execute_after_handlers(&event_type, context, handlers)
            .await?;

        // Record metrics
        let execution_time = start_time.elapsed().as_millis() as f64;
        let success = results.iter().all(|r| r.success || r.skipped);
        let skipped = results.iter().all(|r| r.skipped);

        self.metrics
            .record_after_event(&event_type, execution_time, success, skipped);

        // Update system metrics
        let active_handlers =
            self.before_listener_count(event_name) + self.after_listener_count(event_name);
        self.metrics
            .update_system_metrics(active_handlers as u64, results.len() as u64);

        Ok(results)
    }

    async fn subscribe_before(
        &self,
        event_name: &str,
        handler: BeforeEventHandler,
        metadata: HandlerMetadata,
    ) -> Result<String, AppError> {
        let handler_id = metadata.id.clone();
        let managed_handler = ManagedBeforeHandler { metadata, handler };

        let mut handlers_map = self.before_handlers.write().await;
        let handlers = handlers_map
            .entry(event_name.to_string())
            .or_insert_with(Vec::new);

        // Check for duplicate IDs if not allowed
        if !self.config.continue_on_handler_failure
            && handlers.iter().any(|h| h.metadata.id == handler_id)
        {
            return Err(AppError::internal(format!(
                "Handler with ID {} already exists for event {}",
                handler_id, event_name
            )));
        }

        handlers.push(managed_handler);

        // Keep the stored handler list sorted by priority (desc) at insertion
        // time so dispatch can skip the per-event re-sort on the hot path.
        // Rust's sort_by is stable, so equal-priority handlers keep their
        // insertion order — matching the previous per-dispatch behavior.
        handlers.sort_by_key(|handler| std::cmp::Reverse(handler.metadata.priority));

        info!(
            "Subscribed Before handler {} to event: {}",
            handler_id, event_name
        );
        Ok(handler_id)
    }

    async fn subscribe_after(
        &self,
        event_name: &str,
        handler: AfterEventHandler,
        metadata: HandlerMetadata,
    ) -> Result<String, AppError> {
        let handler_id = metadata.id.clone();
        let managed_handler = ManagedAfterHandler { metadata, handler };

        let mut handlers_map = self.after_handlers.write().await;
        let handlers = handlers_map
            .entry(event_name.to_string())
            .or_insert_with(Vec::new);

        // Check for duplicate IDs if not allowed
        if !self.config.continue_on_handler_failure
            && handlers.iter().any(|h| h.metadata.id == handler_id)
        {
            return Err(AppError::internal(format!(
                "Handler with ID {} already exists for event {}",
                handler_id, event_name
            )));
        }

        handlers.push(managed_handler);

        // Keep the stored handler list sorted by priority (desc) at insertion
        // time so dispatch can skip the per-event re-sort on the hot path.
        handlers.sort_by_key(|handler| std::cmp::Reverse(handler.metadata.priority));

        info!(
            "Subscribed After handler {} to event: {}",
            handler_id, event_name
        );
        Ok(handler_id)
    }

    async fn unsubscribe_before(
        &self,
        event_name: &str,
        handler_id: &str,
    ) -> Result<bool, AppError> {
        let mut handlers_map = self.before_handlers.write().await;

        if let Some(handlers) = handlers_map.get_mut(event_name) {
            let initial_len = handlers.len();
            handlers.retain(|h| h.metadata.id != handler_id);

            let removed = handlers.len() < initial_len;
            if removed {
                info!(
                    "Unsubscribed Before handler {} from event: {}",
                    handler_id, event_name
                );
            }

            // Remove empty event entries
            if handlers.is_empty() {
                handlers_map.remove(event_name);
            }

            Ok(removed)
        } else {
            Ok(false)
        }
    }

    async fn unsubscribe_after(
        &self,
        event_name: &str,
        handler_id: &str,
    ) -> Result<bool, AppError> {
        let mut handlers_map = self.after_handlers.write().await;

        if let Some(handlers) = handlers_map.get_mut(event_name) {
            let initial_len = handlers.len();
            handlers.retain(|h| h.metadata.id != handler_id);

            let removed = handlers.len() < initial_len;
            if removed {
                info!(
                    "Unsubscribed After handler {} from event: {}",
                    handler_id, event_name
                );
            }

            // Remove empty event entries
            if handlers.is_empty() {
                handlers_map.remove(event_name);
            }

            Ok(removed)
        } else {
            Ok(false)
        }
    }

    async fn set_handler_enabled(&self, handler_id: &str, enabled: bool) -> Result<bool, AppError> {
        // Check Before handlers
        {
            let mut handlers_map = self.before_handlers.write().await;
            for handlers in handlers_map.values_mut() {
                for handler in handlers.iter_mut() {
                    if handler.metadata.id == handler_id {
                        handler.metadata.enabled = enabled;
                        info!("Set Before handler {} enabled: {}", handler_id, enabled);
                        return Ok(true);
                    }
                }
            }
        }

        // Check After handlers
        {
            let mut handlers_map = self.after_handlers.write().await;
            for handlers in handlers_map.values_mut() {
                for handler in handlers.iter_mut() {
                    if handler.metadata.id == handler_id {
                        handler.metadata.enabled = enabled;
                        info!("Set After handler {} enabled: {}", handler_id, enabled);
                        return Ok(true);
                    }
                }
            }
        }

        Ok(false)
    }

    fn before_listener_count(&self, event_name: &str) -> usize {
        // Use blocking version since this is a sync method
        if let Ok(handlers_map) = self.before_handlers.try_read() {
            handlers_map
                .get(event_name)
                .map(|handlers| handlers.len())
                .unwrap_or(0)
        } else {
            0
        }
    }

    fn after_listener_count(&self, event_name: &str) -> usize {
        // Use blocking version since this is a sync method
        if let Ok(handlers_map) = self.after_handlers.try_read() {
            handlers_map
                .get(event_name)
                .map(|handlers| handlers.len())
                .unwrap_or(0)
        } else {
            0
        }
    }

    fn list_handlers(&self) -> HashMap<String, Vec<HandlerMetadata>> {
        let mut result = HashMap::new();

        // Add Before handlers
        if let Ok(before_handlers) = self.before_handlers.try_read() {
            for (event_name, handlers) in before_handlers.iter() {
                let metadata: Vec<HandlerMetadata> =
                    handlers.iter().map(|h| h.metadata.clone()).collect();
                result.insert(format!("before:{}", event_name), metadata);
            }
        }

        // Add After handlers
        if let Ok(after_handlers) = self.after_handlers.try_read() {
            for (event_name, handlers) in after_handlers.iter() {
                let metadata: Vec<HandlerMetadata> =
                    handlers.iter().map(|h| h.metadata.clone()).collect();
                result.insert(format!("after:{}", event_name), metadata);
            }
        }

        result
    }

    fn metrics(&self) -> EventMetrics {
        self.metrics.snapshot()
    }

    async fn add_filter(&self, filter: Box<dyn EventFilter>) -> Result<String, AppError> {
        let filter_id = Uuid::new_v4().to_string();
        let mut filters = self.filters.write().await;
        filters.insert(filter_id.clone(), filter);

        info!("Added event filter: {}", filter_id);
        Ok(filter_id)
    }

    async fn remove_filter(&self, filter_id: &str) -> Result<bool, AppError> {
        let mut filters = self.filters.write().await;
        let removed = filters.remove(filter_id).is_some();

        if removed {
            info!("Removed event filter: {}", filter_id);
        }

        Ok(removed)
    }

    fn health_status(&self) -> EventBusHealth {
        let metrics = self.metrics.snapshot();
        let uptime_ms = self.start_time.elapsed().as_millis() as u64;

        let total_handlers = self
            .list_handlers()
            .values()
            .map(|v| v.len())
            .sum::<usize>();
        let enabled_handlers = self
            .list_handlers()
            .values()
            .flatten()
            .filter(|h| h.enabled)
            .count();
        let disabled_handlers = total_handlers - enabled_handlers;

        let mut health = EventBusHealth {
            healthy: true,
            total_handlers,
            enabled_handlers,
            disabled_handlers,
            avg_processing_time_ms: metrics.avg_processing_time_ms(),
            recent_failures: metrics.errors.total_errors,
            memory_usage_bytes: None, // Could be implemented with a memory profiler
            last_check_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            diagnostics: HashMap::new(),
        };

        // Add diagnostic information
        health
            .diagnostics
            .insert("uptime_ms".to_string(), uptime_ms.to_string());
        health.diagnostics.insert(
            "total_events_processed".to_string(),
            metrics.total_events().to_string(),
        );
        health.diagnostics.insert(
            "success_rate_percent".to_string(),
            format!("{:.2}", metrics.success_rate()),
        );
        health.diagnostics.insert(
            "is_shutting_down".to_string(),
            self.is_shutting_down().to_string(),
        );

        // Check if healthy based on metrics
        health.healthy = health.is_healthy_by_metrics() && !self.is_shutting_down();

        if !health.healthy {
            if self.is_shutting_down() {
                health.diagnostics.insert(
                    "unhealthy_reason".to_string(),
                    "Event bus is shutting down".to_string(),
                );
            } else {
                health.diagnostics.insert(
                    "unhealthy_reason".to_string(),
                    "Poor performance metrics".to_string(),
                );
            }
        }

        health
    }

    async fn shutdown(&self) -> Result<(), AppError> {
        info!("Shutting down event bus...");

        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);

        // Wait for existing operations to complete with timeout
        let shutdown_timeout = Duration::from_millis(self.config.shutdown_timeout_ms);
        let start = Instant::now();

        while self.execution_semaphore.available_permits() < self.config.max_concurrent_handlers {
            if start.elapsed() > shutdown_timeout {
                warn!("Shutdown timeout reached, forcing shutdown");
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        info!("Event bus shutdown complete");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_before_event_execution() {
        let bus = InMemoryEventBus::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        // Subscribe a handler
        let handler: BeforeEventHandler = Arc::new(move |context: &mut BeforeEventContext| {
            let counter = call_count_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                // Modify the data
                if let Some(obj) = context.data.as_object_mut() {
                    obj.insert("modified_by_handler".to_string(), serde_json::json!(true));
                }
                Ok(())
            }) as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let metadata = HandlerMetadata::new("test-handler".to_string(), "Test Handler".to_string());
        bus.subscribe_before("BeforeRecordCreate", handler, metadata)
            .await
            .unwrap();

        // Dispatch an event
        let mut context = BeforeEventContext::new_create(
            "users".to_string(),
            serde_json::json!({"name": "John"}),
        );

        let results = bus
            .dispatch_before(BeforeEventType::RecordCreate, &mut context)
            .await
            .unwrap();

        // Verify handler was called and data was modified
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(context.data.get("modified_by_handler").is_some());
    }

    #[tokio::test]
    async fn test_after_event_execution() {
        let bus = InMemoryEventBus::new();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        // Subscribe a handler
        let handler: AfterEventHandler = Arc::new(move |_context: &AfterEventContext| {
            let counter = call_count_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }) as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let metadata = HandlerMetadata::new("test-handler".to_string(), "Test Handler".to_string());
        bus.subscribe_after("AfterRecordCreate", handler, metadata)
            .await
            .unwrap();

        // Dispatch an event
        let context = AfterEventContext::record_created(
            "users".to_string(),
            "123".to_string(),
            serde_json::json!({"name": "John"}),
            Default::default(),
        );

        let results = bus
            .dispatch_after(AfterEventType::RecordCreated, &context)
            .await
            .unwrap();

        // Verify handler was called
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[tokio::test]
    async fn test_handler_priority_ordering() {
        let bus = InMemoryEventBus::new();
        let execution_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        // Subscribe handlers with different priorities
        for (priority, name) in [(100, "high"), (0, "normal"), (50, "medium")] {
            let order_clone = execution_order.clone();
            let name_clone = name.to_string();

            let handler: BeforeEventHandler = Arc::new(move |_context: &mut BeforeEventContext| {
                let order = order_clone.clone();
                let name = name_clone.clone();
                Box::pin(async move {
                    order.lock().unwrap().push(name);
                    Ok(())
                })
                    as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
            });

            let metadata = HandlerMetadata::new(
                format!("{}-handler", name),
                format!("{} Priority Handler", name),
            )
            .with_priority(priority);

            bus.subscribe_before("BeforeRecordCreate", handler, metadata)
                .await
                .unwrap();
        }

        // Dispatch event
        let mut context =
            BeforeEventContext::new_create("users".to_string(), serde_json::json!({}));

        bus.dispatch_before(BeforeEventType::RecordCreate, &mut context)
            .await
            .unwrap();

        // Verify execution order (highest priority first)
        let order = execution_order.lock().unwrap();
        assert_eq!(*order, vec!["high", "medium", "normal"]);
    }

    #[tokio::test]
    async fn test_handler_filtering() {
        use super::super::handlers::CollectionFilter;

        let bus = InMemoryEventBus::with_config(EventBusConfig {
            enable_filtering: true,
            ..EventBusConfig::testing()
        });

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        // Subscribe a handler
        let handler: BeforeEventHandler = Arc::new(move |_context: &mut BeforeEventContext| {
            let counter = call_count_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }) as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let metadata = HandlerMetadata::new("test-handler".to_string(), "Test Handler".to_string());
        bus.subscribe_before("BeforeRecordCreate", handler, metadata)
            .await
            .unwrap();

        // Add a filter that only allows "users" collection
        let filter = Box::new(CollectionFilter::allow(vec!["users".to_string()]));
        bus.add_filter(filter).await.unwrap();

        // Test with allowed collection
        let mut context_users =
            BeforeEventContext::new_create("users".to_string(), serde_json::json!({}));
        bus.dispatch_before(BeforeEventType::RecordCreate, &mut context_users)
            .await
            .unwrap();

        // Test with disallowed collection
        let mut context_admin =
            BeforeEventContext::new_create("admin".to_string(), serde_json::json!({}));
        bus.dispatch_before(BeforeEventType::RecordCreate, &mut context_admin)
            .await
            .unwrap();

        // Should only have been called once (for users collection)
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let bus = InMemoryEventBus::new();

        // Subscribe a handler
        let handler: BeforeEventHandler = Arc::new(|_context: &mut BeforeEventContext| {
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                Ok(())
            }) as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let metadata = HandlerMetadata::new("test-handler".to_string(), "Test Handler".to_string());
        bus.subscribe_before("BeforeRecordCreate", handler, metadata)
            .await
            .unwrap();

        // Dispatch events
        for _ in 0..3 {
            let mut context =
                BeforeEventContext::new_create("users".to_string(), serde_json::json!({}));
            bus.dispatch_before(BeforeEventType::RecordCreate, &mut context)
                .await
                .unwrap();
        }

        // Check metrics
        let metrics = bus.metrics();
        assert_eq!(metrics.total_events(), 3);
        assert!(metrics.avg_processing_time_ms() > 0.0);
        assert_eq!(metrics.success_rate(), 100.0);
    }

    #[tokio::test]
    async fn test_graceful_shutdown() {
        let bus = InMemoryEventBus::new();

        // Start a long-running handler
        let handler: BeforeEventHandler = Arc::new(|_context: &mut BeforeEventContext| {
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                Ok(())
            }) as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let metadata = HandlerMetadata::new("test-handler".to_string(), "Test Handler".to_string());
        bus.subscribe_before("BeforeRecordCreate", handler, metadata)
            .await
            .unwrap();

        // Start event processing
        let bus_clone = Arc::new(bus);
        let bus_task = bus_clone.clone();
        let _handle = tokio::spawn(async move {
            let mut context =
                BeforeEventContext::new_create("users".to_string(), serde_json::json!({}));
            let _ = bus_task
                .dispatch_before(BeforeEventType::RecordCreate, &mut context)
                .await;
        });

        // Wait a bit then shutdown
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let result = bus_clone.shutdown().await;
        assert!(result.is_ok());
        assert!(bus_clone.is_shutting_down());
    }

    #[tokio::test]
    async fn test_health_status() {
        let bus = InMemoryEventBus::new();

        let health = bus.health_status();
        assert!(health.healthy);
        assert_eq!(health.total_handlers, 0);
        assert!(health.diagnostics.contains_key("uptime_ms"));
        assert!(health.diagnostics.contains_key("total_events_processed"));
    }
}
