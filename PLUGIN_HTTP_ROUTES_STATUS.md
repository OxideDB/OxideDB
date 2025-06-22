# Plugin HTTP Routes Implementation Status

## Current State Analysis

### ✅ What's Currently Working

1. **Plugin Route Registration**
   - ✅ `register_http_route` host function implemented in runtime
   - ✅ Plugins can register routes via `Http::register_route()` in SDK
   - ✅ Route information stored in `HostState.registered_routes`
   - ✅ `get_registered_routes()` method available on runtime

2. **Plugin HTTP Request Handling**
   - ✅ `handle_http_request` method implemented in runtime
   - ✅ `get_http_request` and `set_http_response` host functions work
   - ✅ Plugin HTTP context and response types defined

3. **Security & Capabilities**
   - ✅ Plugin security capabilities defined for HTTP operations
   - ✅ `RegisterHttpRoutes` and `HandleHttpRequests` capabilities exist
   - ✅ Plugin configuration supports HTTP route specifications

4. **Authorization Framework**
   - ✅ Permission system supports custom rules for collections
   - ✅ Rule engine can evaluate complex authorization expressions
   - ✅ Permission context includes user claims and metadata

### ❌ What's Missing (Critical Gaps)

1. **API Server Integration**
   - ❌ Plugin routes are NOT integrated into the main Axum router
   - ❌ No HTTP handler to route incoming requests to plugins
   - ❌ No wildcard route in `oxide-api` to catch plugin routes

2. **Plugin Manager HTTP Interface**
   - ❌ PluginManager doesn't expose HTTP functionality to API layer
   - ❌ No bridge between `oxidedb::PluginManager` and `oxide-api`
   - ❌ Missing async HTTP request handling in PluginManager

3. **Plugin-Specific Authorization**
   - ❌ No authorization rules specifically for plugin endpoints
   - ❌ Plugin routes don't integrate with the permission system
   - ❌ No way to configure per-plugin route permissions

## Implementation Plan

### Phase 1: API Server Integration

1. **Add Plugin Route Handler**
   - ✅ Created `oxide-api/src/handlers/plugins.rs` (basic structure)
   - ✅ Added authorization framework for plugin routes
   - ⏳ Need to implement actual plugin route matching
   - ⏳ Need to implement plugin execution bridge

2. **Update PluginManager Interface**
   ```rust
   impl PluginManager {
       // Add these methods:
       pub fn get_registered_routes(&self) -> Result<Vec<RouteRegistration>, AppError>
       pub async fn handle_http_request(&self, plugin_name: &str, handler: &str, request: &HttpRequestContext) -> Result<HttpResponse, AppError>
       pub fn find_route_handler(&self, method: &str, path: &str) -> Option<(String, String)> // (plugin_name, handler_function)
   }
   ```

3. **Add Plugin Routes to Main Router**
   ```rust
   // In oxide-api/src/routes.rs
   fn plugin_routes() -> Router<AppState> {
       Router::new()
           .route("/plugin/*path", any(handle_plugin_route))
           // Alternative: Dynamic route registration based on loaded plugins
   }
   ```

### Phase 2: Authorization Implementation

1. **Plugin Route Permissions**
   - Use collection name format: `plugin:{plugin_name}`
   - Support standard CRUD operations mapped to HTTP methods
   - Allow custom permission rules per plugin

2. **Permission Rule Examples**
   ```javascript
   // Public plugin endpoint
   "true"
   
   // Authenticated users only
   "@req.user.id != ''"
   
   // Admin-only plugin endpoint
   "@req.user.role = 'admin'"
   
   // Custom plugin-specific rule
   "@req.user.permissions.includes('use_plugin_' + @plugin.name)"
   ```

### Phase 3: Enhanced Features

1. **Plugin Route Discovery**
   - Dynamic route registration on plugin load
   - Route conflict detection
   - Plugin route listing API

2. **Advanced Security**
   - Rate limiting per plugin
   - Plugin-specific CORS settings
   - Request/response logging for plugin routes

## Required Code Changes

### 1. Update PluginManager (oxidedb/src/plugin_integration.rs)

```rust
impl PluginManager {
    /// Get all registered HTTP routes from all loaded plugins
    pub fn get_registered_routes(&self) -> Result<Vec<RouteRegistration>, AppError> {
        let runtime_guard = self.runtime.lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;
        Ok(runtime_guard.get_registered_routes())
    }

    /// Handle HTTP request to a plugin route
    pub async fn handle_http_request(
        &self,
        plugin_name: &str,
        handler_function: &str,
        request: &HttpRequestContext,
    ) -> Result<HttpResponse, AppError> {
        let mut runtime_guard = self.runtime.lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;
        
        runtime_guard.handle_http_request(plugin_name, handler_function, request)
            .map_err(|e| AppError::internal(format!("Plugin HTTP request failed: {}", e)))
    }

    /// Find plugin handler for a given route
    pub fn find_route_handler(&self, method: &str, path: &str) -> Result<Option<(String, String)>, AppError> {
        let routes = self.get_registered_routes()?;
        
        for route in routes {
            if route.method.to_uppercase() == method.to_uppercase() {
                if path_matches_pattern(&route.path, path) {
                    return Ok(Some((route.plugin_name, route.handler_function)));
                }
            }
        }
        
        Ok(None)
    }
}
```

