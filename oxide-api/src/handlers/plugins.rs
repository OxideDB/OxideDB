//! Plugin HTTP Route Handlers
//!
//! This module provides HTTP handlers for plugin-registered routes,
//! including authorization and security validation.

use axum::{
    extract::{Path, Query, State, Multipart, DefaultBodyLimit},
    http::{HeaderMap, Method},
    response::Response,
    Json,
};
use std::collections::HashMap;
use tracing::{debug, info, warn, error};
use serde::{Deserialize, Serialize};

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
    extractors::AuthenticatedUser,
};
use oxide_core::{
    plugin_api::{HttpRequestContext, HttpResponse as PluginHttpResponse, RouteRegistration},
    auth::{CollectionPermissions, PermissionContext, PermissionLevel, PermissionService, CrudOperation},
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits, SecurityAuditEntry},
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
            
            perms.set_operation_permission(CrudOperation::Read, default_permission);
            
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
        operation,
        plugin_collection,
        None, // No specific record ID for plugin routes
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

/// Get plugin route permissions for a specific plugin
pub async fn get_plugin_permissions(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    let plugin_collection = format!("plugin:{}", plugin_name);
    
    let permissions = state.database_permission_service
        .get_permissions(&plugin_collection)
        .await?
        .unwrap_or_else(|| {
            // Return default permissions if none exist
            let mut perms = CollectionPermissions::new(plugin_collection);
            perms.set_operation_permission(CrudOperation::Read, PermissionLevel::AuthenticatedOnly);
            perms
        });

    Ok(Json(ApiResponse::success(permissions)))
}

/// Update plugin route permissions
pub async fn update_plugin_permissions(
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
    Json(permissions): Json<CollectionPermissions>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    let plugin_collection = format!("plugin:{}", plugin_name);
    
    // Validate that the collection name matches the plugin
    if permissions.collection != plugin_collection {
        return Err(ApiError::bad_request("Collection name must match plugin name".to_string()));
    }

    // Store the permissions
    state.database_permission_service
        .store_permissions(&permissions)
        .await?;

    info!("✅ Updated permissions for plugin: {}", plugin_name);
    Ok(Json(ApiResponse::success(permissions)))
}

