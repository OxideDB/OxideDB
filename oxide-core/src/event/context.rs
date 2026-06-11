//! Event Context Definitions
//!
//! This module defines the context objects that are passed to event handlers,
//! containing all the data and metadata needed for event processing.

use crate::AppError;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::types::{Collection, EventPriority, RecordData, RecordId};

/// Context for Before events that allows data modification
#[derive(Debug, Clone)]
pub struct BeforeEventContext {
    /// Unique identifier for this event instance
    pub event_id: String,
    /// When this event was created
    pub timestamp: u64,
    /// Priority level for this event
    pub priority: EventPriority,
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
    /// Request context (user ID, IP, etc.)
    pub request_context: RequestContext,
    /// Tags for event filtering and routing
    pub tags: HashMap<String, String>,
    /// Whether this event should be persisted for replay
    pub should_persist: bool,
}

impl BeforeEventContext {
    /// Create a new context for record creation
    pub fn new_create(collection: Collection, data: RecordData) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::Normal,
            collection,
            data,
            metadata: JsonValue::Object(serde_json::Map::new()),
            record_id: None,
            old_data: None,
            request_context: RequestContext::default(),
            tags: HashMap::new(),
            should_persist: false,
        }
    }

    /// Create a new context for record update
    pub fn new_update(
        collection: Collection,
        record_id: RecordId,
        old_data: RecordData,
        new_data: RecordData,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::Normal,
            collection,
            data: new_data,
            metadata: JsonValue::Object(serde_json::Map::new()),
            record_id: Some(record_id),
            old_data: Some(old_data),
            request_context: RequestContext::default(),
            tags: HashMap::new(),
            should_persist: false,
        }
    }

    /// Create a new context for record deletion
    pub fn new_delete(collection: Collection, record_id: RecordId, data: RecordData) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::High, // Deletions are higher priority
            collection,
            data,
            metadata: JsonValue::Object(serde_json::Map::new()),
            record_id: Some(record_id),
            old_data: None,
            request_context: RequestContext::default(),
            tags: HashMap::new(),
            should_persist: true, // Always persist deletion events
        }
    }

    /// Create a new context for record read
    pub fn new_read(collection: Collection, record_id: RecordId, data: RecordData) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::Low, // Reads are lower priority
            collection,
            data,
            metadata: JsonValue::Object(serde_json::Map::new()),
            record_id: Some(record_id),
            old_data: None,
            request_context: RequestContext::default(),
            tags: HashMap::new(),
            should_persist: false,
        }
    }

    /// Create a new context for collection schema update
    pub fn new_collection_update(
        collection: Collection,
        old_schema: JsonValue,
        new_schema: JsonValue,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::High, // Schema changes are high priority
            collection,
            data: new_schema.clone(),
            metadata: serde_json::json!({
                "operation": "schema_update",
                "old_schema": old_schema,
                "new_schema": new_schema
            }),
            record_id: None,
            old_data: Some(old_schema),
            request_context: RequestContext::default(),
            tags: HashMap::new(),
            should_persist: true, // Always persist schema changes
        }
    }

    /// Set the priority of this event
    pub fn with_priority(mut self, priority: EventPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the request context
    pub fn with_request_context(mut self, context: RequestContext) -> Self {
        self.request_context = context;
        self
    }

    /// Add a tag for filtering and routing
    pub fn with_tag(mut self, key: String, value: String) -> Self {
        self.tags.insert(key, value);
        self
    }

    /// Enable persistence for this event
    pub fn with_persistence(mut self) -> Self {
        self.should_persist = true;
        self
    }

    /// Set a metadata value
    pub fn set_metadata(&mut self, key: &str, value: JsonValue) -> Result<(), AppError> {
        if let Some(obj) = self.metadata.as_object_mut() {
            obj.insert(key.to_string(), value);
            Ok(())
        } else {
            Err(AppError::internal("Metadata is not an object"))
        }
    }

    /// Get a metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&JsonValue> {
        self.metadata.get(key)
    }

    /// Check if a tag exists with the given key and value
    pub fn has_tag(&self, key: &str, value: &str) -> bool {
        self.tags.get(key).map(|v| v == value).unwrap_or(false)
    }

    /// Get the age of this event in milliseconds
    pub fn age_ms(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(self.timestamp)
    }

    /// Create a new VFS file write context
    pub fn new_vfs_write(
        namespace: String,
        path: String,
        content: Vec<u8>,
        mime_type: Option<String>,
    ) -> Self {
        let file_data = serde_json::json!({
            "path": path,
            "size": content.len(),
            "mime_type": mime_type.unwrap_or_else(|| "application/octet-stream".to_string()),
            "content_size": content.len()
        });

        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::Normal,
            collection: format!("vfs:{}", namespace),
            data: file_data,
            metadata: serde_json::json!({
                "namespace": namespace,
                "operation": "write"
            }),
            record_id: None,
            old_data: None,
            request_context: RequestContext::anonymous(),
            tags: HashMap::new(),
            should_persist: false,
        }
    }

    /// Create a new VFS file read context
    pub fn new_vfs_read(namespace: String, file_id: String, path: String) -> Self {
        let file_data = serde_json::json!({
            "file_id": file_id,
            "path": path
        });

        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::Normal,
            collection: format!("vfs:{}", namespace),
            data: file_data,
            metadata: serde_json::json!({
                "namespace": namespace,
                "operation": "read"
            }),
            record_id: Some(file_id),
            old_data: None,
            request_context: RequestContext::anonymous(),
            tags: HashMap::new(),
            should_persist: false,
        }
    }

    /// Create a new VFS file delete context
    pub fn new_vfs_delete(namespace: String, file_id: String, path: String) -> Self {
        let file_data = serde_json::json!({
            "file_id": file_id,
            "path": path
        });

        Self {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            priority: EventPriority::Normal,
            collection: format!("vfs:{}", namespace),
            data: file_data,
            metadata: serde_json::json!({
                "namespace": namespace,
                "operation": "delete"
            }),
            record_id: Some(file_id),
            old_data: None,
            request_context: RequestContext::anonymous(),
            tags: HashMap::new(),
            should_persist: false,
        }
    }
}

