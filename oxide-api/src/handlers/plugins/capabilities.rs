//! Plugin capabilities and trust level management

use axum::{
    extract::{Path, State},
    Json,
};
use tracing::{info, warn};

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
};
use super::types::*;
use oxide_core::{
    plugin_security::{PluginCapability, PluginTrustLevel},
    auth::CrudOperation,
};

/// Grant a capability to a plugin
pub async fn grant_plugin_capability(
    State(state): State<AppState>,
    Path((plugin_name, capability_name)): Path<(String, String)>,
    Json(capability_config): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<Vec<PluginCapability>>>, ApiError> {
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Parse capability from name and config
    let capability = parse_capability_from_request(&capability_name, capability_config)?;

    // Update database configuration
    state.plugin_config_service.add_plugin_capability(&plugin_name, capability.clone()).await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    // Grant capability in runtime
    {
        let mut runtime_guard = plugin_manager.runtime.lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.grant_plugin_capability(&plugin_name, capability.clone())
            .map_err(|e| ApiError::internal(format!("Failed to grant capability: {}", e)))?;
    }

    // Get updated capabilities from database
    let config = state.plugin_config_service.get_plugin_config(&plugin_name).await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;
    
    info!("✅ Granted capability {:?} to plugin '{}' and saved to database", capability, plugin_name);
    
    Ok(Json(ApiResponse::success(config.capabilities)))
}

/// Revoke a capability from a plugin
pub async fn revoke_plugin_capability(
    State(state): State<AppState>,
    Path((plugin_name, capability_name)): Path<(String, String)>,
    Json(capability_config): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<Vec<PluginCapability>>>, ApiError> {
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Parse capability from name and config
    let capability = parse_capability_from_request(&capability_name, capability_config)?;

    // Update database configuration
    state.plugin_config_service.remove_plugin_capability(&plugin_name, &capability).await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    // Revoke capability in runtime
    {
        let mut runtime_guard = plugin_manager.runtime.lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.revoke_plugin_capability(&plugin_name, &capability)
            .map_err(|e| ApiError::internal(format!("Failed to revoke capability: {}", e)))?;
    }

    // Get updated capabilities from database
    let config = state.plugin_config_service.get_plugin_config(&plugin_name).await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;
    
    info!("✅ Revoked capability {:?} from plugin '{}' and saved to database", capability, plugin_name);
    
    Ok(Json(ApiResponse::success(config.capabilities)))
}

/// Update plugin trust level
pub async fn update_plugin_trust_level(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
    Json(request): Json<UpdateTrustLevelRequest>,
) -> Result<Json<ApiResponse<PluginTrustLevel>>, ApiError> {
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Note: Currently the plugin runtime doesn't support changing trust levels after loading
    // This would require reloading the plugin with new trust level
    warn!("Trust level update requested for plugin '{}', but runtime doesn't support dynamic trust level changes", plugin_name);
    
    Err(ApiError::bad_request("Dynamic trust level updates not yet supported. Please unregister and re-register the plugin with the new trust level.".to_string()))
}

/// Parse a capability from request data
fn parse_capability_from_request(capability_name: &str, config: serde_json::Value) -> Result<PluginCapability, ApiError> {
    match capability_name {
        "LogInfo" => Ok(PluginCapability::LogInfo),
        "LogError" => Ok(PluginCapability::LogError),
        "ReadEventData" => Ok(PluginCapability::ReadEventData),
        "ModifyEventData" => Ok(PluginCapability::ModifyEventData),
        "BlockOperations" => Ok(PluginCapability::BlockOperations),
        "AccessCollection" => {
            let collection = config.get("collection")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ApiError::bad_request("Missing collection field for AccessCollection capability".to_string()))?;
            let operations = config.get("operations")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_else(|| vec![CrudOperation::Read]);
            Ok(PluginCapability::AccessCollection {
                collection: collection.to_string(),
                operations,
            })
        }
        "RegisterHttpRoutes" => {
            let path_patterns = config.get("path_patterns")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_else(|| vec!["*".to_string()]);
            let methods = config.get("methods")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_else(|| vec!["GET".to_string(), "POST".to_string()]);
            Ok(PluginCapability::RegisterHttpRoutes {
                path_patterns,
                methods,
            })
        }
        "HandleHttpRequests" => Ok(PluginCapability::HandleHttpRequests),
        _ => Err(ApiError::bad_request(format!("Unknown capability: {}", capability_name))),
    }
}

/// Parse a capability string into a PluginCapability enum
pub fn parse_capability_string(cap_str: &str) -> Result<PluginCapability, ApiError> {
    match cap_str {
        "LogInfo" => Ok(PluginCapability::LogInfo),
        "LogError" => Ok(PluginCapability::LogError),
        "ReadEventData" => Ok(PluginCapability::ReadEventData),
        "ModifyEventData" => Ok(PluginCapability::ModifyEventData),
        "BlockOperations" => Ok(PluginCapability::BlockOperations),
        "HandleHttpRequests" => Ok(PluginCapability::HandleHttpRequests),
        s if s.starts_with("AccessCollection(") => {
            // Parse AccessCollection capability
            // Format: AccessCollection(collection="users", operations=["Read", "Create"])
            // For now, provide a default implementation
            Ok(PluginCapability::AccessCollection {
                collection: "default".to_string(),
                operations: vec![CrudOperation::Read],
            })
        }
        s if s.starts_with("RegisterHttpRoutes(") => {
            // Parse RegisterHttpRoutes capability
            // For now, provide a default implementation
            Ok(PluginCapability::RegisterHttpRoutes {
                path_patterns: vec!["*".to_string()],
                methods: vec!["GET".to_string(), "POST".to_string()],
            })
        }
        s if s.starts_with("CreateRecords(") => {
            Ok(PluginCapability::CreateRecords {
                collections: vec!["*".to_string()],
            })
        }
        s if s.starts_with("ReadRecords(") => {
            Ok(PluginCapability::ReadRecords {
                collections: vec!["*".to_string()],
            })
        }
        s if s.starts_with("UpdateRecords(") => {
            Ok(PluginCapability::UpdateRecords {
                collections: vec!["*".to_string()],
            })
        }
        s if s.starts_with("DeleteRecords(") => {
            Ok(PluginCapability::DeleteRecords {
                collections: vec!["*".to_string()],
            })
        }
        _ => Err(ApiError::bad_request(format!("Unknown capability: {}", cap_str))),
    }
}

/// Parse a trust level string into a PluginTrustLevel enum
pub fn parse_trust_level_string(trust_str: &str) -> Result<PluginTrustLevel, ApiError> {
    match trust_str.to_lowercase().as_str() {
        "untrusted" => Ok(PluginTrustLevel::Untrusted),
        "partiallytrusted" | "partially_trusted" => Ok(PluginTrustLevel::PartiallyTrusted),
        "fullytrusted" | "fully_trusted" => Ok(PluginTrustLevel::FullyTrusted),
        "system" => Ok(PluginTrustLevel::System),
        _ => Err(ApiError::bad_request(format!("Unknown trust level: {}", trust_str))),
    }
} 