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

use crate::{
    handlers::{
        admin::{serve_admin_static, serve_admin_ui},
        auth::{login, register, validate_token, get_current_user, logout},
        collections::{
            collection_schema, collection_stats, create_collection, delete_collection,
            list_collections, update_collection_schema,
        },
        health::health_check,
        permissions::{
            get_collection_permissions, update_collection_permissions, list_all_permissions,
            reset_collection_permissions, create_permissions_from_preset,
        },
        records::{create_record, delete_record, get_record, list_records, update_record},
    },

    server::AppState,
};

/// Build the complete router with all routes and middleware
pub fn build_router() -> Router<AppState> {
    Router::new()
        // Health routes (no auth required)
        .merge(health_routes())
        // Auth routes (no auth required for login/register)
        .merge(auth_routes())
        // Admin UI routes (no auth required for static files)
        .merge(admin_routes())
        // API routes (with auth middleware)
        .merge(api_routes())
        // Apply middleware
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
        .route("/auth/login", axum::routing::post(login))
        .route("/auth/register", axum::routing::post(register))
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

/// Admin UI routes
fn admin_routes() -> Router<AppState> {
    Router::new()
        .route("/admin", get(serve_admin_ui))
        .route("/admin/*path", get(serve_admin_static))
}

/// Route configuration for different environments
pub struct RouteConfig {
    /// Enable admin UI routes
    pub enable_admin: bool,
    /// Enable CORS middleware
    pub enable_cors: bool,
    /// Enable request tracing
    pub enable_tracing: bool,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            enable_admin: true,
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
            enable_cors: false,  // Configure CORS more restrictively
            enable_tracing: true,
        }
    }
    
    /// Create a development configuration
    pub fn development() -> Self {
        Self {
            enable_admin: true,
            enable_cors: true,
            enable_tracing: true,
        }
    }
}

/// Build router with custom configuration
pub fn build_router_with_config(config: RouteConfig) -> Router<AppState> {
    let mut router = Router::new()
        .merge(health_routes())
        .merge(auth_routes());
    
    // Add API routes with auth middleware applied
    router = router.merge(api_routes());
    
    // Conditionally add admin routes
    if config.enable_admin {
        router = router.merge(admin_routes());
    }
    
    // Apply middleware based on configuration
    if config.enable_tracing {
        router = router.layer(TraceLayer::new_for_http());
    }
    
    if config.enable_cors {
        router = router.layer(CorsLayer::permissive());
    }
    
    router
}