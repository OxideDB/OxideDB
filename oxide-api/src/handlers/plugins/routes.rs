//! Plugin HTTP route handling

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, Method},
    response::Response,
};
use std::collections::HashMap;
use tracing::{debug, info};

use crate::{
    errors::ApiError,
    server::AppState,
    extractors::AuthenticatedUser,
};
use oxide_core::{
    plugin_api::{HttpRequestContext, HttpResponse as PluginHttpResponse, RouteRegistration},
    auth::{CollectionPermissions, PermissionContext, PermissionLevel, PermissionService, CrudOperation},
};

/// Handle plugin HTTP routes
pub async fn handle_plugin_route(
    State(state): State<AppState>,
    user_claims: Option<AuthenticatedUser>,
    method: Method,
    Path(route_path): Path<String>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Option<String>,
) -> Result<Response<axum::body::Body>, ApiError> {
    debug!("🔌 Plugin route request: {} /{}", method, route_path);

    // Get plugin manager
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Find matching plugin route
    let route = find_matching_route(&plugin_manager, &method, &route_path).await?;
    
    // Check authorization for the plugin route
    check_plugin_route_authorization(&state, &route, user_claims.as_ref(), &method, &route_path).await?;

    // Build HTTP request context
    let mut request_context = build_request_context(
        method,
        route_path.clone(),
        query_params,
        headers,
        body,
        user_claims.as_ref(),
    )?;

    // Extract path parameters from the matched route
    let path_params = extract_path_params(&route.path, &format!("/{}", route_path));
    request_context.path_params = path_params;

    // Execute plugin handler
    let plugin_response = execute_plugin_handler(&plugin_manager, &route, &request_context).await?;

    // Convert plugin response to Axum response
    convert_plugin_response_to_axum(plugin_response)
}

/// Find a matching plugin route for the given method and path
async fn find_matching_route(
    plugin_manager: &oxide_plugin_runtime::PluginManager,
    method: &Method,
    path: &str,
) -> Result<RouteRegistration, ApiError> {
    let routes = plugin_manager.get_registered_routes()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin routes: {}", e)))?;

    debug!("🔍 Looking for plugin route: {} /{}", method, path);
    debug!("📋 Available plugin routes: {:#?}", routes);

    // Find exact match first
    if let Some(route) = routes.iter().find(|r| 
        r.method.to_uppercase() == method.as_str() && r.path == format!("/{}", path)
    ) {
        debug!("✅ Found exact match for plugin route: {} {}", route.method, route.path);
        return Ok(route.clone());
    }

    // Find pattern match (for :id style parameters)
    for route in &routes {
        if route.method.to_uppercase() == method.as_str() {
            if path_matches_pattern(&route.path, &format!("/{}", path)) {
                debug!("✅ Found pattern match for plugin route: {} {} (pattern: {})", route.method, format!("/{}", path), route.path);
                return Ok(route.clone());
            }
        }
    }

    Err(ApiError::not_found(format!("Plugin route not found: {} /{}", method, path)))
}

/// Simple pattern matching for plugin routes
fn path_matches_pattern(pattern: &str, path: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.trim_start_matches('/').split('/').collect();
    let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    if pattern_parts.len() != path_parts.len() {
        return false;
    }

    for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
        if pattern_part.starts_with(':') {
            // Parameter match - always matches
            continue;
        } else if pattern_part != path_part {
            return false;
        }
    }

    true
}

/// Extract path parameters from a matched route
fn extract_path_params(pattern: &str, path: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    let pattern_parts: Vec<&str> = pattern.trim_start_matches('/').split('/').collect();
    let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
        if let Some(param_name) = pattern_part.strip_prefix(':') {
            params.insert(param_name.to_string(), path_part.to_string());
        }
    }

    params
}

