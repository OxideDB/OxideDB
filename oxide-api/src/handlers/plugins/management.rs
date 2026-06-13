//! Plugin management operations

use axum::{
    extract::{Path, State},
    Json,
};
use tracing::{info, warn};

use super::{
    audit::{get_plugin_audit_log, record_plugin_audit_event},
    permissions::get_plugin_permissions_info,
    types::*,
};
use crate::{errors::ApiError, responses::ApiResponse, server::AppState};
use oxide_core::{auth::PermissionService, plugin_config::PluginStatus as PersistedPluginStatus};

/// List all installed plugins with their status and details
pub async fn list_plugins(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PluginInfo>>>, ApiError> {
    // Get plugin configurations from database
    let plugin_configs = state
        .plugin_config_service
        .list_plugin_configs()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configurations: {}", e)))?;

    let mut plugins = Vec::new();
    let runtime_statistics = state
        .plugin_manager
        .as_ref()
        .and_then(|plugin_manager| plugin_manager.get_plugin_statistics().ok());

    for config in plugin_configs {
        let runtime_stat = runtime_statistics
            .as_ref()
            .and_then(|stats| stats.iter().find(|s| s.name == config.name));
        let is_loaded_in_runtime = runtime_stat.is_some();

        // Get runtime statistics if available
        let (executions, errors, last_execution, resource_usage) =
            if let Some(plugin_stat) = runtime_stat {
                (
                    plugin_stat.executions,
                    plugin_stat.errors,
                    timestamp_to_datetime(plugin_stat.last_execution),
                    runtime_resource_usage(plugin_stat),
                )
            } else {
                (0, 0, None, ResourceUsageInfo::default())
            };

        // Determine actual status based on database status and runtime state
        let actual_status = resolve_plugin_status(
            &config.name,
            &config.status,
            runtime_stat.map(|stat| &stat.status),
            is_loaded_in_runtime,
        );

        let plugin_info = PluginInfo {
            name: config.name.clone(),
            status: actual_status,
            version: config.version,
            description: config.description,
            author: config.author,
            capabilities: config.capabilities,
            trust_level: config.trust_level,
            routes: get_plugin_routes(&state, &config.name).await?,
            executions,
            errors,
            last_execution,
            resource_usage,
        };
        plugins.push(plugin_info);
    }

    Ok(Json(ApiResponse::success(plugins)))
}

/// Get detailed information about a specific plugin
pub async fn get_plugin_details(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<PluginDetails>>, ApiError> {
    // Get plugin configuration from database for complete metadata
    let plugin_config = state
        .plugin_config_service
        .get_plugin_config(&plugin_name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;

    let runtime_stat = state
        .plugin_manager
        .as_ref()
        .map(|plugin_manager| get_runtime_stat(plugin_manager, &plugin_name))
        .transpose()?
        .flatten();

    let capabilities = plugin_config.capabilities.clone();
    let trust_level = plugin_config.trust_level.clone();
    let routes = get_plugin_routes(&state, &plugin_name).await?;
    let audit_log = get_plugin_audit_log(&state, &plugin_name).await?;
    let resource_usage = runtime_stat
        .as_ref()
        .map(runtime_resource_usage)
        .unwrap_or_default();

    let details = PluginDetails {
        name: plugin_name.clone(),
        status: resolve_plugin_status(
            &plugin_name,
            &plugin_config.status,
            runtime_stat.as_ref().map(|stat| &stat.status),
            runtime_stat.is_some(),
        ),
        version: plugin_config.version,
        description: plugin_config.description,
        author: plugin_config.author,
        capabilities,
        trust_level,
        routes,
        executions: runtime_stat
            .as_ref()
            .map(|stat| stat.executions)
            .unwrap_or_default(),
        errors: runtime_stat
            .as_ref()
            .map(|stat| stat.errors)
            .unwrap_or_default(),
        last_execution: runtime_stat
            .as_ref()
            .and_then(|stat| timestamp_to_datetime(stat.last_execution)),
        resource_usage,
        audit_log,
        permissions: get_plugin_permissions_info(&state, &plugin_name).await?,
    };

    Ok(Json(ApiResponse::success(details)))
}

/// Enable a plugin
pub async fn enable_plugin(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<PluginStatus>>, ApiError> {
    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    state
        .plugin_config_service
        .update_plugin_status(&plugin_name, PersistedPluginStatus::Loading)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    if let Err(e) = activate_persisted_plugin(&state, &plugin_name).await {
        let _ = state
            .plugin_config_service
            .update_plugin_status(&plugin_name, PersistedPluginStatus::Error)
            .await;
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_load_failed",
            "failure",
            serde_json::json!({
                "source": "api",
                "reason": e.to_string(),
            }),
        )
        .await;
        return Err(e);
    }

    if let Err(e) = state
        .plugin_config_service
        .enable_plugin(&plugin_name)
        .await
    {
        if let Err(cleanup_error) = plugin_manager
            .unregister_and_unload_plugin(&state.event_bus, &plugin_name)
            .await
        {
            warn!(
                "Failed to roll back plugin '{}' after enable status update failure: {}",
                plugin_name, cleanup_error
            );
        }
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_enable_failed",
            "failure",
            serde_json::json!({
                "source": "api",
                "reason": e.to_string(),
            }),
        )
        .await;
        return Err(ApiError::internal(format!(
            "Failed to persist enabled plugin status for '{}': {}",
            plugin_name, e
        )));
    }

    record_plugin_audit_event(
        &state,
        &plugin_name,
        "plugin_enabled",
        "success",
        serde_json::json!({
            "source": "api",
        }),
    )
    .await;
    info!("✅ Plugin '{}' enabled and saved to database", plugin_name);
    Ok(Json(ApiResponse::success(PluginStatus::Enabled)))
}

/// Disable a plugin
pub async fn disable_plugin(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<PluginStatus>>, ApiError> {
    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    state
        .plugin_config_service
        .get_plugin_config(&plugin_name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;

    if let Err(e) = plugin_manager
        .unregister_and_unload_plugin(&state.event_bus, &plugin_name)
        .await
    {
        let _ = state
            .plugin_config_service
            .update_plugin_status(&plugin_name, PersistedPluginStatus::Error)
            .await;
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_deactivation_failed",
            "failure",
            serde_json::json!({
                "source": "api",
                "reason": e.to_string(),
            }),
        )
        .await;
        return Err(ApiError::internal(format!(
            "Failed to deactivate plugin '{}': {}",
            plugin_name, e
        )));
    }

    state
        .plugin_config_service
        .disable_plugin(&plugin_name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    record_plugin_audit_event(
        &state,
        &plugin_name,
        "plugin_disabled",
        "success",
        serde_json::json!({
            "source": "api",
            "reason": "Manually disabled",
        }),
    )
    .await;
    info!("✅ Plugin '{}' disabled and saved to database", plugin_name);
    Ok(Json(ApiResponse::success(PluginStatus::Disabled)))
}

/// Unregister/Uninstall a plugin
pub async fn unregister_plugin(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<String>>, ApiError> {
    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    if let Err(e) = plugin_manager
        .unregister_and_unload_plugin(&state.event_bus, &plugin_name)
        .await
    {
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_deactivation_failed",
            "failure",
            serde_json::json!({
                "source": "api",
                "reason": e.to_string(),
            }),
        )
        .await;
        return Err(ApiError::internal(format!(
            "Failed to deactivate plugin '{}': {}",
            plugin_name, e
        )));
    }

    // Remove from database and filesystem using the new service method
    if let Err(e) = state
        .plugin_config_service
        .uninstall_plugin(&plugin_name)
        .await
    {
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_uninstall_failed",
            "failure",
            serde_json::json!({
                "source": "api",
                "reason": e.to_string(),
            }),
        )
        .await;
        return Err(ApiError::internal(format!(
            "Failed to uninstall plugin: {}",
            e
        )));
    }

    record_plugin_audit_event(
        &state,
        &plugin_name,
        "plugin_uninstalled",
        "success",
        serde_json::json!({
            "source": "api",
        }),
    )
    .await;
    info!(
        "✅ Plugin '{}' unregistered and removed from database",
        plugin_name
    );
    Ok(Json(ApiResponse::success(format!(
        "Plugin '{}' unregistered successfully",
        plugin_name
    ))))
}

/// Load enabled plugins from database on startup
pub async fn load_plugins_from_database(state: &AppState) -> Result<(), ApiError> {
    info!("🔌 Loading plugins from database on startup");

    let plugin_manager = match state.plugin_manager.as_ref() {
        Some(pm) => pm,
        None => {
            warn!("Plugin manager not available, skipping plugin loading");
            return Ok(());
        }
    };

    // Get enabled plugins from database
    let enabled_plugins = state
        .plugin_config_service
        .get_enabled_plugins()
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get enabled plugins: {}", e)))?;

    let mut loaded_count = 0;
    let mut failed_count = 0;

    for config in enabled_plugins {
        info!("📦 Loading plugin: {} (v{})", config.name, config.version);

        // Get WASM data from filesystem
        let wasm_data = match state
            .plugin_config_service
            .load_plugin_wasm(&config.name)
            .await
        {
            Ok(data) => data,
            Err(e) => {
                tracing::error!(
                    "❌ Plugin '{}' WASM loading failed: {}, skipping",
                    config.name,
                    e
                );
                // Update status to error
                let _ = state
                    .plugin_config_service
                    .update_plugin_status(
                        &config.name,
                        oxide_core::plugin_config::PluginStatus::Error,
                    )
                    .await;
                record_plugin_audit_event(
                    state,
                    &config.name,
                    "plugin_startup_load_failed",
                    "failure",
                    serde_json::json!({
                        "source": "startup",
                        "version": config.version,
                        "reason": e.to_string(),
                    }),
                )
                .await;
                failed_count += 1;
                continue;
            }
        };

        match plugin_manager
            .load_and_register_plugin(&state.event_bus, &config, &wasm_data)
            .await
        {
            Ok(_) => {
                info!("✅ Successfully loaded plugin: {}", config.name);
                record_plugin_audit_event(
                    state,
                    &config.name,
                    "plugin_startup_loaded",
                    "success",
                    serde_json::json!({
                        "source": "startup",
                        "version": config.version,
                    }),
                )
                .await;
                loaded_count += 1;
            }
            Err(e) => {
                tracing::error!("❌ Failed to load plugin '{}': {}", config.name, e);
                // Update status to error
                let _ = state
                    .plugin_config_service
                    .update_plugin_status(
                        &config.name,
                        oxide_core::plugin_config::PluginStatus::Error,
                    )
                    .await;
                record_plugin_audit_event(
                    state,
                    &config.name,
                    "plugin_startup_load_failed",
                    "failure",
                    serde_json::json!({
                        "source": "startup",
                        "version": config.version,
                        "reason": e.to_string(),
                    }),
                )
                .await;
                failed_count += 1;
            }
        }
    }

    if loaded_count > 0 {
        info!(
            "🎉 Successfully loaded {} plugins from database",
            loaded_count
        );
    }
    if failed_count > 0 {
        warn!("⚠️  Failed to load {} plugins", failed_count);
    }
    if loaded_count == 0 && failed_count == 0 {
        info!("📭 No enabled plugins found in database");
    }

    Ok(())
}

// Helper functions

fn timestamp_to_datetime(timestamp: Option<u64>) -> Option<chrono::DateTime<chrono::Utc>> {
    timestamp
        .and_then(|timestamp| i64::try_from(timestamp).ok())
        .and_then(|timestamp| chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0))
}

fn get_runtime_stat(
    plugin_manager: &oxide_plugin_runtime::PluginManager,
    plugin_name: &str,
) -> Result<Option<oxide_plugin_runtime::manager::PluginStatistics>, ApiError> {
    let stats = plugin_manager
        .get_plugin_statistics()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin statistics: {}", e)))?;

    Ok(stats.into_iter().find(|stat| stat.name == plugin_name))
}

async fn activate_persisted_plugin(state: &AppState, plugin_name: &str) -> Result<(), ApiError> {
    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let config = state
        .plugin_config_service
        .get_plugin_config(plugin_name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;

    let wasm_data = state
        .plugin_config_service
        .load_plugin_wasm(plugin_name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to load plugin WASM: {}", e)))?;

    plugin_manager
        .load_and_register_plugin(&state.event_bus, &config, &wasm_data)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to load plugin: {}", e)))
}

fn runtime_resource_usage(
    plugin_stat: &oxide_plugin_runtime::manager::PluginStatistics,
) -> ResourceUsageInfo {
    ResourceUsageInfo {
        memory_bytes: plugin_stat.peak_memory_usage,
        cpu_time_ms: plugin_stat.total_execution_time_ms,
        api_calls: plugin_stat.host_function_calls,
        storage_bytes: 0,
    }
}

fn resolve_plugin_status(
    plugin_name: &str,
    config_status: &oxide_core::plugin_config::PluginStatus,
    runtime_status: Option<&oxide_plugin_runtime::manager::PluginStatus>,
    is_loaded_in_runtime: bool,
) -> PluginStatus {
    match config_status {
        oxide_core::plugin_config::PluginStatus::Enabled => {
            if !is_loaded_in_runtime {
                warn!(
                    "Plugin '{}' is marked as enabled in database but not loaded in runtime",
                    plugin_name
                );
                return PluginStatus::Error;
            }

            match runtime_status {
                Some(oxide_plugin_runtime::manager::PluginStatus::Suspended)
                | Some(oxide_plugin_runtime::manager::PluginStatus::Error) => PluginStatus::Error,
                _ => PluginStatus::Enabled,
            }
        }
        oxide_core::plugin_config::PluginStatus::Disabled => PluginStatus::Disabled,
        oxide_core::plugin_config::PluginStatus::Error => PluginStatus::Error,
        oxide_core::plugin_config::PluginStatus::Loading => PluginStatus::Loading,
        oxide_core::plugin_config::PluginStatus::Uninstalling => PluginStatus::Uninstalling,
    }
}

async fn get_plugin_routes(
    state: &AppState,
    plugin_name: &str,
) -> Result<Vec<PluginRouteInfo>, ApiError> {
    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let all_routes = plugin_manager
        .get_registered_routes()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin routes: {}", e)))?;

    let mut plugin_routes = Vec::new();

    for route in all_routes
        .into_iter()
        .filter(|r| r.plugin_name == plugin_name)
    {
        // Check if custom permissions exist for this plugin route
        let plugin_collection = format!("plugin:{}", plugin_name);
        let permissions = state
            .database_permission_service
            .get_permissions(&plugin_collection)
            .await?;

        let has_custom_permissions = permissions.is_some();

        let route_info = PluginRouteInfo {
            plugin_name: route.plugin_name,
            method: route.method,
            path: route.path,
            handler_function: route.handler_function,
            permissions,
            has_custom_permissions,
        };

        plugin_routes.push(route_info);
    }

    Ok(plugin_routes)
}
