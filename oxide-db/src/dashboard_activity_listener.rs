//! Dashboard Activity Listener
//!
//! This module provides an event listener that automatically records dashboard
//! activities based on system events. It follows the Hook-First Principle by
//! listening to events dispatched by the EventBus.

use chrono::Utc;
use std::sync::Arc;
use tracing::{debug, warn};

use oxide_core::{
    event::{AfterEventContext, AfterEventHandler, HandlerMetadata},
    ActivityEntry, ActivityType, AppError, DashboardStatsService,
};

/// Dashboard activity listener that records activities from system events
pub struct DashboardActivityListener {
    /// Dashboard stats service for recording activities
    dashboard_service: Arc<dyn DashboardStatsService>,
}

impl DashboardActivityListener {
    /// Create a new dashboard activity listener
    pub fn new(dashboard_service: Arc<dyn DashboardStatsService>) -> Self {
        Self { dashboard_service }
    }

    /// Convert an after event context to an activity entry
    fn after_event_to_activity(&self, context: &AfterEventContext) -> Option<ActivityEntry> {
        let timestamp = Utc::now().to_rfc3339();

        match context {
            AfterEventContext::RecordCreated {
                collection,
                record_id,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::RecordCreated,
                    user,
                    description: format!(
                        "Created record {} in collection '{}'",
                        record_id, collection
                    ),
                    collection: Some(collection.clone()),
                    metadata: Some(serde_json::json!({
                        "record_id": record_id,
                        "operation": "create"
                    })),
                })
            }
            AfterEventContext::RecordUpdated {
                collection,
                record_id,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::RecordUpdated,
                    user,
                    description: format!(
                        "Updated record {} in collection '{}'",
                        record_id, collection
                    ),
                    collection: Some(collection.clone()),
                    metadata: Some(serde_json::json!({
                        "record_id": record_id,
                        "operation": "update"
                    })),
                })
            }
            AfterEventContext::RecordDeleted {
                collection,
                record_id,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::RecordDeleted,
                    user,
                    description: format!(
                        "Deleted record {} from collection '{}'",
                        record_id, collection
                    ),
                    collection: Some(collection.clone()),
                    metadata: Some(serde_json::json!({
                        "record_id": record_id,
                        "operation": "delete"
                    })),
                })
            }
            AfterEventContext::CollectionCreated {
                collection,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::CollectionCreated,
                    user,
                    description: format!("Created collection '{}'", collection),
                    collection: Some(collection.clone()),
                    metadata: Some(serde_json::json!({
                        "operation": "create_collection"
                    })),
                })
            }
            AfterEventContext::CollectionUpdated {
                collection,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::CollectionModified,
                    user,
                    description: format!("Modified schema for collection '{}'", collection),
                    collection: Some(collection.clone()),
                    metadata: Some(serde_json::json!({
                        "operation": "update_schema"
                    })),
                })
            }
            AfterEventContext::CollectionDeleted {
                collection,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::CollectionDeleted,
                    user,
                    description: format!("Deleted collection '{}'", collection),
                    collection: Some(collection.clone()),
                    metadata: Some(serde_json::json!({
                        "operation": "delete_collection"
                    })),
                })
            }
            AfterEventContext::UserRegistered { user_id, email, .. } => Some(ActivityEntry {
                timestamp,
                activity_type: ActivityType::UserRegistered,
                user: "system".to_string(),
                description: format!("New user '{}' registered with email '{}'", user_id, email),
                collection: Some("_users".to_string()),
                metadata: Some(serde_json::json!({
                    "user_id": user_id,
                    "email": email,
                    "operation": "user_register"
                })),
            }),
            AfterEventContext::UserAuthenticated { user_id, email, .. } => Some(ActivityEntry {
                timestamp,
                activity_type: ActivityType::UserLogin,
                user: user_id.clone(),
                description: format!("User '{}' logged in", email),
                collection: None,
                metadata: Some(serde_json::json!({
                    "user_id": user_id,
                    "email": email,
                    "operation": "user_login"
                })),
            }),
            AfterEventContext::PluginLoaded {
                plugin_name,
                plugin_version,
                ..
            } => Some(ActivityEntry {
                timestamp,
                activity_type: ActivityType::PluginInstalled,
                user: "system".to_string(),
                description: format!("Plugin '{}' v{} loaded", plugin_name, plugin_version),
                collection: None,
                metadata: Some(serde_json::json!({
                    "plugin_name": plugin_name,
                    "plugin_version": plugin_version,
                    "operation": "plugin_loaded"
                })),
            }),
            AfterEventContext::PluginUnloaded { plugin_name, .. } => Some(ActivityEntry {
                timestamp,
                activity_type: ActivityType::PluginToggled,
                user: "system".to_string(),
                description: format!("Plugin '{}' unloaded", plugin_name),
                collection: None,
                metadata: Some(serde_json::json!({
                    "plugin_name": plugin_name,
                    "operation": "plugin_unloaded"
                })),
            }),
            AfterEventContext::SystemStartup { version, .. } => Some(ActivityEntry {
                timestamp,
                activity_type: ActivityType::SystemMaintenance,
                user: "system".to_string(),
                description: format!("System started up (version {})", version),
                collection: None,
                metadata: Some(serde_json::json!({
                    "version": version,
                    "operation": "system_startup"
                })),
            }),
            AfterEventContext::SystemShutdown {
                reason, uptime_ms, ..
            } => Some(ActivityEntry {
                timestamp,
                activity_type: ActivityType::SystemMaintenance,
                user: "system".to_string(),
                description: format!("System shutdown: {} (uptime: {}ms)", reason, uptime_ms),
                collection: None,
                metadata: Some(serde_json::json!({
                    "reason": reason,
                    "uptime_ms": uptime_ms,
                    "operation": "system_shutdown"
                })),
            }),
            AfterEventContext::FileDeleted {
                namespace,
                file_id,
                path,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::Other("file_deleted".to_string()),
                    user,
                    description: format!("Deleted file '{}' from namespace '{}'", path, namespace),
                    collection: None,
                    metadata: Some(serde_json::json!({
                        "file_id": file_id,
                        "path": path,
                        "namespace": namespace,
                        "operation": "file_delete"
                    })),
                })
            }
            AfterEventContext::FileMoved {
                namespace,
                file_id,
                old_path,
                new_path,
                overwritten_file_id,
                request_context,
                ..
            } => {
                let user = request_context
                    .user_id
                    .clone()
                    .unwrap_or_else(|| "system".to_string());
                Some(ActivityEntry {
                    timestamp,
                    activity_type: ActivityType::Other("file_moved".to_string()),
                    user,
                    description: format!(
                        "Moved file '{}' to '{}' in namespace '{}'",
                        old_path, new_path, namespace
                    ),
                    collection: None,
                    metadata: Some(serde_json::json!({
                        "file_id": file_id,
                        "old_path": old_path,
                        "new_path": new_path,
                        "namespace": namespace,
                        "overwritten_file_id": overwritten_file_id,
                        "operation": "file_move"
                    })),
                })
            }
            AfterEventContext::ErrorOccurred {
                error_type,
                message,
                severity,
                ..
            } => Some(ActivityEntry {
                timestamp,
                activity_type: ActivityType::Other("error_occurred".to_string()),
                user: "system".to_string(),
                description: format!("Error occurred: {} - {}", error_type, message),
                collection: None,
                metadata: Some(serde_json::json!({
                    "error_type": error_type,
                    "message": message,
                    "severity": format!("{:?}", severity),
                    "operation": "error"
                })),
            }),
            // Skip read events and other non-activity events
            AfterEventContext::RecordRead { .. }
            | AfterEventContext::DatabaseConnected { .. }
            | AfterEventContext::DatabaseDisconnected { .. }
            | AfterEventContext::PluginError { .. }
            | AfterEventContext::ApiRequestProcessed { .. }
            | AfterEventContext::FileWritten { .. }
            | AfterEventContext::FileRead { .. } => None,
        }
    }

    /// Create an after event handler function
    pub async fn handle_after_event(&self, context: &AfterEventContext) -> Result<(), AppError> {
        debug!("Dashboard activity listener handling after event");

        // Convert event to activity entry
        if let Some(activity) = self.after_event_to_activity(context) {
            // Record the activity
            match self.dashboard_service.record_activity(activity).await {
                Ok(_) => {
                    debug!("Successfully recorded dashboard activity");
                }
                Err(e) => {
                    warn!("Failed to record dashboard activity: {}", e);
                    // Don't fail the event processing for dashboard recording failures
                }
            }
        } else {
            debug!("No activity recorded for this event type");
        }

        Ok(())
    }

    /// Create a handler function for the event bus
    pub fn create_handler(self: Arc<Self>) -> AfterEventHandler {
        Arc::new(move |context| {
            let listener = Arc::clone(&self);
            Box::pin(async move { listener.handle_after_event(context).await })
        })
    }

    /// Create handler metadata
    pub fn create_metadata() -> HandlerMetadata {
        HandlerMetadata::new(
            "dashboard_activity_listener".to_string(),
            "Dashboard Activity Listener".to_string(),
        )
        .with_description("Records user and system activities for the dashboard".to_string())
        .with_priority(10) // Low priority since it's for display purposes
    }
}

