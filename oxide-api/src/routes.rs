//! HTTP route definitions and configuration
//!
//! This module contains all route definitions for the OxideDB API,
//! organized by functional area. Routes are defined separately from
//! the server implementation for better maintainability.

use axum::{
    routing::{delete, get},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use std::path::PathBuf;

use crate::{
    handlers::{
        admin::{serve_admin_static, serve_admin_ui, serve_external_admin_ui, serve_external_admin_static},
        auth::{
            validate_token, get_current_user, logout,
            list_auth_collections, login_collection, register_collection,
        },
        collections::{
            collection_schema, collection_stats, create_collection, delete_collection,
            list_collections, update_collection_schema,
        },
        health::health_check,
        logs::{
            get_logs, get_audit_events, get_dashboard_metrics, get_recent_logs,
            get_retention_stats, create_log_entry, create_audit_event, flush_logs,
            get_logs_by_correlation, get_user_logs, get_collection_logs, logging_health,
        },
        permissions::{
            get_collection_permissions, update_collection_permissions, list_all_permissions,
            reset_collection_permissions, create_permissions_from_preset,
        },
        records::{create_record, delete_record, get_record, list_records, update_record},
    },

    server::AppState,
};

/// Admin UI mode configuration
#[derive(Debug, Clone)]
pub enum AdminUiMode {
    /// Use embedded admin UI (built into the binary)
    Embedded,
    /// Serve admin UI from external filesystem path
    External(PathBuf),
    /// Admin UI is completely disabled
    Disabled,
}

impl Default for AdminUiMode {
    fn default() -> Self {
        Self::Embedded
    }
}

/// Route configuration for different environments
pub struct RouteConfig {
    /// Enable admin UI routes
    pub enable_admin: bool,
    /// Admin UI mode and configuration
    pub admin_mode: AdminUiMode,
    /// Admin UI path prefix (e.g., "/admin")
    pub admin_path: String,
    /// Enable CORS middleware
    pub enable_cors: bool,
    /// Enable request tracing
    pub enable_tracing: bool,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            enable_admin: true,
            admin_mode: AdminUiMode::Embedded,
            admin_path: "/admin".to_string(),
            enable_cors: true,
            enable_tracing: true,
        }
    }
}

impl RouteConfig {
    /// Create a production configuration
    pub fn production() -> Self {
        Self {
            enable_admin: false, // Disable admin UI in production
            admin_mode: AdminUiMode::Disabled,
            admin_path: "/admin".to_string(),
            enable_cors: false,  // Configure CORS more restrictively
            enable_tracing: true,
        }
    }
    
    /// Create a development configuration
    pub fn development() -> Self {
        Self {
            enable_admin: true,
            admin_mode: AdminUiMode::Embedded,
            admin_path: "/admin".to_string(),
            enable_cors: true,
            enable_tracing: true,
        }
    }

    /// Create a configuration for external admin UI
    pub fn with_external_admin(admin_path: PathBuf, url_prefix: String) -> Self {
        Self {
            enable_admin: true,
            admin_mode: AdminUiMode::External(admin_path),
            admin_path: url_prefix,
            enable_cors: true,
            enable_tracing: true,
        }
    }
}

/// Build the complete router with all routes and middleware
pub fn build_router() -> Router<AppState> {
    Router::new()
        // Health routes (no auth required)
        .merge(health_routes())
        // Auth routes (no auth required for login/register)
        .merge(auth_routes())
        // Admin UI routes (no auth required for static files)
        .merge(admin_routes(AdminUiMode::Embedded, "/admin".to_string()))
        // API routes (with auth middleware)
        .merge(api_routes())
        // Apply middleware - note: logging middleware is only available with state
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
}

/// Health check routes
fn health_routes() -> Router<AppState> {
    Router::new().route("/health", get(health_check))
}

/// Authentication routes
fn auth_routes() -> Router<AppState> {
    Router::new()
        // Collection-specific auth routes
        .route("/auth/collections", get(list_auth_collections))
        .route("/auth/:collection/login", axum::routing::post(login_collection))
        .route("/auth/:collection/register", axum::routing::post(register_collection))
        // Common auth routes
        .route("/auth/validate", axum::routing::post(validate_token))
        .route("/auth/logout", axum::routing::post(logout))
        .route("/auth/me", get(get_current_user))
}

/// Core API routes
fn api_routes() -> Router<AppState> {
    Router::new()
        // Collection management routes
        .merge(collection_routes())
        // Record management routes
        .merge(record_routes())
        // Permission management routes
        .merge(permission_routes())
        // Logging routes
        .merge(logging_routes())
}