### 2. Complete Plugin Handler (oxide-api/src/handlers/plugins.rs)

```rust
/// Find a matching plugin route for the given method and path
async fn find_matching_route(
    plugin_manager: &oxidedb::PluginManager,
    method: &Method,
    path: &str,
) -> Result<RouteRegistration, ApiError> {
    let routes = plugin_manager.get_registered_routes()
        .map_err(|e| ApiError::internal_server_error(format!("Failed to get plugin routes: {}", e)))?;

    // Find exact match first
    if let Some(route) = routes.iter().find(|r| 
        r.method.to_uppercase() == method.as_str() && r.path == format!("/{}", path)
    ) {
        return Ok(route.clone());
    }

    // Find pattern match (for :id style parameters)
    for route in &routes {
        if route.method.to_uppercase() == method.as_str() {
            if path_matches_pattern(&route.path, &format!("/{}", path)) {
                return Ok(route.clone());
            }
        }
    }

    Err(ApiError::not_found(format!("Plugin route not found: {} /{}", method, path)))
}

/// Execute plugin handler
async fn execute_plugin_handler(
    plugin_manager: &oxidedb::PluginManager,
    route: &RouteRegistration,
    request_context: &HttpRequestContext,
) -> Result<PluginHttpResponse, ApiError> {
    plugin_manager
        .handle_http_request(&route.plugin_name, &route.handler_function, request_context)
        .await
        .map_err(|e| ApiError::internal_server_error(format!("Plugin execution failed: {}", e)))
}
```

### 3. Add Plugin Routes to Router (oxide-api/src/routes.rs)

```rust
/// Plugin routes
fn plugin_routes() -> Router<AppState> {
    Router::new()
        .route("/plugin/*path", axum::routing::any(handle_plugin_route))
        .route("/api/plugins/routes", axum::routing::get(list_plugin_routes))
        .route("/api/plugins/:plugin_name/permissions", 
               axum::routing::get(get_plugin_permissions)
               .put(update_plugin_permissions))
}

// Add to main router in api_routes()
fn api_routes() -> Router<AppState> {
    Router::new()
        .merge(protected_auth_routes())
        .merge(collection_routes())
        .merge(record_routes())
        .merge(permission_routes())
        .merge(logging_routes())
        .merge(plugin_routes()) // Add this line
}
```

## Testing the Implementation

### 1. Load a Plugin with HTTP Routes

```rust
// In hello-plugin/src/lib.rs
impl PluginEventHandler for HelloPlugin {
    fn on_init(&mut self) -> PluginResult<()> {
        Http::register_route("GET", "/api/hello/items", "handle_get_items")?;
        Http::register_route("POST", "/api/hello/items", "handle_create_item")?;
        Ok(())
    }
}
```

### 2. Test Plugin Route Access

```bash
# Test without authentication (should fail with default permissions)
curl -X GET http://localhost:8080/plugin/api/hello/items

# Test with authentication
curl -X GET http://localhost:8080/plugin/api/hello/items \
  -H "Authorization: Bearer <jwt_token>"
```

### 3. Configure Plugin Permissions

```bash
# Set plugin permissions to allow public access
curl -X PUT http://localhost:8080/api/plugins/hello-plugin/permissions \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "plugin:hello-plugin",
    "rules": {
      "read": {
        "operation": "read",
        "permission": "Public",
        "filter": null
      }
    },
    "auth_required": false
  }'
```

## Current Implementation Status

### Completed ✅
- [x] Basic plugin handler structure
- [x] Authorization framework for plugin routes
- [x] AppState integration with PluginManager
- [x] Permission service integration

### In Progress ⏳
- [ ] PluginManager HTTP interface methods
- [ ] Plugin route matching implementation
- [ ] Plugin execution bridge
- [ ] Router integration

### Not Started ❌
- [ ] Dynamic route registration
- [ ] Plugin route conflict detection
- [ ] Rate limiting for plugin routes
- [ ] Plugin-specific CORS settings

## Next Steps

1. **Implement PluginManager HTTP methods** in `oxidedb/src/plugin_integration.rs`
2. **Complete plugin handler functions** in `oxide-api/src/handlers/plugins.rs`
3. **Add plugin routes to main router** in `oxide-api/src/routes.rs`
4. **Test with hello-plugin** to verify end-to-end functionality
5. **Add comprehensive error handling** and logging
6. **Document API endpoints** for plugin route management

The foundation is solid, but the critical missing piece is the bridge between the plugin system and the HTTP router. Once the PluginManager HTTP interface is implemented, plugin routes will be fully functional with authorization support. 