/// Helper function to register the dashboard activity listener with the event bus
pub async fn register_dashboard_activity_listener(
    event_bus: &Arc<dyn oxide_core::event::EventBus>,
    dashboard_service: Arc<dyn DashboardStatsService>,
) -> Result<(), AppError> {
    let listener = Arc::new(DashboardActivityListener::new(dashboard_service));
    let handler = listener.create_handler();
    let _base_metadata = DashboardActivityListener::create_metadata();

    // Register for specific after events that we want to track
    // (instead of wildcard "*" which isn't supported by the event bus)
    let events_to_track = [
        oxide_core::event::types::AfterEventType::RecordCreated,
        oxide_core::event::types::AfterEventType::RecordUpdated,
        oxide_core::event::types::AfterEventType::RecordDeleted,
        oxide_core::event::types::AfterEventType::CollectionCreated,
        oxide_core::event::types::AfterEventType::CollectionUpdated,
        oxide_core::event::types::AfterEventType::CollectionDeleted,
        oxide_core::event::types::AfterEventType::UserRegistered,
        oxide_core::event::types::AfterEventType::UserAuthenticated,
        oxide_core::event::types::AfterEventType::PluginLoaded,
        oxide_core::event::types::AfterEventType::PluginUnloaded,
        oxide_core::event::types::AfterEventType::FileWritten,
        oxide_core::event::types::AfterEventType::FileDeleted,
        oxide_core::event::types::AfterEventType::ErrorOccurred,
    ];

    for (i, event_type) in events_to_track.iter().enumerate() {
        // Create unique metadata for each subscription
        let metadata = oxide_core::event::handlers::HandlerMetadata::new(
            format!("dashboard_activity_listener_{}", i),
            "Dashboard Activity Listener".to_string(),
        )
        .with_description("Records user and system activities for the dashboard".to_string())
        .with_priority(10); // Low priority since it's for display purposes

        // Clone the handler for each subscription
        let handler_clone = Arc::clone(&handler);

        event_bus
            .subscribe_after(event_type.name(), handler_clone, metadata)
            .await?;
        debug!(
            "Dashboard activity listener registered for event: {}",
            event_type.name()
        );
    }

    debug!(
        "Dashboard activity listener registered successfully for {} event types",
        events_to_track.len()
    );
    Ok(())
}
