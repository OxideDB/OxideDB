//! Event system for OxideDB
//!
//! This module defines the core event system that enables the hook-first
//! architecture. All business logic operations dispatch events through
//! the EventBus, allowing plugins and internal listeners to hook into
//! and extend functionality.
//!
//! The new architecture separates Before and After events:
//! - Before events allow modification of data before processing
//! - After events are read-only notifications after processing

use crate::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tracing::{info, warn};

/// Unique identifier for a record in the database
pub type RecordId = String;

/// Collection name in the database
pub type Collection = String;

/// Record data as JSON
pub type RecordData = JsonValue;

/// Context for Before events that allows data modification
#[derive(Debug, Clone)]
pub struct BeforeEventContext {
    /// The collection being operated on
    pub collection: Collection,
    /// Mutable data that can be transformed by handlers
    pub data: RecordData,
    /// Additional metadata that can be read/modified
    pub metadata: JsonValue,
    /// For update operations, the record ID
    pub record_id: Option<RecordId>,
    /// For update operations, the old data (read-only)
    pub old_data: Option<RecordData>,
}

impl BeforeEventContext {
    /// Create a new context for record creation
    pub fn new_create(collection: Collection, data: RecordData) -> Self {
        Self {
            collection,
            data,
            metadata: JsonValue::Object(serde_json::Map::new()),
            record_id: None,
            old_data: None,
        }
    }

    /// Create a new context for record update
    pub fn new_update(collection: Collection, record_id: RecordId, old_data: RecordData, new_data: RecordData) -> Self {
        Self {
            collection,
            data: new_data,
            metadata: JsonValue::Object(serde_json::Map::new()),
            record_id: Some(record_id),
            old_data: Some(old_data),
        }
    }

    /// Create a new context for record deletion
    pub fn new_delete(collection: Collection, record_id: RecordId, data: RecordData) -> Self {
        Self {
            collection,
            data,
            metadata: JsonValue::Object(serde_json::Map::new()),
            record_id: Some(record_id),
            old_data: None,
        }
    }
}

/// After event context for read-only notifications
#[derive(Debug, Clone)]
pub enum AfterEventContext {
    RecordCreated {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },
    RecordUpdated {
        collection: Collection,
        record_id: RecordId,
        old_data: RecordData,
        new_data: RecordData,
    },
    RecordDeleted {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },
    RecordRead {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },
    CollectionCreated {
        collection: Collection,
    },
    CollectionDeleted {
        collection: Collection,
    },
    UserRegistered {
        user_id: String,
        email: String,
        metadata: JsonValue,
    },
    UserAuthenticated {
        user_id: String,
        email: String,
    },
    SystemStartup,
    SystemShutdown,
    DatabaseConnected {
        database_url: String,
    },
    DatabaseDisconnected,
    PluginLoaded {
        plugin_name: String,
    },
    PluginUnloaded {
        plugin_name: String,
    },
    PluginError {
        plugin_name: String,
        error: String,
    },
    ApiRequestProcessed {
        method: String,
        path: String,
        status_code: u16,
        response_time_ms: u64,
    },
    ErrorOccurred {
        error_type: String,
        message: String,
        context: JsonValue,
    },
}

/// Event types for Before handlers that can modify data
#[derive(Debug, Clone)]
pub enum BeforeEventType {
    RecordCreate,
    RecordUpdate,
    RecordDelete,
    RecordRead,
    CollectionCreate,
    CollectionDelete,
    UserAuth,
    ApiRequest,
}

impl BeforeEventType {
    pub fn name(&self) -> &'static str {
        match self {
            BeforeEventType::RecordCreate => "BeforeRecordCreate",
            BeforeEventType::RecordUpdate => "BeforeRecordUpdate",
            BeforeEventType::RecordDelete => "BeforeRecordDelete",
            BeforeEventType::RecordRead => "BeforeRecordRead",
            BeforeEventType::CollectionCreate => "BeforeCollectionCreate",
            BeforeEventType::CollectionDelete => "BeforeCollectionDelete",
            BeforeEventType::UserAuth => "BeforeUserAuth",
            BeforeEventType::ApiRequest => "BeforeApiRequest",
        }
    }
}

