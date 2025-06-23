//! Plugin management operations

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
use super::{
    audit::get_plugin_audit_log,
    permissions::get_plugin_permissions_info,
    types::*,
};
use oxide_core::{
    auth::PermissionService,
    plugin_security::PluginCapability,
};

/// List all installed plugins with their status and details
pub async fn list_plugins(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PluginInfo>>>, ApiError> {
    // Get plugin configurations from database
    let plugin_configs = state.plugin_config_service.list_plugin_configs().await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configurations: {}", e)))?;

    let mut plugins = Vec::new();
    
    for config in plugin_configs {
        // Check if plugin is actually loaded in runtime
        let is_loaded_in_runtime = if let Some(plugin_manager) = &state.plugin_manager {
            if let Ok(stats) = plugin_manager.get_plugin_statistics() {
                stats.iter().any(|s| s.name == config.name)
            } else {
                false
            }
        } else {
            false
        };

        // Get runtime statistics if available
        let (executions, errors, last_execution, resource_usage) = if let Some(plugin_manager) = &state.plugin_manager {
            if let Ok(stats) = plugin_manager.get_plugin_statistics() {
                if let Some(plugin_stat) = stats.iter().find(|s| s.name == config.name) {
                    (
                        plugin_stat.executions,
                        plugin_stat.errors,
                        None, // Runtime doesn't track last execution time yet
                        ResourceUsageInfo {
                            memory_bytes: 0, // Runtime doesn't track these yet
                            cpu_time_ms: 0,
                            api_calls: 0,
                            storage_bytes: 0,
                        }
                    )
                } else {
                    (0, 0, None, ResourceUsageInfo::default())
                }
            } else {
                (0, 0, None, ResourceUsageInfo::default())
            }
        } else {
            (0, 0, None, ResourceUsageInfo::default())
        };

        // Determine actual status based on database status and runtime state
        let actual_status = match config.status {
            oxide_core::plugin_config::PluginStatus::Enabled => {
                if is_loaded_in_runtime {
                    PluginStatus::Enabled
                } else {
                    // Plugin is marked as enabled in DB but not loaded in runtime
                    warn!("Plugin '{}' is marked as enabled in database but not loaded in runtime", config.name);
                    PluginStatus::Error
                }
            },
            oxide_core::plugin_config::PluginStatus::Disabled => PluginStatus::Disabled,
            oxide_core::plugin_config::PluginStatus::Error => PluginStatus::Error,
            oxide_core::plugin_config::PluginStatus::Loading => PluginStatus::Loading,
            oxide_core::plugin_config::PluginStatus::Uninstalling => PluginStatus::Uninstalling,
        };

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
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Verify plugin exists
    let stats = plugin_manager.get_plugin_statistics()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin statistics: {}", e)))?;
    
    let plugin_stat = stats.iter()
        .find(|s| s.name == plugin_name)
        .ok_or_else(|| ApiError::not_found(format!("Plugin '{}' not found", plugin_name)))?;

    // Get plugin configuration from database for complete metadata
    let plugin_config = state.plugin_config_service.get_plugin_config(&plugin_name).await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;

    let capabilities = plugin_config.capabilities.clone();
    let trust_level = plugin_config.trust_level.clone();
    let routes = get_plugin_routes(&state, &plugin_name).await?;
    let audit_log = get_plugin_audit_log(&state, &plugin_name).await?;
    let resource_usage = ResourceUsageInfo {
        memory_bytes: 0, // Runtime doesn't track these yet
        cpu_time_ms: 0,
        api_calls: 0,
        storage_bytes: 0,
    };

    let details = PluginDetails {
        name: plugin_name.clone(),
        status: match plugin_config.status {
            oxide_core::plugin_config::PluginStatus::Enabled => PluginStatus::Enabled,
            oxide_core::plugin_config::PluginStatus::Disabled => PluginStatus::Disabled,
            oxide_core::plugin_config::PluginStatus::Error => PluginStatus::Error,
            oxide_core::plugin_config::PluginStatus::Loading => PluginStatus::Loading,
            oxide_core::plugin_config::PluginStatus::Uninstalling => PluginStatus::Uninstalling,
        },
        version: plugin_config.version,
        description: plugin_config.description,
        author: plugin_config.author,
        capabilities,
        trust_level,
        routes,
        executions: plugin_stat.executions,
        errors: plugin_stat.errors,
        last_execution: None, // Runtime doesn't track last execution time yet
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
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Update database configuration
    state.plugin_config_service.enable_plugin(&plugin_name).await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    // Resume plugin in runtime
    {
        let mut runtime_guard = plugin_manager.runtime.lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.resume_plugin(&plugin_name)
            .map_err(|e| ApiError::internal(format!("Failed to enable plugin: {}", e)))?;
    }

    info!("✅ Plugin '{}' enabled and saved to database", plugin_name);
    Ok(Json(ApiResponse::success(PluginStatus::Enabled)))
}

/// Disable a plugin
pub async fn disable_plugin(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<PluginStatus>>, ApiError> {
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Update database configuration
    state.plugin_config_service.disable_plugin(&plugin_name).await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    // Suspend plugin in runtime
    {
        let mut runtime_guard = plugin_manager.runtime.lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.suspend_plugin(&plugin_name, "Manually disabled".to_string())
            .map_err(|e| ApiError::internal(format!("Failed to disable plugin: {}", e)))?;
    }

    info!("✅ Plugin '{}' disabled and saved to database", plugin_name);
    Ok(Json(ApiResponse::success(PluginStatus::Disabled)))
}

/// Unregister/Uninstall a plugin
pub async fn unregister_plugin(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<String>>, ApiError> {
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Remove from runtime first
    {
        use oxide_core::plugin_api::PluginRuntime;
        
        let mut runtime_guard = plugin_manager.runtime.lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.unload_plugin(&plugin_name)
            .map_err(|e| ApiError::internal(format!("Failed to unregister plugin: {}", e)))?;
    }

    // Remove from database and filesystem using the new service method
    state.plugin_config_service.uninstall_plugin(&plugin_name).await
        .map_err(|e| ApiError::internal(format!("Failed to uninstall plugin: {}", e)))?;

    info!("✅ Plugin '{}' unregistered and removed from database", plugin_name);
    Ok(Json(ApiResponse::success(format!("Plugin '{}' unregistered successfully", plugin_name))))
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
    let enabled_plugins = state.plugin_config_service.get_enabled_plugins().await
        .map_err(|e| ApiError::internal(format!("Failed to get enabled plugins: {}", e)))?;

    let mut loaded_count = 0;
    let mut failed_count = 0;

    for config in enabled_plugins {
        info!("📦 Loading plugin: {} (v{})", config.name, config.version);

        // Get WASM data from filesystem
        let wasm_data = match state.plugin_config_service.load_plugin_wasm(&config.name).await {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("❌ Plugin '{}' WASM loading failed: {}, skipping", config.name, e);
                // Update status to error
                let _ = state.plugin_config_service.update_plugin_status(&config.name, oxide_core::plugin_config::PluginStatus::Error).await;
                failed_count += 1;
                continue;
            }
        };

        // Load plugin into runtime
        {
            let mut runtime_guard = match plugin_manager.runtime.lock() {
                Ok(guard) => guard,
                Err(_) => {
                    tracing::error!("❌ Failed to acquire plugin runtime lock for '{}'", config.name);
                    failed_count += 1;
                    continue;
                }
            };

            match runtime_guard.load_plugin_with_trust(
                &config.name,
                &wasm_data,
                config.trust_level.clone(),
                config.capabilities.clone(),
                config.resource_limits.clone(),
            ) {
                Ok(_) => {
                    info!("✅ Successfully loaded plugin: {}", config.name);
                    loaded_count += 1;
                }
                Err(e) => {
                    tracing::error!("❌ Failed to load plugin '{}': {}", config.name, e);
                    // Update status to error
                    let _ = state.plugin_config_service.update_plugin_status(&config.name, oxide_core::plugin_config::PluginStatus::Error).await;
                    failed_count += 1;
                }
            }
        }
    }

    if loaded_count > 0 {
        info!("🎉 Successfully loaded {} plugins from database", loaded_count);
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

async fn get_plugin_routes(state: &AppState, plugin_name: &str) -> Result<Vec<PluginRouteInfo>, ApiError> {
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let all_routes = plugin_manager.get_registered_routes()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin routes: {}", e)))?;

    let mut plugin_routes = Vec::new();

    for route in all_routes.into_iter().filter(|r| r.plugin_name == plugin_name) {
        // Check if custom permissions exist for this plugin route
        let plugin_collection = format!("plugin:{}", plugin_name);
        let permissions = state.database_permission_service
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