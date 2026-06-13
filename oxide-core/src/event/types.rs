//! Event Types and Metadata
//!
//! This module defines all event types supported by the event system,
//! along with their metadata for better introspection and debugging.

use serde::{Deserialize, Serialize};

/// Unique identifier for a record in the database
pub type RecordId = String;

/// Collection name in the database
pub type Collection = String;

/// Record data as JSON
pub type RecordData = serde_json::Value;

/// Event types for Before handlers that can modify data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BeforeEventType {
    RecordCreate,
    RecordUpdate,
    RecordDelete,
    RecordRead,
    CollectionCreate,
    CollectionUpdate,
    CollectionDelete,
    UserAuth,
    ApiRequest,
    FileWrite,
    FileMove,
    FileRead,
    FileDelete,
}

impl BeforeEventType {
    /// Get the string name of the event type
    pub fn name(&self) -> &'static str {
        match self {
            BeforeEventType::RecordCreate => "BeforeRecordCreate",
            BeforeEventType::RecordUpdate => "BeforeRecordUpdate",
            BeforeEventType::RecordDelete => "BeforeRecordDelete",
            BeforeEventType::RecordRead => "BeforeRecordRead",
            BeforeEventType::CollectionCreate => "BeforeCollectionCreate",
            BeforeEventType::CollectionUpdate => "BeforeCollectionUpdate",
            BeforeEventType::CollectionDelete => "BeforeCollectionDelete",
            BeforeEventType::UserAuth => "BeforeUserAuth",
            BeforeEventType::ApiRequest => "BeforeApiRequest",
            BeforeEventType::FileWrite => "BeforeFileWrite",
            BeforeEventType::FileMove => "BeforeFileMove",
            BeforeEventType::FileRead => "BeforeFileRead",
            BeforeEventType::FileDelete => "BeforeFileDelete",
        }
    }

    /// Get metadata about this event type
    pub fn metadata(&self) -> EventTypeMetadata {
        match self {
            BeforeEventType::RecordCreate => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a record is created, allows modification of record data",
                category: EventCategory::DataOperation,
                can_modify_data: true,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            BeforeEventType::RecordUpdate => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a record is updated, allows modification of new data",
                category: EventCategory::DataOperation,
                can_modify_data: true,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            BeforeEventType::RecordDelete => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a record is deleted, can prevent deletion",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: true,
                recommended_timeout_ms: 2000,
            },
            BeforeEventType::RecordRead => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a record is read, can filter or transform data",
                category: EventCategory::DataOperation,
                can_modify_data: true,
                is_system_critical: false,
                recommended_timeout_ms: 500,
            },
            BeforeEventType::CollectionCreate => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a collection is created, can modify schema",
                category: EventCategory::SchemaOperation,
                can_modify_data: true,
                is_system_critical: true,
                recommended_timeout_ms: 5000,
            },
            BeforeEventType::CollectionUpdate => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a collection schema is updated, can modify new schema",
                category: EventCategory::SchemaOperation,
                can_modify_data: true,
                is_system_critical: true,
                recommended_timeout_ms: 5000,
            },
            BeforeEventType::CollectionDelete => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a collection is deleted, can prevent deletion",
                category: EventCategory::SchemaOperation,
                can_modify_data: false,
                is_system_critical: true,
                recommended_timeout_ms: 5000,
            },
            BeforeEventType::UserAuth => EventTypeMetadata {
                name: self.name(),
                description: "Fired before user authentication, can modify auth flow",
                category: EventCategory::Authentication,
                can_modify_data: true,
                is_system_critical: true,
                recommended_timeout_ms: 3000,
            },
            BeforeEventType::ApiRequest => EventTypeMetadata {
                name: self.name(),
                description: "Fired before API request processing, can modify request",
                category: EventCategory::ApiOperation,
                can_modify_data: true,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            BeforeEventType::FileWrite => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a file is written to VFS, can modify file data",
                category: EventCategory::DataOperation,
                can_modify_data: true,
                is_system_critical: false,
                recommended_timeout_ms: 2000,
            },
            BeforeEventType::FileMove => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a file is moved within VFS, can prevent the move",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            BeforeEventType::FileRead => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a file is read from VFS, can filter access",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 500,
            },
            BeforeEventType::FileDelete => EventTypeMetadata {
                name: self.name(),
                description: "Fired before a file is deleted from VFS, can prevent deletion",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: true,
                recommended_timeout_ms: 1000,
            },
        }
    }

    /// Get all available Before event types
    pub fn all() -> Vec<BeforeEventType> {
        vec![
            BeforeEventType::RecordCreate,
            BeforeEventType::RecordUpdate,
            BeforeEventType::RecordDelete,
            BeforeEventType::RecordRead,
            BeforeEventType::CollectionCreate,
            BeforeEventType::CollectionUpdate,
            BeforeEventType::CollectionDelete,
            BeforeEventType::UserAuth,
            BeforeEventType::ApiRequest,
            BeforeEventType::FileWrite,
            BeforeEventType::FileMove,
            BeforeEventType::FileRead,
            BeforeEventType::FileDelete,
        ]
    }
}

