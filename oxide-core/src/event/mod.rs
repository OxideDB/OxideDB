//! Event System for OxideDB
//!
//! This module provides a comprehensive, production-ready event system that enables
//! the hook-first architecture. The system is designed for:
//!
//! - High performance with async operations
//! - Observable and debuggable with comprehensive metrics
//! - Maintainable with clear separation of concerns
//! - Extensible for future features like persistence and distributed events
//! - Resilient with error handling and recovery mechanisms
//!
//! ## Architecture
//!
//! The event system separates Before and After events:
//! - **Before events**: Allow modification of data before processing
//! - **After events**: Read-only notifications after processing
//!
//! ## Key Features
//!
//! - **Handler Middleware**: Retry, timeout, and circuit breaker patterns
//! - **Event Filtering**: Route events based on collection, data, or custom criteria
//! - **Metrics & Observability**: Comprehensive tracking of event performance
//! - **Configuration**: Flexible configuration for different environments
//! - **Error Recovery**: Graceful degradation when handlers fail
//!
//! ## Usage
//!
//! ```no_run
//! use oxide_core::event::{EventBus, InMemoryEventBus, BeforeEventType, BeforeEventContext};
//!
//! # async fn example() -> Result<(), oxide_core::AppError> {
//! let bus = InMemoryEventBus::new();
//! let mut context = BeforeEventContext::new_create("users".into(), serde_json::json!({}));
//! bus.dispatch_before(BeforeEventType::RecordCreate, &mut context).await?;
//! # Ok(())
//! # }
//! ```

pub mod bus;
pub mod config;
pub mod context;
pub mod handlers;
pub mod memory;
pub mod metrics;
pub mod middleware;
pub mod types;

// Re-export main public API - specific exports to avoid conflicts
pub use bus::{EventBus, EventBusExt, EventBusHealth};
pub use config::{EventBusConfig as BusConfig, EventSystemConfig};
pub use context::{AfterEventContext, BeforeEventContext, ErrorSeverity, RequestContext};
pub use handlers::{
    AfterEventHandler, BeforeEventHandler, CollectionFilter, CompositeFilter, EventFilter,
    HandlerExecutionResult, HandlerMetadata, ManagedAfterHandler, ManagedBeforeHandler, TagFilter,
};
pub use memory::InMemoryEventBus;
pub use metrics::{EventMetrics, EventMetricsCollector, EventTypeMetrics, HandlerMetrics};
pub use middleware::{
    AfterHandlerMiddleware, BeforeHandlerMiddleware, CircuitBreakerMiddleware,
    CompositeAfterMiddleware, CompositeBeforeMiddleware, RetryMiddleware, TimeoutMiddleware,
};
pub use types::{AfterEventType, BeforeEventType, EventCategory, EventPriority};