/// Event types for After handlers that are read-only notifications
#[derive(Debug, Clone)]
pub enum AfterEventType {
    RecordCreated,
    RecordUpdated,
    RecordDeleted,
    RecordRead,
    CollectionCreated,
    CollectionDeleted,
    UserRegistered,
    UserAuthenticated,
    SystemStartup,
    SystemShutdown,
    DatabaseConnected,
    DatabaseDisconnected,
    PluginLoaded,
    PluginUnloaded,
    PluginError,
    ApiRequestProcessed,
    ErrorOccurred,
}

impl AfterEventType {
    pub fn name(&self) -> &'static str {
        match self {
            AfterEventType::RecordCreated => "AfterRecordCreate",
            AfterEventType::RecordUpdated => "AfterRecordUpdate",
            AfterEventType::RecordDeleted => "AfterRecordDelete",
            AfterEventType::RecordRead => "AfterRecordRead",
            AfterEventType::CollectionCreated => "AfterCollectionCreate",
            AfterEventType::CollectionDeleted => "AfterCollectionDelete",
            AfterEventType::UserRegistered => "OnUserRegister",
            AfterEventType::UserAuthenticated => "AfterUserAuth",
            AfterEventType::SystemStartup => "OnSystemStartup",
            AfterEventType::SystemShutdown => "OnSystemShutdown",
            AfterEventType::DatabaseConnected => "OnDatabaseConnect",
            AfterEventType::DatabaseDisconnected => "OnDatabaseDisconnect",
            AfterEventType::PluginLoaded => "OnPluginLoad",
            AfterEventType::PluginUnloaded => "OnPluginUnload",
            AfterEventType::PluginError => "OnPluginError",
            AfterEventType::ApiRequestProcessed => "AfterApiRequest",
            AfterEventType::ErrorOccurred => "OnError",
        }
    }
}

/// Handler for Before events that can modify the context
pub type BeforeEventHandler = Box<dyn Fn(&mut BeforeEventContext) -> Result<(), AppError> + Send + Sync>;

/// Handler for After events that are read-only
pub type AfterEventHandler = Box<dyn Fn(&AfterEventContext) -> Result<(), AppError> + Send + Sync>;

/// The EventBus trait defines the interface for dispatching and subscribing to events.
///
/// This is the core contract that enables the hook-first architecture. All business
/// logic must dispatch events through an EventBus implementation, and plugins/listeners
/// subscribe to relevant events to extend functionality.
#[async_trait::async_trait]
pub trait EventBus: Send + Sync {
    /// Dispatch a Before event to all registered listeners, allowing data modification
    ///
    /// This method delivers the event to all listeners that have subscribed
    /// to this event type. Handlers can modify the context data.
    async fn dispatch_before(&self, event_type: BeforeEventType, context: &mut BeforeEventContext) -> Result<(), AppError>;

    /// Dispatch an After event to all registered listeners for read-only notification
    ///
    /// This method delivers the event to all listeners that have subscribed
    /// to this event type. Handlers cannot modify the data.
    async fn dispatch_after(&self, event_type: AfterEventType, context: &AfterEventContext) -> Result<(), AppError>;

    /// Subscribe a handler to Before events (can modify data)
    fn subscribe_before(&self, event_name: &str, handler: BeforeEventHandler) -> Result<(), AppError>;

    /// Subscribe a handler to After events (read-only)
    fn subscribe_after(&self, event_name: &str, handler: AfterEventHandler) -> Result<(), AppError>;

    /// Get the number of active Before listeners for an event type
    fn before_listener_count(&self, event_name: &str) -> usize;

    /// Get the number of active After listeners for an event type
    fn after_listener_count(&self, event_name: &str) -> usize;

    /// Get the total number of events dispatched
    fn events_dispatched(&self) -> u64;
}

