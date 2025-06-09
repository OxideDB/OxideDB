//! Event system for OxideDB
//!
//! This module defines the core event system that enables the hook-first
//! architecture. All business logic operations dispatch events through
//! the EventBus, allowing plugins and internal listeners to hook into
//! and extend functionality.

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

/// The master Event enum that defines all events in the OxideDB system.
///
/// This is the heart of the hooking system. Each event represents a specific
/// point in the application lifecycle where plugins and listeners can hook in.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
    // === Record Management Events ===
    /// Dispatched before creating a new record
    BeforeRecordCreate {
        collection: Collection,
        data: RecordData,
    },
    /// Dispatched after successfully creating a record
    AfterRecordCreate {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },
    /// Dispatched before updating a record
    BeforeRecordUpdate {
        collection: Collection,
        record_id: RecordId,
        old_data: RecordData,
        new_data: RecordData,
    },
    /// Dispatched after successfully updating a record
    AfterRecordUpdate {
        collection: Collection,
        record_id: RecordId,
        old_data: RecordData,
        new_data: RecordData,
    },
    /// Dispatched before deleting a record
    BeforeRecordDelete {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },
    /// Dispatched after successfully deleting a record
    AfterRecordDelete {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },
    /// Dispatched before reading a record
    BeforeRecordRead {
        collection: Collection,
        record_id: RecordId,
    },
    /// Dispatched after successfully reading a record
    AfterRecordRead {
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
    },

    // === Collection Management Events ===
    /// Dispatched before creating a collection
    BeforeCollectionCreate { collection: Collection },
    /// Dispatched after successfully creating a collection
    AfterCollectionCreate { collection: Collection },
    /// Dispatched before deleting a collection
    BeforeCollectionDelete { collection: Collection },
    /// Dispatched after successfully deleting a collection
    AfterCollectionDelete { collection: Collection },

    // === User Authentication Events ===
    /// Dispatched when a user registers
    OnUserRegister {
        user_id: String,
        email: String,
        metadata: JsonValue,
    },
    /// Dispatched when a user logs in
    OnUserLogin { user_id: String, email: String },
    /// Dispatched when a user logs out
    OnUserLogout { user_id: String },
    /// Dispatched before user authentication
    BeforeUserAuth { email: String },
    /// Dispatched after successful user authentication
    AfterUserAuth { user_id: String, email: String },

    // === System Events ===
    /// Dispatched when the system starts up
    OnSystemStartup,
    /// Dispatched when the system shuts down
    OnSystemShutdown,
    /// Dispatched on database connection established
    OnDatabaseConnect { database_url: String },
    /// Dispatched on database connection lost
    OnDatabaseDisconnect,

    // === Plugin Events ===
    /// Dispatched when a plugin is loaded
    OnPluginLoad { plugin_name: String },
    /// Dispatched when a plugin is unloaded
    OnPluginUnload { plugin_name: String },
    /// Dispatched when a plugin encounters an error
    OnPluginError { plugin_name: String, error: String },

    // === API Events ===
    /// Dispatched before processing an API request
    BeforeApiRequest {
        method: String,
        path: String,
        headers: JsonValue,
    },
    /// Dispatched after processing an API request
    AfterApiRequest {
        method: String,
        path: String,
        status_code: u16,
        response_time_ms: u64,
    },

    // === Error Events ===
    /// Dispatched when an error occurs
    OnError {
        error_type: String,
        message: String,
        context: JsonValue,
    },
}

impl Event {
    /// Get a human-readable name for this event type
    pub fn name(&self) -> &'static str {
        match self {
            Event::BeforeRecordCreate { .. } => "BeforeRecordCreate",
            Event::AfterRecordCreate { .. } => "AfterRecordCreate",
            Event::BeforeRecordUpdate { .. } => "BeforeRecordUpdate",
            Event::AfterRecordUpdate { .. } => "AfterRecordUpdate",
            Event::BeforeRecordDelete { .. } => "BeforeRecordDelete",
            Event::AfterRecordDelete { .. } => "AfterRecordDelete",
            Event::BeforeRecordRead { .. } => "BeforeRecordRead",
            Event::AfterRecordRead { .. } => "AfterRecordRead",
            Event::BeforeCollectionCreate { .. } => "BeforeCollectionCreate",
            Event::AfterCollectionCreate { .. } => "AfterCollectionCreate",
            Event::BeforeCollectionDelete { .. } => "BeforeCollectionDelete",
            Event::AfterCollectionDelete { .. } => "AfterCollectionDelete",
            Event::OnUserRegister { .. } => "OnUserRegister",
            Event::OnUserLogin { .. } => "OnUserLogin",
            Event::OnUserLogout { .. } => "OnUserLogout",
            Event::BeforeUserAuth { .. } => "BeforeUserAuth",
            Event::AfterUserAuth { .. } => "AfterUserAuth",
            Event::OnSystemStartup => "OnSystemStartup",
            Event::OnSystemShutdown => "OnSystemShutdown",
            Event::OnDatabaseConnect { .. } => "OnDatabaseConnect",
            Event::OnDatabaseDisconnect => "OnDatabaseDisconnect",
            Event::OnPluginLoad { .. } => "OnPluginLoad",
            Event::OnPluginUnload { .. } => "OnPluginUnload",
            Event::OnPluginError { .. } => "OnPluginError",
            Event::BeforeApiRequest { .. } => "BeforeApiRequest",
            Event::AfterApiRequest { .. } => "AfterApiRequest",
            Event::OnError { .. } => "OnError",
        }
    }
}

