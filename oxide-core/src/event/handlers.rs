//! Event Handler Definitions and Utilities
//!
//! This module defines the event handler types and provides utilities
//! for working with event handlers, including middleware support.

use crate::AppError;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::context::{AfterEventContext, BeforeEventContext};

/// Handler for Before events that can modify the context
pub type BeforeEventHandler = Arc<
    dyn Fn(
            &mut BeforeEventContext,
        ) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        + Send
        + Sync,
>;

/// Handler for After events that are read-only
pub type AfterEventHandler = Arc<
    dyn Fn(&AfterEventContext) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        + Send
        + Sync,
>;

/// Metadata about a registered event handler
#[derive(Debug, Clone)]
pub struct HandlerMetadata {
    /// Unique identifier for this handler
    pub id: String,
    /// Human-readable name for debugging
    pub name: String,
    /// Description of what this handler does
    pub description: String,
    /// Priority order for execution (higher numbers execute first)
    pub priority: i32,
    /// Whether this handler is enabled
    pub enabled: bool,
    /// Tags for filtering and organization
    pub tags: std::collections::HashMap<String, String>,
    /// Maximum execution time before timeout (milliseconds)
    pub timeout_ms: Option<u64>,
    /// Maximum number of retries on failure
    pub max_retries: Option<u32>,
}

impl HandlerMetadata {
    /// Create new handler metadata with defaults
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            description: String::new(),
            priority: 0,
            enabled: true,
            tags: std::collections::HashMap::new(),
            timeout_ms: None,
            max_retries: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = description;
        self
    }

    /// Set the priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Set enabled state
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Add a tag
    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(max_retries);
        self
    }
}

/// A Before event handler with metadata
#[derive(Clone)]
pub struct ManagedBeforeHandler {
    pub metadata: HandlerMetadata,
    pub handler: BeforeEventHandler,
}

impl std::fmt::Debug for ManagedBeforeHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedBeforeHandler")
            .field("metadata", &self.metadata)
            .field("handler", &"<function>")
            .finish()
    }
}

/// An After event handler with metadata
#[derive(Clone)]
pub struct ManagedAfterHandler {
    pub metadata: HandlerMetadata,
    pub handler: AfterEventHandler,
}

impl std::fmt::Debug for ManagedAfterHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManagedAfterHandler")
            .field("metadata", &self.metadata)
            .field("handler", &"<function>")
            .finish()
    }
}

/// Result of handler execution with timing and error information
#[derive(Debug, Clone)]
pub struct HandlerExecutionResult {
    /// Handler ID that was executed
    pub handler_id: String,
    /// Whether execution was successful
    pub success: bool,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Error message if execution failed
    pub error: Option<String>,
    /// Number of retry attempts made
    pub retry_attempts: u32,
    /// Whether the handler was skipped due to filtering
    pub skipped: bool,
    /// Additional metadata from middleware
    pub metadata: std::collections::HashMap<String, String>,
}