/// List all plugin routes and their permissions
pub async fn list_plugin_routes(
    State(_state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PluginRouteInfo>>>, ApiError> {
    // Placeholder implementation - needs proper PluginManager integration
    warn!("Plugin routes listing not yet implemented - needs PluginManager interface update");
    Ok(Json(ApiResponse::success(Vec::new())))
}

/// Information about a plugin route including permissions
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PluginRouteInfo {
    pub plugin_name: String,
    pub method: String,
    pub path: String,
    pub handler_function: String,
    pub permissions: Option<CollectionPermissions>,
    pub has_custom_permissions: bool,
}

/// List all installed plugins with their status and details
pub async fn list_plugins(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<PluginInfo>>>, ApiError> {
    // Get plugin configurations from database
    let plugin_configs = state.plugin_config_service.list_plugin_configs().await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configurations: {}", e)))?;

    let mut plugins = Vec::new();
    
    for config in plugin_configs {
        let plugin_info = PluginInfo {
            name: config.name.clone(),
            status: match config.status {
                oxide_core::plugin_config::PluginStatus::Enabled => PluginStatus::Enabled,
                oxide_core::plugin_config::PluginStatus::Disabled => PluginStatus::Disabled,
                oxide_core::plugin_config::PluginStatus::Error => PluginStatus::Error,
                oxide_core::plugin_config::PluginStatus::Loading => PluginStatus::Loading,
                oxide_core::plugin_config::PluginStatus::Uninstalling => PluginStatus::Uninstalling,
            },
            version: config.version,
            description: config.description,
            author: config.author,
            capabilities: config.capabilities,
            trust_level: config.trust_level,
            routes: get_plugin_routes(&state, &config.name).await?,
            executions: 0, // TODO: Track execution statistics
            errors: 0, // TODO: Track error statistics
            last_execution: None, // TODO: Implement execution tracking
            resource_usage: ResourceUsageInfo::default(), // TODO: Track resource usage
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

    let capabilities = get_plugin_capabilities(&state, &plugin_name).await?;
    let trust_level = get_plugin_trust_level(&state, &plugin_name).await?;
    let routes = get_plugin_routes(&state, &plugin_name).await?;
    let audit_log = get_plugin_audit_log(&state, &plugin_name).await?;
    let resource_usage = get_plugin_resource_usage(&state, &plugin_name).await?;

    let details = PluginDetails {
        name: plugin_name.clone(),
        status: match plugin_stat.status {
            oxide_plugin_runtime::PluginStatus::Active => PluginStatus::Enabled,
            oxide_plugin_runtime::PluginStatus::Suspended => PluginStatus::Disabled,
            oxide_plugin_runtime::PluginStatus::Error => PluginStatus::Error,
        },
        version: "1.0.0".to_string(), // TODO: Get from plugin metadata
        description: format!("Plugin: {}", plugin_name),
        author: "Unknown".to_string(), // TODO: Get from plugin metadata
        capabilities,
        trust_level,
        routes,
        executions: plugin_stat.executions,
        errors: plugin_stat.errors,
        last_execution: None, // TODO: Implement execution tracking
        resource_usage,
        audit_log,
        permissions: get_plugin_permissions_info(&state, &plugin_name).await?,
    };

    Ok(Json(ApiResponse::success(details)))
}

/// Register/Install a new plugin
pub async fn register_plugin(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<PluginInfo>>, ApiError> {
    debug!("🔌 Starting plugin registration");
    
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let mut plugin_name: Option<String> = None;
    let mut plugin_data: Option<Vec<u8>> = None;
    let mut trust_level = PluginTrustLevel::Untrusted;
    let mut capabilities: Vec<PluginCapability> = Vec::new();
    let mut version = "1.0.0".to_string();
    let mut description = "".to_string();
    let mut author = "Unknown".to_string();

    // Parse multipart form data
    debug!("🔌 Parsing multipart form data");
    while let Some(field) = multipart.next_field().await
        .map_err(|e| {
            error!("Multipart field parsing error: {}", e);
            ApiError::bad_request(format!("Error parsing multipart field: {}", e))
        })? 
    {
        let field_name = field.name().unwrap_or("unknown");
        debug!("🔌 Processing field: {}", field_name);
        
        match field_name {
            "plugin_name" => {
                plugin_name = Some(field.text().await
                    .map_err(|e| {
                        error!("Error reading plugin_name field: {}", e);
                        ApiError::bad_request(format!("Invalid plugin name: {}", e))
                    })?);
                debug!("🔌 Plugin name: {:?}", plugin_name);
            }
            "version" => {
                version = field.text().await
                    .map_err(|e| {
                        error!("Error reading version field: {}", e);
                        ApiError::bad_request(format!("Invalid version: {}", e))
                    })?;
                debug!("🔌 Version: {}", version);
            }
            "description" => {
                description = field.text().await
                    .map_err(|e| {
                        error!("Error reading description field: {}", e);
                        ApiError::bad_request(format!("Invalid description: {}", e))
                    })?;
                debug!("🔌 Description: {}", description);
            }
            "author" => {
                author = field.text().await
                    .map_err(|e| {
                        error!("Error reading author field: {}", e);
                        ApiError::bad_request(format!("Invalid author: {}", e))
                    })?;
                debug!("🔌 Author: {}", author);
            }
            "wasm_file" => {
                debug!("🔌 Reading WASM file data");
                // Get file name if available
                let file_name = field.file_name().unwrap_or("unknown.wasm");
                debug!("🔌 WASM file name: {}", file_name);
                
                let content_type = field.content_type().unwrap_or("application/octet-stream");
                debug!("🔌 Content type: {}", content_type);
                
                plugin_data = Some(field.bytes().await
                    .map_err(|e| {
                        error!("Error reading WASM file bytes: {}", e);
                        ApiError::bad_request(format!("Error reading WASM file: {}", e))
                    })?
                    .to_vec());
                
                if let Some(ref data) = plugin_data {
                    debug!("🔌 WASM file size: {} bytes", data.len());
                }
            }
            "trust_level" => {
                let trust_str = field.text().await
                    .map_err(|e| {
                        error!("Error reading trust_level field: {}", e);
                        ApiError::bad_request(format!("Invalid trust level: {}", e))
                    })?;
                debug!("🔌 Trust level string: {}", trust_str);
                trust_level = serde_json::from_str(&format!("\"{}\"", trust_str))
                    .map_err(|e| {
                        error!("Error parsing trust level '{}': {}", trust_str, e);
                        ApiError::bad_request(format!("Invalid trust level format: {}", e))
                    })?;
                debug!("🔌 Trust level: {:?}", trust_level);
            }
            "capabilities" => {
                let caps_str = field.text().await
                    .map_err(|e| {
                        error!("Error reading capabilities field: {}", e);
                        ApiError::bad_request(format!("Invalid capabilities: {}", e))
                    })?;
                debug!("🔌 Capabilities string: {}", caps_str);
                capabilities = serde_json::from_str(&caps_str)
                    .map_err(|e| {
                        error!("Error parsing capabilities '{}': {}", caps_str, e);
                        ApiError::bad_request(format!("Invalid capabilities format: {}", e))
                    })?;
                debug!("🔌 Capabilities: {:?}", capabilities);
            }
            _ => {
                debug!("🔌 Skipping unknown field: {}", field_name);
                // Skip unknown fields - we need to consume the field to avoid errors
                let _ = field.bytes().await;
            }
        }
    }
    
    debug!("🔌 Finished parsing multipart data");

    let plugin_name = plugin_name
        .ok_or_else(|| ApiError::bad_request("Missing plugin_name field".to_string()))?;
    let plugin_data = plugin_data
        .ok_or_else(|| ApiError::bad_request("Missing wasm_file field".to_string()))?;

    // Validate plugin data
    if plugin_data.is_empty() {
        return Err(ApiError::bad_request("WASM file is empty".to_string()));
    }

    // Basic WASM magic number check
    if plugin_data.len() < 4 || &plugin_data[0..4] != b"\x00asm" {
        return Err(ApiError::bad_request("Invalid WASM file format - missing magic number".to_string()));
    }

    debug!("🔌 Plugin validation successful: name={}, size={} bytes", plugin_name, plugin_data.len());

    // Install plugin using the new service method
    let record_id = state.plugin_config_service.install_plugin(
        plugin_name.clone(),
        version.clone(),
        description.clone(),
        author.clone(),
        trust_level.clone(),
        capabilities.clone(),
        ResourceLimits::default(),
        plugin_data.clone(),
        None, // metadata
    ).await
        .map_err(|e| ApiError::internal(format!("Failed to install plugin: {}", e)))?;

    // Load the plugin with specified settings
    {
        let mut runtime_guard = plugin_manager.runtime.lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.load_plugin_with_trust(
            &plugin_name,
            &plugin_data,
            trust_level.clone(),
            capabilities.clone(),
            ResourceLimits::default(), // TODO: Make configurable
        ).map_err(|e| ApiError::internal(format!("Failed to load plugin: {}", e)))?;
    }

    info!("✅ Plugin '{}' registered successfully and saved to database", plugin_name);

    // Return plugin info
    let plugin_info = PluginInfo {
        name: plugin_name.clone(),
        status: PluginStatus::Enabled,
        version,
        description: if description.is_empty() { format!("Plugin: {}", plugin_name) } else { description },
        author,
        capabilities,
        trust_level,
        routes: get_plugin_routes(&state, &plugin_name).await?,
        executions: 0,
        errors: 0,
        last_execution: None,
        resource_usage: ResourceUsageInfo::default(),
    };

    Ok(Json(ApiResponse::success(plugin_info)))
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

// Helper functions

async fn get_plugin_capabilities(state: &AppState, plugin_name: &str) -> Result<Vec<PluginCapability>, ApiError> {
    // TODO: Implement proper capability retrieval from plugin runtime
    // For now, return basic capabilities
    Ok(vec![
        PluginCapability::LogInfo,
        PluginCapability::LogError,
        PluginCapability::ReadEventData,
    ])
}

async fn get_plugin_trust_level(state: &AppState, plugin_name: &str) -> Result<PluginTrustLevel, ApiError> {
    // TODO: Implement proper trust level retrieval from plugin runtime
    Ok(PluginTrustLevel::Untrusted)
}

async fn get_plugin_routes(state: &AppState, plugin_name: &str) -> Result<Vec<PluginRouteInfo>, ApiError> {
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let all_routes = plugin_manager.get_registered_routes()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin routes: {}", e)))?;

    let plugin_routes = all_routes
        .into_iter()
        .filter(|route| route.plugin_name == plugin_name)
        .map(|route| PluginRouteInfo {
            plugin_name: route.plugin_name,
            method: route.method,
            path: route.path,
            handler_function: route.handler_function,
            permissions: None, // TODO: Get actual permissions
            has_custom_permissions: false, // TODO: Check if custom permissions exist
        })
        .collect();

    Ok(plugin_routes)
}

async fn get_plugin_audit_log(state: &AppState, plugin_name: &str) -> Result<Vec<SecurityAuditEntry>, ApiError> {
    // TODO: Implement audit log retrieval
    Ok(Vec::new())
}

async fn get_plugin_resource_usage(state: &AppState, plugin_name: &str) -> Result<ResourceUsageInfo, ApiError> {
    // TODO: Implement resource usage tracking
    Ok(ResourceUsageInfo::default())
}

async fn get_plugin_permissions_info(state: &AppState, plugin_name: &str) -> Result<Option<CollectionPermissions>, ApiError> {
    let plugin_collection = format!("plugin:{}", plugin_name);
    
    state.database_permission_service
        .get_permissions(&plugin_collection)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin permissions: {}", e)))
}

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

// Data structures for API responses

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub status: PluginStatus,
    pub version: String,
    pub description: String,
    pub author: String,
    pub capabilities: Vec<PluginCapability>,
    pub trust_level: PluginTrustLevel,
    pub routes: Vec<PluginRouteInfo>,
    pub executions: u64,
    pub errors: u64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
    pub resource_usage: ResourceUsageInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginDetails {
    pub name: String,
    pub status: PluginStatus,
    pub version: String,
    pub description: String,
    pub author: String,
    pub capabilities: Vec<PluginCapability>,
    pub trust_level: PluginTrustLevel,
    pub routes: Vec<PluginRouteInfo>,
    pub executions: u64,
    pub errors: u64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
    pub resource_usage: ResourceUsageInfo,
    pub audit_log: Vec<SecurityAuditEntry>,
    pub permissions: Option<CollectionPermissions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PluginStatus {
    Enabled,
    Disabled,
    Error,
    Loading,
    Uninstalling,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ResourceUsageInfo {
    pub memory_bytes: u64,
    pub cpu_time_ms: u64,
    pub api_calls: u64,
    pub storage_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTrustLevelRequest {
    pub trust_level: PluginTrustLevel,
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
                error!("❌ Plugin '{}' WASM loading failed: {}, skipping", config.name, e);
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
                    error!("❌ Failed to acquire plugin runtime lock for '{}'", config.name);
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
                    error!("❌ Failed to load plugin '{}': {}", config.name, e);
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