/// Event types for After handlers that are read-only notifications
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AfterEventType {
    RecordCreated,
    RecordUpdated,
    RecordDeleted,
    RecordRead,
    CollectionCreated,
    CollectionUpdated,
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
    FileWritten,
    FileMoved,
    FileRead,
    FileDeleted,
    ErrorOccurred,
}

impl AfterEventType {
    /// Get the string name of the event type
    pub fn name(&self) -> &'static str {
        match self {
            AfterEventType::RecordCreated => "AfterRecordCreate",
            AfterEventType::RecordUpdated => "AfterRecordUpdate",
            AfterEventType::RecordDeleted => "AfterRecordDelete",
            AfterEventType::RecordRead => "AfterRecordRead",
            AfterEventType::CollectionCreated => "AfterCollectionCreate",
            AfterEventType::CollectionUpdated => "AfterCollectionUpdate",
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
            AfterEventType::FileWritten => "AfterFileWrite",
            AfterEventType::FileMoved => "AfterFileMove",
            AfterEventType::FileRead => "AfterFileRead",
            AfterEventType::FileDeleted => "AfterFileDelete",
            AfterEventType::ErrorOccurred => "OnError",
        }
    }

    /// Get metadata about this event type
    pub fn metadata(&self) -> EventTypeMetadata {
        match self {
            AfterEventType::RecordCreated => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a record is successfully created",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::RecordUpdated => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a record is successfully updated",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::RecordDeleted => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a record is successfully deleted",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::RecordRead => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a record is successfully read",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 500,
            },
            AfterEventType::CollectionCreated => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a collection is successfully created",
                category: EventCategory::SchemaOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 2000,
            },
            AfterEventType::CollectionUpdated => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a collection schema is successfully updated",
                category: EventCategory::SchemaOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 2000,
            },
            AfterEventType::CollectionDeleted => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a collection is successfully deleted",
                category: EventCategory::SchemaOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 2000,
            },
            AfterEventType::UserRegistered => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a user is successfully registered",
                category: EventCategory::Authentication,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 2000,
            },
            AfterEventType::UserAuthenticated => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a user is successfully authenticated",
                category: EventCategory::Authentication,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::SystemStartup => EventTypeMetadata {
                name: self.name(),
                description: "Fired when the system starts up",
                category: EventCategory::SystemLifecycle,
                can_modify_data: false,
                is_system_critical: true,
                recommended_timeout_ms: 10000,
            },
            AfterEventType::SystemShutdown => EventTypeMetadata {
                name: self.name(),
                description: "Fired when the system shuts down",
                category: EventCategory::SystemLifecycle,
                can_modify_data: false,
                is_system_critical: true,
                recommended_timeout_ms: 30000,
            },
            AfterEventType::DatabaseConnected => EventTypeMetadata {
                name: self.name(),
                description: "Fired when database connection is established",
                category: EventCategory::SystemLifecycle,
                can_modify_data: false,
                is_system_critical: true,
                recommended_timeout_ms: 5000,
            },
            AfterEventType::DatabaseDisconnected => EventTypeMetadata {
                name: self.name(),
                description: "Fired when database connection is lost",
                category: EventCategory::SystemLifecycle,
                can_modify_data: false,
                is_system_critical: true,
                recommended_timeout_ms: 5000,
            },
            AfterEventType::PluginLoaded => EventTypeMetadata {
                name: self.name(),
                description: "Fired when a plugin is successfully loaded",
                category: EventCategory::PluginOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 3000,
            },
            AfterEventType::PluginUnloaded => EventTypeMetadata {
                name: self.name(),
                description: "Fired when a plugin is unloaded",
                category: EventCategory::PluginOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 3000,
            },
            AfterEventType::PluginError => EventTypeMetadata {
                name: self.name(),
                description: "Fired when a plugin encounters an error",
                category: EventCategory::PluginOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::ApiRequestProcessed => EventTypeMetadata {
                name: self.name(),
                description: "Fired after an API request is processed",
                category: EventCategory::ApiOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::FileWritten => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a file is successfully written to VFS",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::FileMoved => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a file is successfully moved within VFS",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::FileRead => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a file is successfully read from VFS",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 500,
            },
            AfterEventType::FileDeleted => EventTypeMetadata {
                name: self.name(),
                description: "Fired after a file is successfully deleted from VFS",
                category: EventCategory::DataOperation,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
            AfterEventType::ErrorOccurred => EventTypeMetadata {
                name: self.name(),
                description: "Fired when an error occurs in the system",
                category: EventCategory::SystemLifecycle,
                can_modify_data: false,
                is_system_critical: false,
                recommended_timeout_ms: 1000,
            },
        }
    }

    /// Get all available After event types
    pub fn all() -> Vec<AfterEventType> {
        vec![
            AfterEventType::RecordCreated,
            AfterEventType::RecordUpdated,
            AfterEventType::RecordDeleted,
            AfterEventType::RecordRead,
            AfterEventType::CollectionCreated,
            AfterEventType::CollectionUpdated,
            AfterEventType::CollectionDeleted,
            AfterEventType::UserRegistered,
            AfterEventType::UserAuthenticated,
            AfterEventType::SystemStartup,
            AfterEventType::SystemShutdown,
            AfterEventType::DatabaseConnected,
            AfterEventType::DatabaseDisconnected,
            AfterEventType::PluginLoaded,
            AfterEventType::PluginUnloaded,
            AfterEventType::PluginError,
            AfterEventType::ApiRequestProcessed,
            AfterEventType::FileWritten,
            AfterEventType::FileMoved,
            AfterEventType::FileRead,
            AfterEventType::FileDeleted,
            AfterEventType::ErrorOccurred,
        ]
    }
}