/// Event handler function type
pub type EventHandler = Box<dyn Fn(&Event) -> Result<(), AppError> + Send + Sync>;

/// The EventBus trait defines the interface for dispatching and subscribing to events.
///
/// This is the core contract that enables the hook-first architecture. All business
/// logic must dispatch events through an EventBus implementation, and plugins/listeners
/// subscribe to relevant events to extend functionality.
#[async_trait::async_trait]
pub trait EventBus: Send + Sync {
    /// Dispatch an event to all registered listeners
    ///
    /// This method delivers the event to all listeners that have subscribed
    /// to this event type. The method should be non-blocking and handle
    /// any listener errors gracefully.
    async fn dispatch(&self, event: Event) -> Result<(), AppError>;

    /// Subscribe a handler to specific event types
    ///
    /// The handler will be called whenever an event of the specified type
    /// is dispatched. Multiple handlers can be registered for the same event type.
    fn subscribe(&self, event_name: &str, handler: EventHandler) -> Result<(), AppError>;

    /// Get the number of active listeners for an event type
    fn listener_count(&self, event_name: &str) -> usize;

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
    handlers: Arc<Mutex<HashMap<String, Vec<EventHandler>>>>,
    events_dispatched: Arc<Mutex<u64>>,
}

impl InMemoryEventBus {
    /// Create a new InMemoryEventBus instance
    pub fn new() -> Self {
        Self {
            handlers: Arc::new(Mutex::new(HashMap::new())),
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
    async fn dispatch(&self, event: Event) -> Result<(), AppError> {
        let event_name = event.name();

        info!("Dispatching event: {} - {:?}", event_name, event);

        // Increment dispatch counter
        {
            let mut counter = self.events_dispatched.lock().map_err(|_| {
                AppError::internal("Failed to acquire lock on events_dispatched counter")
            })?;
            *counter += 1;
        }

        // Execute all handlers while holding the lock
        let mut errors = Vec::new();
        {
            let handlers_map = self
                .handlers
                .lock()
                .map_err(|_| AppError::internal("Failed to acquire lock on event handlers"))?;

            if let Some(handlers) = handlers_map.get(event_name) {
                for (index, handler) in handlers.iter().enumerate() {
                    if let Err(err) = handler(&event) {
                        warn!("Handler {} for event {} failed: {}", index, event_name, err);
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

    fn subscribe(&self, event_name: &str, handler: EventHandler) -> Result<(), AppError> {
        let mut handlers_map = self.handlers.lock().map_err(|_| {
            AppError::internal("Failed to acquire lock on event handlers for subscription")
        })?;

        let handlers = handlers_map
            .entry(event_name.to_string())
            .or_insert_with(Vec::new);

        handlers.push(handler);

        info!("Subscribed handler to event: {}", event_name);
        Ok(())
    }

    fn listener_count(&self, event_name: &str) -> usize {
        self.handlers
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_event_bus_dispatch_and_subscribe() {
        let bus = InMemoryEventBus::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = Arc::clone(&call_count);

        // Subscribe to BeforeRecordCreate events
        bus.subscribe(
            "BeforeRecordCreate",
            Box::new(move |_event| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
        )
        .unwrap();

        // Dispatch an event
        let event = Event::BeforeRecordCreate {
            collection: "users".to_string(),
            data: serde_json::json!({"name": "John"}),
        };

        bus.dispatch(event).await.unwrap();

        // Verify handler was called
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.listener_count("BeforeRecordCreate"), 1);
        assert_eq!(bus.events_dispatched(), 1);
    }

    #[tokio::test]
    async fn test_event_handler_error_propagation() {
        let bus = InMemoryEventBus::new();

        // Subscribe a handler that always fails
        bus.subscribe(
            "BeforeRecordCreate",
            Box::new(|_event| Err(AppError::internal("Handler failed"))),
        )
        .unwrap();

        let event = Event::BeforeRecordCreate {
            collection: "users".to_string(),
            data: serde_json::json!({"name": "John"}),
        };

        let result = bus.dispatch(event).await;
        assert!(result.is_err());
    }
}