/// After event context for read-only event notifications
#[derive(Debug, Clone)]
pub enum AfterEventContext {
    RecordCreated {
        event_id: String,
        timestamp: u64,
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
        request_context: RequestContext,
    },
    RecordUpdated {
        event_id: String,
        timestamp: u64,
        collection: Collection,
        record_id: RecordId,
        old_data: RecordData,
        new_data: RecordData,
        request_context: RequestContext,
    },
    RecordDeleted {
        event_id: String,
        timestamp: u64,
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
        request_context: RequestContext,
    },
    RecordRead {
        event_id: String,
        timestamp: u64,
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
        request_context: RequestContext,
    },
    CollectionCreated {
        event_id: String,
        timestamp: u64,
        collection: Collection,
        schema: JsonValue,
        request_context: RequestContext,
    },
    CollectionUpdated {
        event_id: String,
        timestamp: u64,
        collection: Collection,
        old_schema: JsonValue,
        new_schema: JsonValue,
        request_context: RequestContext,
    },
    CollectionDeleted {
        event_id: String,
        timestamp: u64,
        collection: Collection,
        request_context: RequestContext,
    },
    UserRegistered {
        event_id: String,
        timestamp: u64,
        user_id: String,
        email: String,
        metadata: JsonValue,
        request_context: RequestContext,
    },
    UserAuthenticated {
        event_id: String,
        timestamp: u64,
        user_id: String,
        email: String,
        request_context: RequestContext,
    },
    SystemStartup {
        event_id: String,
        timestamp: u64,
        version: String,
        config: JsonValue,
    },
    SystemShutdown {
        event_id: String,
        timestamp: u64,
        reason: String,
        uptime_ms: u64,
    },
    DatabaseConnected {
        event_id: String,
        timestamp: u64,
        database_url: String,
        connection_pool_size: usize,
    },
    DatabaseDisconnected {
        event_id: String,
        timestamp: u64,
        error: Option<String>,
    },
    PluginLoaded {
        event_id: String,
        timestamp: u64,
        plugin_name: String,
        plugin_version: String,
        capabilities: JsonValue,
    },
    PluginUnloaded {
        event_id: String,
        timestamp: u64,
        plugin_name: String,
        reason: String,
    },
    PluginError {
        event_id: String,
        timestamp: u64,
        plugin_name: String,
        error_type: String,
        error_message: String,
        context: JsonValue,
    },
    ApiRequestProcessed {
        event_id: String,
        timestamp: u64,
        method: String,
        path: String,
        status_code: u16,
        response_time_ms: f64,
        request_context: RequestContext,
    },
    FileWritten {
        event_id: String,
        timestamp: u64,
        namespace: String,
        file_id: String,
        path: String,
        size: u64,
        mime_type: String,
        content_hash: String,
        request_context: RequestContext,
    },
    FileRead {
        event_id: String,
        timestamp: u64,
        namespace: String,
        file_id: String,
        path: String,
        size: u64,
        include_content: bool,
        request_context: RequestContext,
    },
    FileDeleted {
        event_id: String,
        timestamp: u64,
        namespace: String,
        file_id: String,
        path: String,
        request_context: RequestContext,
    },
    ErrorOccurred {
        event_id: String,
        timestamp: u64,
        error_type: String,
        message: String,
        context: JsonValue,
        severity: ErrorSeverity,
    },
}