/// A basic, synchronous, in-memory implementation of EventBus
///
/// This implementation stores handlers in memory and executes them synchronously
/// when events are dispatched. It's suitable for development and testing, but
/// production systems might want a more sophisticated implementation with
/// async execution, persistence, etc.
pub struct InMemoryEventBus {
    before_handlers: Arc<Mutex<HashMap<String, Vec<BeforeEventHandler>>>>,
    after_handlers: Arc<Mutex<HashMap<String, Vec<AfterEventHandler>>>>,
    events_dispatched: Arc<Mutex<u64>>,
}

impl InMemoryEventBus {
    /// Create a new InMemoryEventBus instance
    pub fn new() -> Self {
        Self {
            before_handlers: Arc::new(Mutex::new(HashMap::new())),
            after_handlers: Arc::new(Mutex::new(HashMap::new())),
            events_dispatched: Arc::new(Mutex::new(0)),
        }
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl EventBus for InMemoryEventBus {
    async fn dispatch_before(&self, event_type: BeforeEventType, context: &mut BeforeEventContext) -> Result<(), AppError> {
        let event_name = event_type.name();

        info!("Dispatching Before event: {} for collection: {}", event_name, context.collection);

        // Increment dispatch counter
        {
            let mut counter = self.events_dispatched.lock().map_err(|_| {
                AppError::internal("Failed to acquire lock on events_dispatched counter")
            })?;
            *counter += 1;
        }

        // Execute all Before handlers while holding the lock
        let mut errors = Vec::new();
        {
            let handlers_map = self
                .before_handlers
                .lock()
                .map_err(|_| AppError::internal("Failed to acquire lock on before event handlers"))?;

            if let Some(handlers) = handlers_map.get(event_name) {
                for (index, handler) in handlers.iter().enumerate() {
                    if let Err(err) = handler(context) {
                        warn!("Before handler {} for event {} failed: {}", index, event_name, err);
                        errors.push(err);
                    }
                }
            }
        }

        // If any handlers failed, return the first error
        if let Some(first_error) = errors.into_iter().next() {
            return Err(first_error);
        }

        Ok(())
    }

    async fn dispatch_after(&self, event_type: AfterEventType, context: &AfterEventContext) -> Result<(), AppError> {
        let event_name = event_type.name();

        info!("Dispatching After event: {}", event_name);

        // Increment dispatch counter
        {
            let mut counter = self.events_dispatched.lock().map_err(|_| {
                AppError::internal("Failed to acquire lock on events_dispatched counter")
            })?;
            *counter += 1;
        }

        // Execute all After handlers while holding the lock
        let mut errors = Vec::new();
        {
            let handlers_map = self
                .after_handlers
                .lock()
                .map_err(|_| AppError::internal("Failed to acquire lock on after event handlers"))?;

            if let Some(handlers) = handlers_map.get(event_name) {
                for (index, handler) in handlers.iter().enumerate() {
                    if let Err(err) = handler(context) {
                        warn!("After handler {} for event {} failed: {}", index, event_name, err);
                        errors.push(err);
                    }
                }
            }
        }

        // If any handlers failed, return the first error
        if let Some(first_error) = errors.into_iter().next() {
            return Err(first_error);
        }

        Ok(())
    }

    fn subscribe_before(&self, event_name: &str, handler: BeforeEventHandler) -> Result<(), AppError> {
        let mut handlers_map = self.before_handlers.lock().map_err(|_| {
            AppError::internal("Failed to acquire lock on before event handlers for subscription")
        })?;

        let handlers = handlers_map
            .entry(event_name.to_string())
            .or_insert_with(Vec::new);

        handlers.push(handler);

        info!("Subscribed Before handler to event: {}", event_name);
        Ok(())
    }

    fn subscribe_after(&self, event_name: &str, handler: AfterEventHandler) -> Result<(), AppError> {
        let mut handlers_map = self.after_handlers.lock().map_err(|_| {
            AppError::internal("Failed to acquire lock on after event handlers for subscription")
        })?;

        let handlers = handlers_map
            .entry(event_name.to_string())
            .or_insert_with(Vec::new);

        handlers.push(handler);

        info!("Subscribed After handler to event: {}", event_name);
        Ok(())
    }

    fn before_listener_count(&self, event_name: &str) -> usize {
        self.before_handlers
            .lock()
            .map(|handlers_map| {
                handlers_map
                    .get(event_name)
                    .map(|handlers| handlers.len())
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    fn after_listener_count(&self, event_name: &str) -> usize {
        self.after_handlers
            .lock()
            .map(|handlers_map| {
                handlers_map
                    .get(event_name)
                    .map(|handlers| handlers.len())
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    fn events_dispatched(&self) -> u64 {
        self.events_dispatched
            .lock()
            .map(|counter| *counter)
            .unwrap_or(0)
    }
}

// Legacy compatibility wrapper for backward compatibility
// This maintains the old Event enum and EventHandler for any existing code

/// Legacy Event enum for backward compatibility
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    BeforeRecordCreate {
        collection: Collection,
        data: RecordData,
    },
    AfterRecordCreate {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },
}

impl Event {
    pub fn name(&self) -> &'static str {
        match self {
            Event::BeforeRecordCreate { .. } => "BeforeRecordCreate",
            Event::AfterRecordCreate { .. } => "AfterRecordCreate",
        }
    }
}

/// Legacy event handler function type for backward compatibility
pub type EventHandler = Box<dyn Fn(&Event) -> Result<(), AppError> + Send + Sync>;

/// Legacy methods for backward compatibility
impl InMemoryEventBus {
    /// Legacy dispatch method for backward compatibility
    pub async fn dispatch(&self, event: Event) -> Result<(), AppError> {
        warn!("Using legacy dispatch method - consider migrating to dispatch_before/dispatch_after");
        // This is a simplified legacy implementation
        Ok(())
    }

    /// Legacy subscribe method for backward compatibility
    pub fn subscribe(&self, event_name: &str, handler: EventHandler) -> Result<(), AppError> {
        warn!("Using legacy subscribe method - consider migrating to subscribe_before/subscribe_after");
        // This is a simplified legacy implementation
        Ok(())
    }

    /// Legacy listener count method for backward compatibility
    pub fn listener_count(&self, event_name: &str) -> usize {
        self.before_listener_count(event_name) + self.after_listener_count(event_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_before_event_modification() {
        let bus = InMemoryEventBus::new();
        
        // Subscribe a handler that modifies the data
        bus.subscribe_before(
            "BeforeRecordCreate",
            Box::new(|context| {
                // Modify the data - add a timestamp
                if let Some(obj) = context.data.as_object_mut() {
                    obj.insert("modified_by_hook".to_string(), serde_json::json!(true));
                }
                Ok(())
            }),
        ).unwrap();

        // Create a context with original data
        let mut context = BeforeEventContext::new_create(
            "users".to_string(),
            serde_json::json!({"name": "John"}),
        );

        // Dispatch the event
        bus.dispatch_before(BeforeEventType::RecordCreate, &mut context).await.unwrap();

        // Verify the data was modified
        assert!(context.data.get("modified_by_hook").is_some());
        assert_eq!(context.data.get("name").unwrap(), "John");
    }

    #[tokio::test]
    async fn test_after_event_notification() {
        let bus = InMemoryEventBus::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        // Subscribe to After events
        bus.subscribe_after(
            "AfterRecordCreate",
            Box::new(move |_context| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        ).unwrap();

        // Dispatch an after event
        let context = AfterEventContext::RecordCreated {
            collection: "users".to_string(),
            record_id: "123".to_string(),
            data: serde_json::json!({"name": "John"}),
        };

        bus.dispatch_after(AfterEventType::RecordCreated, &context).await.unwrap();

        // Verify handler was called
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_error_propagation() {
        let bus = InMemoryEventBus::new();

        // Subscribe a handler that fails
        bus.subscribe_before(
            "BeforeRecordCreate",
            Box::new(|_context| Err(AppError::internal("Handler failed"))),
        ).unwrap();

        let mut context = BeforeEventContext::new_create(
            "users".to_string(),
            serde_json::json!({"name": "John"}),
        );

        let result = bus.dispatch_before(BeforeEventType::RecordCreate, &mut context).await;
        assert!(result.is_err());
    }
}