/// Collection management routes
fn collection_routes() -> Router<AppState> {
    Router::new()
        // Collection CRUD operations
        .route(
            "/collections",
            get(list_collections).post(create_collection),
        )
        .route("/collections/:collection", delete(delete_collection))
        // Collection metadata and schema
        .route("/collections/:collection/stats", get(collection_stats))
        .route(
            "/collections/:collection/schema",
            get(collection_schema).put(update_collection_schema),
        )
}

/// Record management routes
fn record_routes() -> Router<AppState> {
    Router::new()
        // Record CRUD operations
        .route(
            "/collections/:collection/records",
            get(list_records).post(create_record),
        )
        .route(
            "/collections/:collection/records/:id",
            get(get_record).put(update_record).delete(delete_record),
        )
}

/// Permission management routes
fn permission_routes() -> Router<AppState> {
    Router::new()
        // Global permissions overview
        .route("/permissions", get(list_all_permissions))
        // Collection-specific permission management
        .route(
            "/collections/:collection/permissions",
            get(get_collection_permissions).put(update_collection_permissions),
        )
        .route(
            "/collections/:collection/permissions/reset",
            axum::routing::post(reset_collection_permissions),
        )
        .route(
            "/collections/:collection/permissions/preset",
            axum::routing::post(create_permissions_from_preset),
        )
}

/// Logging routes
fn logging_routes() -> Router<AppState> {
    Router::new()
        .route("/logs", get(get_logs))
        .route("/logs/audit-events", get(get_audit_events))
        .route("/logs/dashboard-metrics", get(get_dashboard_metrics))
        .route("/logs/recent", get(get_recent_logs))
        .route("/logs/retention-stats", get(get_retention_stats))
        .route("/logs/create-log-entry", axum::routing::post(create_log_entry))
        .route("/logs/create-audit-event", axum::routing::post(create_audit_event))
        .route("/logs/flush", axum::routing::post(flush_logs))
        .route("/logs/correlation/:correlation_id", get(get_logs_by_correlation))
        .route("/logs/user/:user_id", get(get_user_logs))
        .route("/logs/collection/:collection", get(get_collection_logs))
        .route("/logs/health", get(logging_health))
}

/// Admin UI routes with configurable mode
fn admin_routes(mode: AdminUiMode, path_prefix: String) -> Router<AppState> {
    match mode {
        AdminUiMode::Embedded => {
            Router::new()
                .route(&path_prefix, get(serve_admin_ui))
                .route(&format!("{}/*path", path_prefix), get(serve_admin_static))
        }
        AdminUiMode::External(admin_path) => {
            use axum::extract::Path;
            
            // For external mode, we need to create a custom handler that includes the path
            // We'll use a simple approach that works with Axum's routing
            Router::new()
                .route(&path_prefix, get({
                    let admin_path = admin_path.clone();
                    || async move { serve_external_admin_ui(admin_path).await }
                }))
                .route(&format!("{}/*path", path_prefix), get({
                    let admin_path = admin_path.clone();
                    |path: Path<String>| async move {
                        serve_external_admin_static(admin_path, path).await
                    }
                }))
        }
        AdminUiMode::Disabled => {
            // Return empty router when admin is disabled
            Router::new()
        }
    }
}

/// Build router with custom configuration
pub fn build_router_with_config(config: RouteConfig) -> Router<AppState> {
    let mut router = Router::new()
        .merge(health_routes())
        .merge(auth_routes());
    
    // Add API routes
    router = router.merge(api_routes());
    
    // Conditionally add admin routes based on configuration
    if config.enable_admin {
        router = router.merge(admin_routes(config.admin_mode, config.admin_path));
    }
    
    router
}

/// Build router with custom configuration and apply middleware in correct order
pub fn build_router_with_config_and_middleware(config: RouteConfig, state: AppState) -> Router<()> {
    let mut router = Router::new()
        .merge(health_routes())
        .merge(auth_routes());
    
    // Add API routes with auth middleware applied to protected routes only
    router = router.merge(
        api_routes()
            .route_layer(axum::middleware::from_fn_with_state(
                state.clone(),
                crate::middleware::auth_middleware,
            ))
    );
    
    // Conditionally add admin routes based on configuration
    if config.enable_admin {
        router = router.merge(admin_routes(config.admin_mode, config.admin_path));
    }
    
    // Apply logging middleware to all routes (should be applied before CORS and tracing)
    router = router.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::request_logging_middleware,
    ));
    
    // Apply middleware based on configuration - CORS should be outermost
    if config.enable_tracing {
        router = router.layer(TraceLayer::new_for_http());
    }
    
    if config.enable_cors {
        router = router.layer(CorsLayer::permissive());
    }
    
    router.with_state(state)
}