//! Plugin audit functionality

use crate::{errors::ApiError, server::AppState};
use oxide_core::plugin_security::SecurityAuditEntry;
use oxide_logging::api::AuditQueryParams;

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
            event_type: Some("plugin_event".to_string()),
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
                        event_type: match event.event_type {
                            oxide_logging::models::AuditEventType::PluginEvent => {
                                oxide_core::plugin_security::SecurityEventType::PluginLoaded
                            }
                            oxide_logging::models::AuditEventType::SecurityViolation => {
                                oxide_core::plugin_security::SecurityEventType::SecurityViolation
                            }
                            oxide_logging::models::AuditEventType::ConfigurationChange => {
                                oxide_core::plugin_security::SecurityEventType::CapabilityGranted
                            }
                            _ => oxide_core::plugin_security::SecurityEventType::PluginLoaded,
                        },
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