impl AfterEventContext {
    /// Get the event ID for any After event
    pub fn event_id(&self) -> &str {
        match self {
            AfterEventContext::RecordCreated { event_id, .. } => event_id,
            AfterEventContext::RecordUpdated { event_id, .. } => event_id,
            AfterEventContext::RecordDeleted { event_id, .. } => event_id,
            AfterEventContext::RecordRead { event_id, .. } => event_id,
            AfterEventContext::CollectionCreated { event_id, .. } => event_id,
            AfterEventContext::CollectionUpdated { event_id, .. } => event_id,
            AfterEventContext::CollectionDeleted { event_id, .. } => event_id,
            AfterEventContext::UserRegistered { event_id, .. } => event_id,
            AfterEventContext::UserAuthenticated { event_id, .. } => event_id,
            AfterEventContext::SystemStartup { event_id, .. } => event_id,
            AfterEventContext::SystemShutdown { event_id, .. } => event_id,
            AfterEventContext::DatabaseConnected { event_id, .. } => event_id,
            AfterEventContext::DatabaseDisconnected { event_id, .. } => event_id,
            AfterEventContext::PluginLoaded { event_id, .. } => event_id,
            AfterEventContext::PluginUnloaded { event_id, .. } => event_id,
            AfterEventContext::PluginError { event_id, .. } => event_id,
            AfterEventContext::ApiRequestProcessed { event_id, .. } => event_id,
            AfterEventContext::FileWritten { event_id, .. } => event_id,
            AfterEventContext::FileRead { event_id, .. } => event_id,
            AfterEventContext::FileDeleted { event_id, .. } => event_id,
            AfterEventContext::ErrorOccurred { event_id, .. } => event_id,
        }
    }

    /// Get the timestamp for any After event
    pub fn timestamp(&self) -> u64 {
        match self {
            AfterEventContext::RecordCreated { timestamp, .. } => *timestamp,
            AfterEventContext::RecordUpdated { timestamp, .. } => *timestamp,
            AfterEventContext::RecordDeleted { timestamp, .. } => *timestamp,
            AfterEventContext::RecordRead { timestamp, .. } => *timestamp,
            AfterEventContext::CollectionCreated { timestamp, .. } => *timestamp,
            AfterEventContext::CollectionUpdated { timestamp, .. } => *timestamp,
            AfterEventContext::CollectionDeleted { timestamp, .. } => *timestamp,
            AfterEventContext::UserRegistered { timestamp, .. } => *timestamp,
            AfterEventContext::UserAuthenticated { timestamp, .. } => *timestamp,
            AfterEventContext::SystemStartup { timestamp, .. } => *timestamp,
            AfterEventContext::SystemShutdown { timestamp, .. } => *timestamp,
            AfterEventContext::DatabaseConnected { timestamp, .. } => *timestamp,
            AfterEventContext::DatabaseDisconnected { timestamp, .. } => *timestamp,
            AfterEventContext::PluginLoaded { timestamp, .. } => *timestamp,
            AfterEventContext::PluginUnloaded { timestamp, .. } => *timestamp,
            AfterEventContext::PluginError { timestamp, .. } => *timestamp,
            AfterEventContext::ApiRequestProcessed { timestamp, .. } => *timestamp,
            AfterEventContext::FileWritten { timestamp, .. } => *timestamp,
            AfterEventContext::FileRead { timestamp, .. } => *timestamp,
            AfterEventContext::FileDeleted { timestamp, .. } => *timestamp,
            AfterEventContext::ErrorOccurred { timestamp, .. } => *timestamp,
        }
    }

    /// Get the age of this event in milliseconds
    pub fn age_ms(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        now.saturating_sub(self.timestamp())
    }

    /// Create a new RecordCreated event
    pub fn record_created(
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
        request_context: RequestContext,
    ) -> Self {
        AfterEventContext::RecordCreated {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            collection,
            record_id,
            data,
            request_context,
        }
    }

    /// Create a new RecordUpdated event
    pub fn record_updated(
        collection: Collection,
        record_id: RecordId,
        old_data: RecordData,
        new_data: RecordData,
        request_context: RequestContext,
    ) -> Self {
        AfterEventContext::RecordUpdated {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            collection,
            record_id,
            old_data,
            new_data,
            request_context,
        }
    }

    /// Create a new RecordDeleted event
    pub fn record_deleted(
        collection: Collection,
        record_id: RecordId,
        data: RecordData,
        request_context: RequestContext,
    ) -> Self {
        AfterEventContext::RecordDeleted {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            collection,
            record_id,
            data,
            request_context,
        }
    }

    /// Create a new CollectionUpdated event
    pub fn collection_updated(
        collection: Collection,
        old_schema: JsonValue,
        new_schema: JsonValue,
        request_context: RequestContext,
    ) -> Self {
        AfterEventContext::CollectionUpdated {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            collection,
            old_schema,
            new_schema,
            request_context,
        }
    }