/// Check authorization for plugin routes
async fn check_plugin_route_authorization(
    state: &AppState,
    route: &RouteRegistration,
    user_claims: Option<&AuthenticatedUser>,
    method: &Method,
    path: &str,
) -> Result<(), ApiError> {
    debug!("🔒 Checking authorization for plugin route: {} /{}", method, path);

    // Create a special collection name for plugin routes
    let plugin_collection = format!("plugin:{}", route.plugin_name);

    // Get permission service
    let permission_service = &state.database_permission_service;

    // Try to get permissions for the plugin collection
    let permissions = match permission_service.get_permissions(&plugin_collection).await? {
        Some(perms) => perms,
        None => {
            // Create default permissions for plugin routes
            // By default, plugin routes require authentication unless configured otherwise
            info!("Creating default permissions for plugin collection: {}", plugin_collection);
            let mut perms = CollectionPermissions::new(plugin_collection.clone());
            
            // Set default permission based on plugin security configuration
            // For now, default to authenticated-only access
            let default_permission = PermissionLevel::AuthenticatedOnly;
            
            perms.set_crud_permission(CrudOperation::Read, default_permission);
            
            // Store the default permissions
            permission_service.store_permissions(&perms).await?;
            perms
        }
    };

    // Create permission context
    let operation = match method.as_str() {
        "GET" => CrudOperation::Read,
        "POST" => CrudOperation::Create,
        "PUT" | "PATCH" => CrudOperation::Update,
        "DELETE" => CrudOperation::Delete,
        _ => CrudOperation::Read,
    };

    let mut metadata = std::collections::HashMap::new();
    metadata.insert("plugin_name".to_string(), serde_json::Value::String(route.plugin_name.clone()));
    metadata.insert("handler_function".to_string(), serde_json::Value::String(route.handler_function.clone()));
    metadata.insert("route_path".to_string(), serde_json::Value::String(route.path.clone()));

    let permission_context = PermissionContext::new(
        user_claims.map(|user| user.claims.clone()),
        oxide_core::auth::types::Operation::Crud(operation),
        plugin_collection,
        None,
    ).with_metadata(metadata);

    // Check permission
    let allowed = permission_service.check_permission(&permissions, &permission_context)?;

    if !allowed {
        let user_info = user_claims
            .map(|user| format!("user {} ({})", user.claims.sub, user.claims.role))
            .unwrap_or_else(|| "anonymous".to_string());

        return Err(ApiError::forbidden(format!(
            "Access denied: {} cannot access plugin route {} {} (plugin: {})",
            user_info, method, path, route.plugin_name
        )));
    }

    info!("✅ Plugin route authorization granted for {}: {} {}", route.plugin_name, method, path);
    Ok(())
}

/// Build HTTP request context for plugin
fn build_request_context(
    method: Method,
    path: String,
    query_params: HashMap<String, String>,
    headers: HeaderMap,
    body: Option<String>,
    user_claims: Option<&AuthenticatedUser>,
) -> Result<HttpRequestContext, ApiError> {
    // Convert headers to HashMap
    let mut header_map = HashMap::new();
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            header_map.insert(name.as_str().to_string(), value_str.to_string());
        }
    }

    // Build user context
    let user_context = user_claims.map(|user| {
        serde_json::json!({
            "id": user.claims.sub,
            "role": user.claims.role,
            "exp": user.claims.exp,
            "iat": user.claims.iat,
        })
    });

    Ok(HttpRequestContext {
        method: method.to_string(),
        path: format!("/{}", path),
        query_params,
        headers: header_map,
        body,
        path_params: HashMap::new(), // TODO: Extract from route pattern
        user: user_context,
    })
}

/// Execute plugin handler
async fn execute_plugin_handler(
    plugin_manager: &oxide_plugin_runtime::PluginManager,
    route: &RouteRegistration,
    request_context: &HttpRequestContext,
) -> Result<PluginHttpResponse, ApiError> {
    debug!("🚀 Executing plugin handler: {}::{}", route.plugin_name, route.handler_function);

    plugin_manager
        .handle_http_request(&route.plugin_name, &route.handler_function, request_context)
        .await
        .map_err(|e| ApiError::internal(format!("Plugin execution failed: {}", e)))
}

/// Convert plugin response to Axum response
fn convert_plugin_response_to_axum(
    plugin_response: PluginHttpResponse,
) -> Result<Response<axum::body::Body>, ApiError> {
    let mut response_builder = Response::builder()
        .status(plugin_response.status_code);

    // Set default content-type if not specified
    let needs_content_type = !plugin_response.headers.contains_key("content-type") && 
                             !plugin_response.headers.contains_key("Content-Type");

    // Add headers
    for (name, value) in plugin_response.headers {
        response_builder = response_builder.header(name, value);
    }

    if needs_content_type {
        response_builder = response_builder.header("content-type", "application/json");
    }

    let response = response_builder
        .body(axum::body::Body::from(plugin_response.body))
        .map_err(|e| ApiError::internal(format!("Failed to build response: {}", e)))?;

    Ok(response)
} 