impl HandlerExecutionResult {
    /// Create a successful execution result
    pub fn success(handler_id: String, execution_time_ms: u64) -> Self {
        Self {
            handler_id,
            success: true,
            execution_time_ms,
            error: None,
            retry_attempts: 0,
            skipped: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a failed execution result
    pub fn failure(handler_id: String, execution_time_ms: u64, error: String) -> Self {
        Self {
            handler_id,
            success: false,
            execution_time_ms,
            error: Some(error),
            retry_attempts: 0,
            skipped: false,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a skipped execution result
    pub fn skipped(handler_id: String) -> Self {
        Self {
            handler_id,
            success: true,
            execution_time_ms: 0,
            error: None,
            retry_attempts: 0,
            skipped: true,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Set the number of retry attempts
    pub fn with_retries(mut self, retry_attempts: u32) -> Self {
        self.retry_attempts = retry_attempts;
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Filter for determining which handlers should execute for an event
pub trait EventFilter: Send + Sync {
    /// Check if a Before handler should execute for the given context
    fn should_execute_before(
        &self,
        handler: &HandlerMetadata,
        context: &BeforeEventContext,
    ) -> bool;

    /// Check if an After handler should execute for the given context
    fn should_execute_after(&self, handler: &HandlerMetadata, context: &AfterEventContext) -> bool;
}

/// Collection-based filter that only executes handlers for specific collections
pub struct CollectionFilter {
    pub allowed_collections: std::collections::HashSet<String>,
    pub denied_collections: std::collections::HashSet<String>,
}

impl CollectionFilter {
    /// Create a new filter that allows specific collections
    pub fn allow(collections: Vec<String>) -> Self {
        Self {
            allowed_collections: collections.into_iter().collect(),
            denied_collections: std::collections::HashSet::new(),
        }
    }

    /// Create a new filter that denies specific collections
    pub fn deny(collections: Vec<String>) -> Self {
        Self {
            allowed_collections: std::collections::HashSet::new(),
            denied_collections: collections.into_iter().collect(),
        }
    }

    /// Create a filter that allows all collections
    pub fn allow_all() -> Self {
        Self {
            allowed_collections: std::collections::HashSet::new(),
            denied_collections: std::collections::HashSet::new(),
        }
    }

    fn should_execute_for_collection(&self, collection: &str) -> bool {
        // If there are denied collections, check if this collection is denied
        if !self.denied_collections.is_empty() {
            return !self.denied_collections.contains(collection);
        }

        // If there are allowed collections, check if this collection is allowed
        if !self.allowed_collections.is_empty() {
            return self.allowed_collections.contains(collection);
        }

        // If no restrictions, allow all
        true
    }
}

impl EventFilter for CollectionFilter {
    fn should_execute_before(
        &self,
        _handler: &HandlerMetadata,
        context: &BeforeEventContext,
    ) -> bool {
        self.should_execute_for_collection(&context.collection)
    }

    fn should_execute_after(
        &self,
        _handler: &HandlerMetadata,
        context: &AfterEventContext,
    ) -> bool {
        match context {
            AfterEventContext::RecordCreated { collection, .. }
            | AfterEventContext::RecordUpdated { collection, .. }
            | AfterEventContext::RecordDeleted { collection, .. }
            | AfterEventContext::RecordRead { collection, .. }
            | AfterEventContext::CollectionCreated { collection, .. }
            | AfterEventContext::CollectionUpdated { collection, .. }
            | AfterEventContext::CollectionDeleted { collection, .. } => {
                self.should_execute_for_collection(collection)
            }
            // Non-collection events are always allowed
            _ => true,
        }
    }
}

/// Tag-based filter that executes handlers based on tags
pub struct TagFilter {
    pub required_tags: std::collections::HashMap<String, String>,
    pub forbidden_tags: std::collections::HashMap<String, String>,
}

impl TagFilter {
    /// Create a new filter that requires specific tags
    pub fn require_tags(tags: std::collections::HashMap<String, String>) -> Self {
        Self {
            required_tags: tags,
            forbidden_tags: std::collections::HashMap::new(),
        }
    }

    /// Create a new filter that forbids specific tags
    pub fn forbid_tags(tags: std::collections::HashMap<String, String>) -> Self {
        Self {
            required_tags: std::collections::HashMap::new(),
            forbidden_tags: tags,
        }
    }

    fn should_execute_for_tags(
        &self,
        handler_tags: &std::collections::HashMap<String, String>,
        context_tags: &std::collections::HashMap<String, String>,
    ) -> bool {
        // Check required tags
        for (key, value) in &self.required_tags {
            if let Some(handler_value) = handler_tags.get(key) {
                if handler_value != value {
                    return false;
                }
            } else if let Some(context_value) = context_tags.get(key) {
                if context_value != value {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check forbidden tags
        for (key, value) in &self.forbidden_tags {
            if let Some(handler_value) = handler_tags.get(key) {
                if handler_value == value {
                    return false;
                }
            }
            if let Some(context_value) = context_tags.get(key) {
                if context_value == value {
                    return false;
                }
            }
        }

        true
    }
}

impl EventFilter for TagFilter {
    fn should_execute_before(
        &self,
        handler: &HandlerMetadata,
        context: &BeforeEventContext,
    ) -> bool {
        self.should_execute_for_tags(&handler.tags, &context.tags)
    }

    fn should_execute_after(
        &self,
        handler: &HandlerMetadata,
        _context: &AfterEventContext,
    ) -> bool {
        // After events don't have tags directly, so only check handler tags against required/forbidden
        self.should_execute_for_tags(&handler.tags, &std::collections::HashMap::new())
    }
}

/// Composite filter that combines multiple filters with AND logic
pub struct CompositeFilter {
    pub filters: Vec<Box<dyn EventFilter>>,
}

impl CompositeFilter {
    /// Create a new composite filter
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Add a filter to the composite
    pub fn add_filter(mut self, filter: Box<dyn EventFilter>) -> Self {
        self.filters.push(filter);
        self
    }
}

impl Default for CompositeFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventFilter for CompositeFilter {
    fn should_execute_before(
        &self,
        handler: &HandlerMetadata,
        context: &BeforeEventContext,
    ) -> bool {
        self.filters
            .iter()
            .all(|filter| filter.should_execute_before(handler, context))
    }

    fn should_execute_after(&self, handler: &HandlerMetadata, context: &AfterEventContext) -> bool {
        self.filters
            .iter()
            .all(|filter| filter.should_execute_after(handler, context))
    }
}

/// Utility macros for creating handlers
#[macro_export]
macro_rules! before_handler {
    ($name:expr, $handler:expr) => {
        ManagedBeforeHandler {
            metadata: HandlerMetadata::new(uuid::Uuid::new_v4().to_string(), $name.to_string()),
            handler: std::sync::Arc::new($handler),
        }
    };
    ($id:expr, $name:expr, $handler:expr) => {
        ManagedBeforeHandler {
            metadata: HandlerMetadata::new($id.to_string(), $name.to_string()),
            handler: std::sync::Arc::new($handler),
        }
    };
}

#[macro_export]
macro_rules! after_handler {
    ($name:expr, $handler:expr) => {
        ManagedAfterHandler {
            metadata: HandlerMetadata::new(uuid::Uuid::new_v4().to_string(), $name.to_string()),
            handler: std::sync::Arc::new($handler),
        }
    };
    ($id:expr, $name:expr, $handler:expr) => {
        ManagedAfterHandler {
            metadata: HandlerMetadata::new($id.to_string(), $name.to_string()),
            handler: std::sync::Arc::new($handler),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::context::BeforeEventContext;

    #[test]
    fn test_handler_metadata_fluent_api() {
        let metadata = HandlerMetadata::new("test-id".to_string(), "Test Handler".to_string())
            .with_description("A test handler".to_string())
            .with_priority(100)
            .with_timeout(5000)
            .with_max_retries(3)
            .with_tag("env".to_string(), "test".to_string());

        assert_eq!(metadata.id, "test-id");
        assert_eq!(metadata.name, "Test Handler");
        assert_eq!(metadata.description, "A test handler");
        assert_eq!(metadata.priority, 100);
        assert_eq!(metadata.timeout_ms, Some(5000));
        assert_eq!(metadata.max_retries, Some(3));
        assert_eq!(metadata.tags.get("env").unwrap(), "test");
    }

    #[test]
    fn test_collection_filter() {
        let filter = CollectionFilter::allow(vec!["users".to_string(), "posts".to_string()]);
        let metadata = HandlerMetadata::new("test".to_string(), "test".to_string());

        let context_users =
            BeforeEventContext::new_create("users".to_string(), serde_json::json!({}));
        let context_admin =
            BeforeEventContext::new_create("admin".to_string(), serde_json::json!({}));

        assert!(filter.should_execute_before(&metadata, &context_users));
        assert!(!filter.should_execute_before(&metadata, &context_admin));
    }

    #[test]
    fn test_tag_filter() {
        let mut required_tags = std::collections::HashMap::new();
        required_tags.insert("env".to_string(), "production".to_string());
        let filter = TagFilter::require_tags(required_tags);

        let metadata_prod = HandlerMetadata::new("test".to_string(), "test".to_string())
            .with_tag("env".to_string(), "production".to_string());
        let metadata_dev = HandlerMetadata::new("test".to_string(), "test".to_string())
            .with_tag("env".to_string(), "development".to_string());

        let context = BeforeEventContext::new_create("users".to_string(), serde_json::json!({}));

        assert!(filter.should_execute_before(&metadata_prod, &context));
        assert!(!filter.should_execute_before(&metadata_dev, &context));
    }

    #[test]
    fn test_execution_result() {
        let result = HandlerExecutionResult::success("handler-1".to_string(), 150)
            .with_retries(2)
            .with_metadata("cache_hit".to_string(), "true".to_string());

        assert!(result.success);
        assert_eq!(result.execution_time_ms, 150);
        assert_eq!(result.retry_attempts, 2);
        assert_eq!(result.metadata.get("cache_hit").unwrap(), "true");
    }
}
