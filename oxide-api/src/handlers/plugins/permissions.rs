//! Plugin permissions management

use axum::{
    extract::{Path, State},
    Json,
};
use tracing::info;

use super::types::*;
use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};
use oxide_core::auth::{CollectionPermissions, CrudOperation, PermissionLevel, PermissionService};

/// Get plugin route permissions for a specific plugin
pub async fn get_plugin_permissions(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    super::ensure_plugin_superuser(&authenticated_user, "view plugin permissions")?;

    let plugin_collection = format!("plugin:{}", plugin_name);

    let permissions = state
        .database_permission_service
        .get_permissions(&plugin_collection)
        .await?
        .unwrap_or_else(|| {
            // Return default permissions if none exist
            let mut perms = CollectionPermissions::new(plugin_collection);
            perms.set_crud_permission(CrudOperation::Read, PermissionLevel::AuthenticatedOnly);
            perms
        });

    Ok(Json(ApiResponse::success(permissions)))
}

/// Update plugin route permissions
pub async fn update_plugin_permissions(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
    Json(permissions): Json<CollectionPermissions>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    super::ensure_plugin_superuser(&authenticated_user, "update plugin permissions")?;

    let plugin_collection = format!("plugin:{}", plugin_name);

    // Validate that the collection name matches the plugin
    if permissions.collection != plugin_collection {
        return Err(ApiError::bad_request(
            "Collection name must match plugin name".to_string(),
        ));
    }

    // Store the permissions
    state
        .database_permission_service
        .store_permissions(&permissions)
        .await?;

    info!("✅ Updated permissions for plugin: {}", plugin_name);
    Ok(Json(ApiResponse::success(permissions)))
}

/// List all plugin routes and their permissions
pub async fn list_plugin_routes(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PluginRouteInfo>>>, ApiError> {
    super::ensure_plugin_superuser(&authenticated_user, "list plugin routes")?;

    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let all_routes = plugin_manager
        .get_registered_routes()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin routes: {}", e)))?;

    let mut route_infos = Vec::new();

    for route in all_routes {
        // Check if custom permissions exist for this plugin
        let plugin_collection = format!("plugin:{}", route.plugin_name);
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

        route_infos.push(route_info);
    }

    Ok(Json(ApiResponse::success(route_infos)))
}

/// Get plugin permissions information for internal use
pub async fn get_plugin_permissions_info(
    state: &AppState,
    plugin_name: &str,
) -> Result<Option<CollectionPermissions>, ApiError> {
    let plugin_collection = format!("plugin:{}", plugin_name);

    state
        .database_permission_service
        .get_permissions(&plugin_collection)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin permissions: {}", e)))
}