    /// Create a new FileWritten event
    pub fn file_written(
        namespace: String,
        file_id: String,
        path: String,
        size: u64,
        mime_type: String,
        content_hash: String,
        request_context: RequestContext,
    ) -> Self {
        AfterEventContext::FileWritten {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            namespace,
            file_id,
            path,
            size,
            mime_type,
            content_hash,
            request_context,
        }
    }

    /// Create a new FileRead event
    pub fn file_read(
        namespace: String,
        file_id: String,
        path: String,
        size: u64,
        include_content: bool,
        request_context: RequestContext,
    ) -> Self {
        AfterEventContext::FileRead {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            namespace,
            file_id,
            path,
            size,
            include_content,
            request_context,
        }
    }

    /// Create a new FileDeleted event
    pub fn file_deleted(
        namespace: String,
        file_id: String,
        path: String,
        request_context: RequestContext,
    ) -> Self {
        AfterEventContext::FileDeleted {
            event_id: Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            namespace,
            file_id,
            path,
            request_context,
        }
    }
}

/// Request context information for events
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RequestContext {
    /// User ID if authenticated
    pub user_id: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Client IP address
    pub client_ip: Option<String>,
    /// User agent string
    pub user_agent: Option<String>,
    /// Request correlation ID
    pub correlation_id: Option<String>,
    /// API key used for the request
    pub api_key_id: Option<String>,
    /// Additional custom context
    pub custom: HashMap<String, String>,
}

impl RequestContext {
    /// Create a new RequestContext with user authentication
    pub fn authenticated(user_id: String) -> Self {
        Self {
            user_id: Some(user_id),
            ..Default::default()
        }
    }

    /// Create a new RequestContext for anonymous requests
    pub fn anonymous() -> Self {
        Self::default()
    }

    /// Add client information
    pub fn with_client_info(mut self, ip: String, user_agent: String) -> Self {
        self.client_ip = Some(ip);
        self.user_agent = Some(user_agent);
        self
    }

    /// Add correlation ID for request tracing
    pub fn with_correlation_id(mut self, correlation_id: String) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    /// Add custom context value
    pub fn with_custom(mut self, key: String, value: String) -> Self {
        self.custom.insert(key, value);
        self
    }
}

/// Error severity levels for error events
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Low,
    #[default]
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_before_context_creation() {
        let data = serde_json::json!({"name": "test"});
        let context = BeforeEventContext::new_create("users".to_string(), data.clone());

        assert_eq!(context.collection, "users");
        assert_eq!(context.data, data);
        assert_eq!(context.priority, EventPriority::Normal);
        assert!(context.record_id.is_none());
        assert!(context.old_data.is_none());
    }

    #[test]
    fn test_before_context_with_fluent_api() {
        let data = serde_json::json!({"name": "test"});
        let context = BeforeEventContext::new_create("users".to_string(), data)
            .with_priority(EventPriority::High)
            .with_tag("env".to_string(), "test".to_string())
            .with_persistence();

        assert_eq!(context.priority, EventPriority::High);
        assert!(context.has_tag("env", "test"));
        assert!(context.should_persist);
    }

    #[test]
    fn test_after_context_factory_methods() {
        let data = serde_json::json!({"name": "test"});
        let request_context = RequestContext::authenticated("user123".to_string());

        let context = AfterEventContext::record_created(
            "users".to_string(),
            "rec123".to_string(),
            data,
            request_context,
        );

        assert!(!context.event_id().is_empty());
        assert!(context.timestamp() > 0);
        assert!(context.age_ms() < 100); // Should be very recent
    }

    #[test]
    fn test_request_context_fluent_api() {
        let context = RequestContext::authenticated("user123".to_string())
            .with_client_info("192.168.1.1".to_string(), "Mozilla/5.0".to_string())
            .with_correlation_id("corr-123".to_string())
            .with_custom("source".to_string(), "web".to_string());

        assert_eq!(context.user_id.as_ref().unwrap(), "user123");
        assert_eq!(context.client_ip.as_ref().unwrap(), "192.168.1.1");
        assert_eq!(context.correlation_id.as_ref().unwrap(), "corr-123");
        assert_eq!(context.custom.get("source").unwrap(), "web");
    }

    #[test]
    fn test_metadata_operations() {
        let mut context = BeforeEventContext::new_create(
            "users".to_string(),
            serde_json::json!({"name": "test"}),
        );

        context
            .set_metadata("test_key", serde_json::json!("test_value"))
            .unwrap();
        assert_eq!(
            context.get_metadata("test_key").unwrap(),
            &serde_json::json!("test_value")
        );
    }
}