/// Categories for grouping event types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EventCategory {
    DataOperation,
    SchemaOperation,
    Authentication,
    ApiOperation,
    PluginOperation,
    SystemLifecycle,
}

/// Metadata about an event type for introspection and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTypeMetadata {
    /// Human-readable name of the event
    pub name: &'static str,
    /// Description of when this event is fired and what it does
    pub description: &'static str,
    /// Category this event belongs to
    pub category: EventCategory,
    /// Whether handlers for this event can modify data
    pub can_modify_data: bool,
    /// Whether this event is critical for system operation
    pub is_system_critical: bool,
    /// Recommended timeout for handlers of this event type (in milliseconds)
    pub recommended_timeout_ms: u64,
}

/// Priority levels for event processing
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EventPriority {
    Low = 0,
    #[default]
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl EventPriority {
    /// Get all priority levels in order from lowest to highest
    pub fn all() -> Vec<EventPriority> {
        vec![
            EventPriority::Low,
            EventPriority::Normal,
            EventPriority::High,
            EventPriority::Critical,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_metadata() {
        let metadata = BeforeEventType::RecordCreate.metadata();
        assert_eq!(metadata.name, "BeforeRecordCreate");
        assert!(metadata.can_modify_data);
        assert_eq!(metadata.category, EventCategory::DataOperation);
    }

    #[test]
    fn test_all_before_events_have_unique_names() {
        let events = BeforeEventType::all();
        let names: std::collections::HashSet<_> = events.iter().map(|e| e.name()).collect();
        assert_eq!(
            events.len(),
            names.len(),
            "All Before event names should be unique"
        );
    }

    #[test]
    fn test_all_after_events_have_unique_names() {
        let events = AfterEventType::all();
        let names: std::collections::HashSet<_> = events.iter().map(|e| e.name()).collect();
        assert_eq!(
            events.len(),
            names.len(),
            "All After event names should be unique"
        );
    }

    #[test]
    fn test_event_priority_ordering() {
        assert!(EventPriority::Critical > EventPriority::High);
        assert!(EventPriority::High > EventPriority::Normal);
        assert!(EventPriority::Normal > EventPriority::Low);
    }

    #[test]
    fn test_event_type_serialization() {
        let event = BeforeEventType::RecordCreate;
        let serialized = serde_json::to_string(&event).unwrap();
        let deserialized: BeforeEventType = serde_json::from_str(&serialized).unwrap();
        assert_eq!(event, deserialized);
    }
}
