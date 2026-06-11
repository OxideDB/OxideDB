//! Plugin audit functionality

use crate::{errors::ApiError, server::AppState};
use oxide_core::{
    plugin_security::{SecurityAuditEntry, SecurityEventType},
    LogContext, SecurityAuditor,
};
use oxide_logging::api::AuditQueryParams;
use serde_json::json;
use tracing::warn;

/// Record a plugin lifecycle audit event when the logging system is available.
pub(super) async fn record_plugin_audit_event(
    state: &AppState,
    plugin_name: &str,
    action: &str,
    result: &str,
    metadata: serde_json::Value,
) {
    let Some(logging_service) = &state.logging_service else {
        return;
    };

    let mut context = LogContext::new()
        .with_collection("_plugins")
        .with_operation(action.to_string())
        .with_metadata("plugin_name", json!(plugin_name))
        .with_metadata("plugin_target", json!(format!("plugin:{}", plugin_name)));

    if let Some(metadata_object) = metadata.as_object() {
        for (key, value) in metadata_object {
            context.metadata.insert(key.clone(), value.clone());
        }
    } else if !metadata.is_null() {
        context
            .metadata
            .insert("metadata".to_string(), metadata.clone());
    }

    if let Err(error) = logging_service
        .log_plugin_event(
            plugin_name.to_string(),
            action.to_string(),
            result.to_string(),
            context,
        )
        .await
    {
        warn!(
            "Failed to record plugin audit event '{}' for '{}': {}",
            action, plugin_name, error
        );
    }
}

/// Get plugin audit log entries
pub async fn get_plugin_audit_log(
    state: &AppState,
    plugin_name: &str,
) -> Result<Vec<SecurityAuditEntry>, ApiError> {
    // Try to get audit log from logging service if available
    if let Some(ref logging_service) = state.logging_api_service {
        // Create query parameters for plugin-related audit events
        let params = AuditQueryParams {
            actor: Some(plugin_name.to_string()), // Filter by plugin name as actor
            event_type: None,
            limit: Some(100),
            offset: Some(0),
            severity: None,
            start_time: None,
            end_time: None,
            target: None,
            correlation_id: None,
            min_risk_score: None,
            sort: Some("desc".to_string()),
        };

        match logging_service.query_audit_events(params).await {
            Ok(response) => {
                // Convert SecurityAuditEvent to SecurityAuditEntry
                let audit_entries: Vec<SecurityAuditEntry> = response
                    .data
                    .into_iter()
                    .map(|event| SecurityAuditEntry {
                        timestamp: event.timestamp.timestamp() as u64,
                        plugin_name: plugin_name.to_string(),
                        event_type: security_event_type_from_audit_event(
                            event.event_type,
                            &event.action,
                        ),
                        details: serde_json::json!({
                            "id": event.id,
                            "description": event.description,
                            "action": event.action,
                            "result": event.result,
                            "target": event.target,
                            "severity": event.severity,
                            "risk_score": event.risk_score,
                            "integrity_hash": event.integrity_hash,
                            "context": event.context,
                        }),
                    })
                    .collect();
                Ok(audit_entries)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to query audit events for plugin {}: {}",
                    plugin_name,
                    e
                );
                // Fallback to empty audit log if logging service fails
                Ok(Vec::new())
            }
        }
    } else {
        // No logging service available
        Ok(Vec::new())
    }
}

fn security_event_type_from_audit_event(
    audit_event_type: oxide_logging::models::AuditEventType,
    action: &str,
) -> SecurityEventType {
    match audit_event_type {
        oxide_logging::models::AuditEventType::SecurityViolation => {
            SecurityEventType::SecurityViolation
        }
        oxide_logging::models::AuditEventType::ConfigurationChange => {
            configuration_action_to_security_event_type(action)
        }
        oxide_logging::models::AuditEventType::PluginEvent => {
            plugin_action_to_security_event_type(action)
        }
        _ => SecurityEventType::PluginLoaded,
    }
}

fn configuration_action_to_security_event_type(action: &str) -> SecurityEventType {
    match action {
        "capability_granted" => SecurityEventType::CapabilityGranted,
        "capability_revoked" => SecurityEventType::CapabilityRevoked,
        _ => SecurityEventType::CapabilityGranted,
    }
}

fn plugin_action_to_security_event_type(action: &str) -> SecurityEventType {
    match action {
        "plugin_installed" | "plugin_loaded" | "plugin_startup_loaded" => {
            SecurityEventType::PluginLoaded
        }
        "plugin_enabled" | "plugin_resumed" => SecurityEventType::PluginResumed,
        "plugin_disabled" | "plugin_suspended" => SecurityEventType::PluginSuspended,
        "plugin_uninstalled" | "plugin_unregistered" | "plugin_unloaded" => {
            SecurityEventType::PluginUnloaded
        }
        "plugin_install_failed"
        | "plugin_load_failed"
        | "plugin_resume_failed"
        | "plugin_suspend_failed"
        | "plugin_unregister_failed"
        | "plugin_unload_failed"
        | "plugin_uninstall_failed"
        | "plugin_registration_failed"
        | "plugin_signature_rejected"
        | "plugin_startup_load_failed"
        | "plugin_startup_registration_failed" => SecurityEventType::SecurityViolation,
        "plugin_resource_limit_exceeded" => SecurityEventType::ResourceLimitExceeded,
        _ => SecurityEventType::PluginLoaded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_logging::models::AuditEventType;

    #[test]
    fn plugin_audit_actions_map_to_security_event_types() {
        assert!(matches!(
            security_event_type_from_audit_event(AuditEventType::PluginEvent, "plugin_installed"),
            SecurityEventType::PluginLoaded
        ));
        assert!(matches!(
            security_event_type_from_audit_event(AuditEventType::PluginEvent, "plugin_enabled"),
            SecurityEventType::PluginResumed
        ));
        assert!(matches!(
            security_event_type_from_audit_event(AuditEventType::PluginEvent, "plugin_disabled"),
            SecurityEventType::PluginSuspended
        ));
        assert!(matches!(
            security_event_type_from_audit_event(AuditEventType::PluginEvent, "plugin_uninstalled"),
            SecurityEventType::PluginUnloaded
        ));
        assert!(matches!(
            security_event_type_from_audit_event(
                AuditEventType::PluginEvent,
                "plugin_install_failed"
            ),
            SecurityEventType::SecurityViolation
        ));
    }
}
