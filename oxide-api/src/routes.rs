//! HTTP route definitions and configuration
//!
//! This module contains all route definitions for the OxideDB API,
//! organized by functional area. Routes are defined separately from
//! the server implementation for better maintainability.

use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderName, HeaderValue, Method},
    routing::{delete, get, patch, post},
    Router,
};
use std::{path::PathBuf, sync::OnceLock};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, info, warn};

use crate::{
    handlers::{
        admin::{
            serve_admin_static, serve_admin_ui, serve_external_admin_static,
            serve_external_admin_ui,
        },
        api_keys::{list_api_key_rules, revoke_api_key_rule, upsert_api_key_rule},
        auth::{
            get_current_user, list_auth_collections, login_collection, logout, refresh_token,
            register_collection, validate_token,
        },
        backups::{export_backup, export_backup_stream, get_backup_manifest, restore_backup},
        collections::{
            collection_schema, collection_stats, create_collection, delete_collection,
            list_collections, update_collection_schema,
        },
        dashboard::{
            get_dashboard_statistics, get_recent_dashboard_activities, get_system_statistics,
            record_dashboard_activity,
        },
        health::health_check,
        logs::{
            create_audit_event, create_log_entry, flush_logs, get_audit_events,
            get_collection_logs, get_dashboard_metrics, get_logs, get_logs_by_correlation,
            get_recent_logs, get_retention_stats, get_user_logs, logging_health,
        },
        permissions::{
            create_permissions_from_preset, get_collection_permissions, list_all_permissions,
            reset_collection_permissions, update_collection_permissions,
        },
        plugins::{
            analyze_plugin, disable_plugin, enable_plugin, get_plugin_details,
            get_plugin_permissions, grant_plugin_capability, handle_plugin_route,
            list_plugin_routes, list_plugins, register_plugin, revoke_plugin_capability,
            unregister_plugin, update_plugin_permissions, update_plugin_trust_level,
        },
        records::{create_record, delete_record, get_record, list_records, update_record},
        site_settings::{
            get_settings_health, get_settings_section, get_site_settings, reset_site_settings,
            test_email_configuration, update_settings_section, update_site_settings,
        },
        user_preferences::{
            delete_all_user_preferences, delete_user_preference, get_user_preference,
            list_user_preferences, store_user_preference,
        },
        vfs,
    },
    server::AppState,
};

/// Admin UI mode configuration
#[derive(Debug, Clone, Default)]
pub enum AdminUiMode {
    /// Use embedded admin UI (built into the binary)
    #[default]
    Embedded,
    /// Serve admin UI from external filesystem path
    External(PathBuf),
    /// Admin UI is completely disabled
    Disabled,
}

/// Route configuration for different environments
#[derive(Clone)]
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
    /// Require HTTPS based on forwarded protocol headers
    pub require_https: bool,
    /// Maximum request body size in bytes
    pub max_request_size: usize,
    /// Directory used for persisted plugin package files
    pub plugin_dir: PathBuf,
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            enable_admin: true,
            admin_mode: AdminUiMode::Embedded,
            admin_path: "/admin".to_string(),
            enable_cors: false,
            enable_tracing: true,
            require_https: false,
            max_request_size: 16 * 1024 * 1024,
            plugin_dir: PathBuf::from("oxide-plugins"),
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
            enable_cors: false, // Configure CORS more restrictively
            enable_tracing: true,
            require_https: true,
            max_request_size: 16 * 1024 * 1024,
            plugin_dir: PathBuf::from("oxide-plugins"),
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
            require_https: false,
            max_request_size: 16 * 1024 * 1024,
            plugin_dir: PathBuf::from("oxide-plugins"),
        }
    }

    /// Create a configuration for external admin UI
    pub fn with_external_admin(admin_path: PathBuf, url_prefix: String) -> Self {
        Self {
            enable_admin: true,
            admin_mode: AdminUiMode::External(admin_path),
            admin_path: url_prefix,
            enable_cors: false,
            enable_tracing: true,
            require_https: false,
            max_request_size: 16 * 1024 * 1024,
            plugin_dir: PathBuf::from("oxide-plugins"),
        }
    }
}

/// Registered API endpoint information
#[derive(Debug, Clone)]
pub struct RegisteredEndpoint {
    pub method: String,
    pub path: String,
    pub handler: String,
    pub category: EndpointCategory,
    pub auth_required: bool,
    pub description: Option<String>,
}

/// Category of endpoint for organization
#[derive(Debug, Clone)]
pub enum EndpointCategory {
    Health,
    Auth,
    Collections,
    Records,
    Permissions,
    ApiKeys,
    Backups,
    SiteSettings,
    UserPreferences,
    Plugins,
    Logging,
    Admin,
    PluginRegistered { plugin_name: String },
    VFS,
}

impl std::fmt::Display for EndpointCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EndpointCategory::Health => write!(f, "Health"),
            EndpointCategory::Auth => write!(f, "Authentication"),
            EndpointCategory::Collections => write!(f, "Collections"),
            EndpointCategory::Records => write!(f, "Records"),
            EndpointCategory::Permissions => write!(f, "Permissions"),
            EndpointCategory::ApiKeys => write!(f, "API Keys"),
            EndpointCategory::Backups => write!(f, "Backups"),
            EndpointCategory::SiteSettings => write!(f, "Site Settings"),
            EndpointCategory::UserPreferences => write!(f, "User Preferences"),
            EndpointCategory::Plugins => write!(f, "Plugins"),
            EndpointCategory::Logging => write!(f, "Logging"),
            EndpointCategory::Admin => write!(f, "Admin UI"),
            EndpointCategory::PluginRegistered { plugin_name } => {
                write!(f, "Plugin: {}", plugin_name)
            }
            EndpointCategory::VFS => write!(f, "VFS"),
        }
    }
}

/// Log all registered API endpoints
pub fn log_registered_endpoints(state: &AppState, config: &RouteConfig) {
    info!("📋 Registered API Endpoints Summary");
    info!("=====================================");

    let mut endpoints = get_static_endpoints(config);

    // Add plugin endpoints if plugin manager is available
    if let Some(plugin_manager) = &state.plugin_manager {
        match plugin_manager.get_registered_routes() {
            Ok(plugin_routes) => {
                for route in &plugin_routes {
                    endpoints.push(RegisteredEndpoint {
                        method: route.method.clone(),
                        path: format!("/plugin{}", route.path),
                        handler: format!("{}::{}", route.plugin_name, route.handler_function),
                        category: EndpointCategory::PluginRegistered {
                            plugin_name: route.plugin_name.clone(),
                        },
                        auth_required: true, // Plugin routes require auth by default
                        description: Some(format!("Plugin route handled by {}", route.plugin_name)),
                    });
                }
                info!("✅ Found {} plugin-registered routes", plugin_routes.len());
            }
            Err(e) => {
                debug!("⚠️ Could not retrieve plugin routes: {}", e);
            }
        }
    }

    // Group endpoints by category
    let mut categories: std::collections::HashMap<String, Vec<RegisteredEndpoint>> =
        std::collections::HashMap::new();
    for endpoint in endpoints {
        let category_name = endpoint.category.to_string();
        categories.entry(category_name).or_default().push(endpoint);
    }

    // Log each category
    for (category_name, mut endpoints_in_category) in categories {
        info!("\n📂 {} Endpoints:", category_name);

        // Sort endpoints by method then path
        endpoints_in_category.sort_by(|a, b| match a.method.cmp(&b.method) {
            std::cmp::Ordering::Equal => a.path.cmp(&b.path),
            other => other,
        });

        for endpoint in endpoints_in_category {
            let auth_indicator = if endpoint.auth_required {
                "🔒"
            } else {
                "🔓"
            };
            info!(
                "  {} {:>6} {:<30} → {}",
                auth_indicator, endpoint.method, endpoint.path, endpoint.handler
            );

            if let Some(description) = endpoint.description {
                debug!("      └─ {}", description);
            }
        }
    }

    let total_endpoints = get_total_endpoint_count(state, config);
    info!("\n📊 Total registered endpoints: {}", total_endpoints);
    info!("   🔒 = Authentication required");
    info!("   🔓 = Public access");
    info!("=====================================\n");
}

/// Get count of total endpoints
fn get_total_endpoint_count(state: &AppState, config: &RouteConfig) -> usize {
    let static_count = get_static_endpoints(config).len();
    let plugin_count = if let Some(plugin_manager) = &state.plugin_manager {
        plugin_manager
            .get_registered_routes()
            .unwrap_or_default()
            .len()
    } else {
        0
    };
    static_count + plugin_count
}

/// Get all static (non-plugin) API endpoints
fn get_static_endpoints(config: &RouteConfig) -> Vec<RegisteredEndpoint> {
    let mut endpoints = Vec::new();

    // Health endpoints
    endpoints.push(RegisteredEndpoint {
        method: "GET".to_string(),
        path: "/health".to_string(),
        handler: "health::health_check".to_string(),
        category: EndpointCategory::Health,
        auth_required: false,
        description: Some("System health check".to_string()),
    });

    // Authentication endpoints
    let auth_endpoints = vec![
        (
            "GET",
            "/auth/collections",
            "auth::list_auth_collections",
            false,
            "List authentication collections",
        ),
        (
            "POST",
            "/auth/:collection/login",
            "auth::login_collection",
            false,
            "Login to collection",
        ),
        (
            "POST",
            "/auth/:collection/register",
            "auth::register_collection",
            false,
            "Register in collection",
        ),
        (
            "POST",
            "/auth/validate",
            "auth::validate_token",
            false,
            "Validate JWT token",
        ),
        ("POST", "/auth/logout", "auth::logout", false, "Logout user"),
        (
            "POST",
            "/auth/refresh",
            "auth::refresh_token",
            false,
            "Refresh JWT token",
        ),
        (
            "GET",
            "/auth/me",
            "auth::get_current_user",
            true,
            "Get current user info",
        ),
    ];

    for (method, path, handler, auth_required, description) in auth_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::Auth,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Collection endpoints
    let collection_endpoints = vec![
        (
            "GET",
            "/collections",
            "collections::list_collections",
            true,
            "List all collections",
        ),
        (
            "POST",
            "/collections",
            "collections::create_collection",
            true,
            "Create new collection",
        ),
        (
            "DELETE",
            "/collections/:collection",
            "collections::delete_collection",
            true,
            "Delete collection",
        ),
        (
            "GET",
            "/collections/:collection/stats",
            "collections::collection_stats",
            true,
            "Get collection statistics",
        ),
        (
            "GET",
            "/collections/:collection/schema",
            "collections::collection_schema",
            true,
            "Get collection schema",
        ),
        (
            "PUT",
            "/collections/:collection/schema",
            "collections::update_collection_schema",
            true,
            "Update collection schema",
        ),
    ];

    for (method, path, handler, auth_required, description) in collection_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::Collections,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Record endpoints
    let record_endpoints = vec![
        (
            "GET",
            "/collections/:collection/records",
            "records::list_records",
            true,
            "List records in collection",
        ),
        (
            "POST",
            "/collections/:collection/records",
            "records::create_record",
            true,
            "Create new record",
        ),
        (
            "GET",
            "/collections/:collection/records/:id",
            "records::get_record",
            true,
            "Get specific record",
        ),
        (
            "PUT",
            "/collections/:collection/records/:id",
            "records::update_record",
            true,
            "Update record",
        ),
        (
            "DELETE",
            "/collections/:collection/records/:id",
            "records::delete_record",
            true,
            "Delete record",
        ),
    ];

    for (method, path, handler, auth_required, description) in record_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::Records,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Permission endpoints
    let permission_endpoints = vec![
        (
            "GET",
            "/permissions",
            "permissions::list_all_permissions",
            true,
            "List all permissions",
        ),
        (
            "GET",
            "/collections/:collection/permissions",
            "permissions::get_collection_permissions",
            true,
            "Get collection permissions",
        ),
        (
            "PUT",
            "/collections/:collection/permissions",
            "permissions::update_collection_permissions",
            true,
            "Update collection permissions",
        ),
        (
            "POST",
            "/collections/:collection/permissions/reset",
            "permissions::reset_collection_permissions",
            true,
            "Reset collection permissions",
        ),
        (
            "POST",
            "/collections/:collection/permissions/preset",
            "permissions::create_permissions_from_preset",
            true,
            "Create permissions from preset",
        ),
    ];

    for (method, path, handler, auth_required, description) in permission_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::Permissions,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // API key endpoints
    let api_key_endpoints = vec![
        (
            "GET",
            "/admin/api-keys",
            "api_keys::list_api_key_rules",
            true,
            "List API key access rules",
        ),
        (
            "POST",
            "/admin/api-keys",
            "api_keys::upsert_api_key_rule",
            true,
            "Create or replace an API key access rule",
        ),
        (
            "POST",
            "/admin/api-keys/revoke",
            "api_keys::revoke_api_key_rule",
            true,
            "Revoke an API key access rule",
        ),
    ];

    for (method, path, handler, auth_required, description) in api_key_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::ApiKeys,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Backup endpoints
    let backup_endpoints = vec![
        (
            "GET",
            "/admin/backups/manifest",
            "backups::get_backup_manifest",
            true,
            "Preview backup/export contents",
        ),
        (
            "GET",
            "/admin/backups/export",
            "backups::export_backup",
            true,
            "Export a JSON database snapshot",
        ),
        (
            "GET",
            "/admin/backups/export/stream",
            "backups::export_backup_stream",
            true,
            "Stream a JSON database snapshot",
        ),
        (
            "POST",
            "/admin/backups/restore",
            "backups::restore_backup",
            true,
            "Restore a JSON database snapshot",
        ),
    ];

    for (method, path, handler, auth_required, description) in backup_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::Backups,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Site settings endpoints
    let site_settings_endpoints = vec![
        (
            "GET",
            "/admin/settings",
            "site_settings::get_site_settings",
            true,
            "Get site settings",
        ),
        (
            "PUT",
            "/admin/settings",
            "site_settings::update_site_settings",
            true,
            "Update site settings",
        ),
        (
            "POST",
            "/admin/settings/reset",
            "site_settings::reset_site_settings",
            true,
            "Reset site settings to defaults",
        ),
        (
            "GET",
            "/admin/settings/health",
            "site_settings::get_settings_health",
            true,
            "Get settings health status",
        ),
        (
            "POST",
            "/admin/settings/email/test",
            "site_settings::test_email_configuration",
            true,
            "Test email configuration",
        ),
        (
            "GET",
            "/admin/settings/:section",
            "site_settings::get_settings_section",
            true,
            "Get specific settings section",
        ),
        (
            "PUT",
            "/admin/settings/:section",
            "site_settings::update_settings_section",
            true,
            "Update specific settings section",
        ),
    ];

    for (method, path, handler, auth_required, description) in site_settings_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::SiteSettings,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // User preferences endpoints
    let user_preferences_endpoints = vec![
        (
            "GET",
            "/user/preferences",
            "user_preferences::list_user_preferences",
            true,
            "List user preferences",
        ),
        (
            "DELETE",
            "/user/preferences",
            "user_preferences::delete_all_user_preferences",
            true,
            "Delete all user preferences",
        ),
        (
            "GET",
            "/user/preferences/:key",
            "user_preferences::get_user_preference",
            true,
            "Get user preference",
        ),
        (
            "PUT",
            "/user/preferences/:key",
            "user_preferences::store_user_preference",
            true,
            "Store user preference",
        ),
        (
            "DELETE",
            "/user/preferences/:key",
            "user_preferences::delete_user_preference",
            true,
            "Delete user preference",
        ),
    ];

    for (method, path, handler, auth_required, description) in user_preferences_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::UserPreferences,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Plugin endpoints
    let plugin_endpoints = vec![
        (
            "GET",
            "/plugins",
            "plugins::list_plugins",
            true,
            "List all plugins",
        ),
        (
            "POST",
            "/plugins",
            "plugins::register_plugin",
            true,
            "Register/install new plugin",
        ),
        (
            "POST",
            "/plugins/analyze",
            "plugins::analyze_plugin",
            true,
            "Analyze plugin",
        ),
        (
            "GET",
            "/plugins/:plugin_name",
            "plugins::get_plugin_details",
            true,
            "Get plugin details",
        ),
        (
            "DELETE",
            "/plugins/:plugin_name",
            "plugins::unregister_plugin",
            true,
            "Unregister/uninstall plugin",
        ),
        (
            "POST",
            "/plugins/:plugin_name/enable",
            "plugins::enable_plugin",
            true,
            "Enable plugin",
        ),
        (
            "POST",
            "/plugins/:plugin_name/disable",
            "plugins::disable_plugin",
            true,
            "Disable plugin",
        ),
        (
            "POST",
            "/plugins/:plugin_name/capabilities/:capability_name",
            "plugins::grant_plugin_capability",
            true,
            "Grant capability to plugin",
        ),
        (
            "DELETE",
            "/plugins/:plugin_name/capabilities/:capability_name",
            "plugins::revoke_plugin_capability",
            true,
            "Revoke capability from plugin",
        ),
        (
            "PUT",
            "/plugins/:plugin_name/trust-level",
            "plugins::update_plugin_trust_level",
            true,
            "Update plugin trust level",
        ),
        (
            "GET",
            "/plugins/routes",
            "plugins::list_plugin_routes",
            true,
            "List plugin routes",
        ),
        (
            "GET",
            "/plugins/:plugin_name/permissions",
            "plugins::get_plugin_permissions",
            true,
            "Get plugin permissions",
        ),
        (
            "PUT",
            "/plugins/:plugin_name/permissions",
            "plugins::update_plugin_permissions",
            true,
            "Update plugin permissions",
        ),
        (
            "ANY",
            "/plugin/*path",
            "plugins::handle_plugin_route",
            true,
            "Handle plugin routes",
        ),
    ];

    for (method, path, handler, auth_required, description) in plugin_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::Plugins,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Logging endpoints
    let logging_endpoints = vec![
        (
            "GET",
            "/logs",
            "logs::get_logs",
            true,
            "Get application logs",
        ),
        (
            "GET",
            "/logs/audit",
            "logs::get_audit_events",
            true,
            "Get audit events",
        ),
        (
            "GET",
            "/logs/dashboard",
            "logs::get_dashboard_metrics",
            true,
            "Get dashboard metrics",
        ),
        (
            "GET",
            "/logs/recent",
            "logs::get_recent_logs",
            true,
            "Get recent logs",
        ),
        (
            "GET",
            "/logs/retention",
            "logs::get_retention_stats",
            true,
            "Get retention statistics",
        ),
        (
            "POST",
            "/logs/create",
            "logs::create_log_entry",
            true,
            "Create log entry",
        ),
        (
            "POST",
            "/logs/audit/create",
            "logs::create_audit_event",
            true,
            "Create audit event",
        ),
        (
            "POST",
            "/logs/flush",
            "logs::flush_logs",
            true,
            "Flush logs to storage",
        ),
        (
            "GET",
            "/logs/correlation/:correlation_id",
            "logs::get_logs_by_correlation",
            true,
            "Get logs by correlation ID",
        ),
        (
            "GET",
            "/logs/user/:user_id",
            "logs::get_user_logs",
            true,
            "Get user-specific logs",
        ),
        (
            "GET",
            "/logs/collection/:collection",
            "logs::get_collection_logs",
            true,
            "Get collection-specific logs",
        ),
        (
            "GET",
            "/logs/health",
            "logs::logging_health",
            true,
            "Get logging system health",
        ),
    ];

    for (method, path, handler, auth_required, description) in logging_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::Logging,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // VFS endpoints
    let vfs_endpoints = vec![
        (
            "POST",
            "/collections/:collection/files",
            "vfs::upload_file",
            true,
            "Upload file to collection",
        ),
        (
            "GET",
            "/collections/:collection/files",
            "vfs::list_files",
            true,
            "List files in collection",
        ),
        (
            "GET",
            "/collections/:collection/files/:file_id/metadata",
            "vfs::get_file_metadata",
            true,
            "Get file metadata from collection",
        ),
        (
            "GET",
            "/collections/:collection/files/:file_id",
            "vfs::download_file",
            true,
            "Download file from collection",
        ),
        (
            "PATCH",
            "/collections/:collection/files/:file_id",
            "vfs::move_file",
            true,
            "Move or rename file in collection",
        ),
        (
            "DELETE",
            "/collections/:collection/files/:file_id",
            "vfs::delete_file",
            true,
            "Delete file from collection",
        ),
        (
            "GET",
            "/collections/:collection/vfs/usage",
            "vfs::get_collection_usage_stats",
            true,
            "Get collection VFS usage statistics",
        ),
        (
            "GET",
            "/vfs/usage",
            "vfs::get_usage_stats",
            true,
            "Get aggregate VFS usage statistics",
        ),
    ];

    for (method, path, handler, auth_required, description) in vfs_endpoints {
        endpoints.push(RegisteredEndpoint {
            method: method.to_string(),
            path: path.to_string(),
            handler: handler.to_string(),
            category: EndpointCategory::VFS,
            auth_required,
            description: Some(description.to_string()),
        });
    }

    // Admin endpoints (if enabled)
    if config.enable_admin {
        let admin_wildcard_path = format!("{}/*path", config.admin_path);
        let admin_endpoints = vec![
            (
                "GET",
                config.admin_path.as_str(),
                "admin::serve_admin_ui",
                false,
                "Admin UI main page",
            ),
            (
                "GET",
                admin_wildcard_path.as_str(),
                "admin::serve_admin_static",
                false,
                "Admin UI static assets",
            ),
        ];

        for (method, path, handler, auth_required, description) in admin_endpoints {
            endpoints.push(RegisteredEndpoint {
                method: method.to_string(),
                path: path.to_string(),
                handler: handler.to_string(),
                category: EndpointCategory::Admin,
                auth_required,
                description: Some(description.to_string()),
            });
        }
    }

    endpoints
}

static PROTECTED_ADMIN_API_PATTERNS: OnceLock<Vec<String>> = OnceLock::new();

/// Check whether a path belongs to a registered authenticated admin API route.
pub(crate) fn is_registered_protected_admin_api_path(path: &str) -> bool {
    if !is_admin_path(path) {
        return false;
    }

    protected_admin_api_patterns()
        .iter()
        .any(|pattern| route_pattern_matches(pattern, path))
}

fn protected_admin_api_patterns() -> &'static [String] {
    PROTECTED_ADMIN_API_PATTERNS
        .get_or_init(|| {
            get_static_endpoints(&RouteConfig::default())
                .into_iter()
                .filter(|endpoint| endpoint.auth_required && is_admin_path(&endpoint.path))
                .map(|endpoint| endpoint.path)
                .collect()
        })
        .as_slice()
}

fn is_admin_path(path: &str) -> bool {
    path == "/admin" || path.starts_with("/admin/")
}

fn route_pattern_matches(pattern: &str, path: &str) -> bool {
    let pattern_parts: Vec<&str> = pattern.trim_start_matches('/').split('/').collect();
    let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

    let mut pattern_iter = pattern_parts.iter();
    let mut path_iter = path_parts.iter();

    loop {
        match (pattern_iter.next(), path_iter.next()) {
            (Some(pattern_part), Some(_)) if pattern_part.starts_with('*') => return true,
            (Some(pattern_part), Some(_)) if pattern_part.starts_with(':') => continue,
            (Some(pattern_part), Some(path_part)) if pattern_part == path_part => continue,
            (None, None) => return true,
            _ => return false,
        }
    }
}

/// Build the complete router with default configuration and middleware.
pub fn build_router(state: AppState) -> Router<()> {
    build_router_with_config(RouteConfig::default(), state)
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
        .route(
            "/auth/:collection/login",
            axum::routing::post(login_collection),
        )
        .route(
            "/auth/:collection/register",
            axum::routing::post(register_collection),
        )
        // Common auth routes
        .route("/auth/validate", axum::routing::post(validate_token))
        .route("/auth/logout", axum::routing::post(logout))
        .route("/auth/refresh", axum::routing::post(refresh_token))
    // Note: /auth/me moved to protected_auth_routes()
}

/// Core API routes
fn api_routes(config: &RouteConfig) -> Router<AppState> {
    Router::new()
        // Protected auth routes (require authentication)
        .merge(protected_auth_routes())
        // Dashboard statistics routes
        .merge(dashboard_routes())
        // Collection management routes
        .merge(collection_routes())
        // Record management routes
        .merge(record_routes())
        // Permission management routes
        .merge(permission_routes())
        // API key rule management routes
        .merge(api_key_routes())
        // Backup and export routes
        .merge(backup_routes())
        // Site settings routes
        .merge(site_settings_routes())
        // User preferences routes
        .merge(user_preferences_routes())
        // Plugin routes
        .merge(plugin_routes(config.max_request_size))
        // Logging routes
        .merge(logging_routes())
        // VFS routes
        .merge(vfs_routes())
}

/// Protected authentication routes that require authentication
fn protected_auth_routes() -> Router<AppState> {
    Router::new().route("/auth/me", get(get_current_user))
}

/// Dashboard statistics routes
fn dashboard_routes() -> Router<AppState> {
    Router::new()
        // Comprehensive dashboard statistics
        .route("/dashboard/stats", get(get_dashboard_statistics))
        // Basic system statistics
        .route("/dashboard/system", get(get_system_statistics))
        // Activity management
        .route(
            "/dashboard/activities",
            post(record_dashboard_activity).get(get_recent_dashboard_activities),
        )
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

/// API key rule management routes
fn api_key_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/api-keys",
            get(list_api_key_rules).post(upsert_api_key_rule),
        )
        .route("/admin/api-keys/revoke", post(revoke_api_key_rule))
}

/// Backup and export routes
fn backup_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/backups/manifest", get(get_backup_manifest))
        .route("/admin/backups/export", get(export_backup))
        .route("/admin/backups/export/stream", get(export_backup_stream))
        .route("/admin/backups/restore", post(restore_backup))
}

/// Site settings routes
fn site_settings_routes() -> Router<AppState> {
    Router::new()
        // Site settings management (admin only)
        .route(
            "/admin/settings",
            get(get_site_settings).put(update_site_settings),
        )
        .route(
            "/admin/settings/reset",
            axum::routing::post(reset_site_settings),
        )
        .route("/admin/settings/health", get(get_settings_health))
        .route(
            "/admin/settings/email/test",
            axum::routing::post(test_email_configuration),
        )
        .route(
            "/admin/settings/:section",
            get(get_settings_section).put(update_settings_section),
        )
}

/// User preferences routes
fn user_preferences_routes() -> Router<AppState> {
    Router::new()
        // User preferences management
        .route(
            "/user/preferences",
            get(list_user_preferences).delete(delete_all_user_preferences),
        )
        .route(
            "/user/preferences/:key",
            get(get_user_preference)
                .put(store_user_preference)
                .delete(delete_user_preference),
        )
}

/// Plugin management routes
fn plugin_routes(max_request_size: usize) -> Router<AppState> {
    Router::new()
        // Plugin management
        .route(
            "/plugins",
            get(list_plugins)
                .post(register_plugin)
                .layer(DefaultBodyLimit::max(max_request_size)),
        )
        .route(
            "/plugins/analyze",
            axum::routing::post(analyze_plugin).layer(DefaultBodyLimit::max(max_request_size)),
        )
        .route(
            "/plugins/:plugin_name",
            get(get_plugin_details).delete(unregister_plugin),
        )
        .route(
            "/plugins/:plugin_name/enable",
            axum::routing::post(enable_plugin),
        )
        .route(
            "/plugins/:plugin_name/disable",
            axum::routing::post(disable_plugin),
        )
        // Plugin capability management
        .route(
            "/plugins/:plugin_name/capabilities/:capability_name",
            axum::routing::post(grant_plugin_capability).delete(revoke_plugin_capability),
        )
        .route(
            "/plugins/:plugin_name/trust-level",
            axum::routing::put(update_plugin_trust_level),
        )
        // Plugin route management
        .route("/plugins/routes", get(list_plugin_routes))
        // Plugin permission management
        .route(
            "/plugins/:plugin_name/permissions",
            get(get_plugin_permissions).put(update_plugin_permissions),
        )
        // Catch-all for plugin-registered routes
        .route("/plugin/*path", axum::routing::any(handle_plugin_route))
}

/// Logging routes
fn logging_routes() -> Router<AppState> {
    Router::new()
        .route("/logs", get(get_logs))
        .route("/logs/audit", get(get_audit_events))
        .route("/logs/dashboard", get(get_dashboard_metrics))
        .route("/logs/recent", get(get_recent_logs))
        .route("/logs/retention", get(get_retention_stats))
        .route("/logs/create", axum::routing::post(create_log_entry))
        .route(
            "/logs/audit/create",
            axum::routing::post(create_audit_event),
        )
        .route("/logs/flush", axum::routing::post(flush_logs))
        .route(
            "/logs/correlation/:correlation_id",
            get(get_logs_by_correlation),
        )
        .route("/logs/user/:user_id", get(get_user_logs))
        .route("/logs/collection/:collection", get(get_collection_logs))
        .route("/logs/health", get(logging_health))
}

/// VFS routes
fn vfs_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/collections/:collection/files",
            axum::routing::post(vfs::upload_file),
        )
        .route(
            "/collections/:collection/files",
            axum::routing::get(vfs::list_files),
        )
        .route(
            "/collections/:collection/files/:file_id",
            axum::routing::get(vfs::download_file),
        )
        .route(
            "/collections/:collection/files/:file_id",
            patch(vfs::move_file),
        )
        .route(
            "/collections/:collection/files/:file_id/metadata",
            axum::routing::get(vfs::get_file_metadata),
        )
        .route(
            "/collections/:collection/files/:file_id",
            axum::routing::delete(vfs::delete_file),
        )
        .route(
            "/collections/:collection/vfs/usage",
            axum::routing::get(vfs::get_collection_usage_stats),
        )
        .route("/vfs/usage", axum::routing::get(vfs::get_usage_stats))
}

/// Admin UI routes with configurable mode
fn admin_routes(mode: AdminUiMode, path_prefix: String) -> Router<AppState> {
    match mode {
        AdminUiMode::Embedded => Router::new()
            .route(&path_prefix, get(serve_admin_ui))
            .route(&format!("{}/*path", path_prefix), get(serve_admin_static)),
        AdminUiMode::External(admin_path) => {
            use axum::extract::Path;

            // For external mode, we need to create a custom handler that includes the path
            // We'll use a simple approach that works with Axum's routing
            Router::new()
                .route(
                    &path_prefix,
                    get({
                        let admin_path = admin_path.clone();
                        || async move { serve_external_admin_ui(admin_path).await }
                    }),
                )
                .route(
                    &format!("{}/*path", path_prefix),
                    get({
                        let admin_path = admin_path.clone();
                        |path: Path<String>| async move {
                            serve_external_admin_static(admin_path, path).await
                        }
                    }),
                )
        }
        AdminUiMode::Disabled => {
            // Return empty router when admin is disabled
            Router::new()
        }
    }
}

fn configured_cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(cors_allowed_origins())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            header::ACCEPT,
            HeaderName::from_static("x-api-key"),
            HeaderName::from_static("x-requested-with"),
        ])
        .allow_credentials(true)
}

fn cors_allowed_origins() -> Vec<HeaderValue> {
    let configured_origins = std::env::var("OXIDEDB_CORS_ALLOWED_ORIGINS")
        .ok()
        .map(|value| parse_cors_origins(&value))
        .unwrap_or_default();

    if !configured_origins.is_empty() {
        return configured_origins;
    }

    warn!(
        "CORS enabled without OXIDEDB_CORS_ALLOWED_ORIGINS; allowing localhost development origins only"
    );
    parse_cors_origins(
        "http://localhost:5173,http://127.0.0.1:5173,http://localhost:3000,http://127.0.0.1:3000",
    )
}

fn parse_cors_origins(value: &str) -> Vec<HeaderValue> {
    value
        .split(',')
        .filter_map(|origin| {
            let origin = origin.trim();
            if origin.is_empty() {
                return None;
            }

            match HeaderValue::from_str(origin) {
                Ok(value) => Some(value),
                Err(error) => {
                    warn!("Ignoring invalid CORS origin '{}': {}", origin, error);
                    None
                }
            }
        })
        .collect()
}

/// Build the unlayered route tree. Authenticated app builders must wrap this
/// with the state-aware auth middleware before serving protected API routes.
fn build_route_tree(config: RouteConfig) -> Router<AppState> {
    let mut router = Router::new().merge(health_routes()).merge(auth_routes());

    // Add API routes
    router = router.merge(api_routes(&config));

    // Conditionally add admin routes based on configuration
    if config.enable_admin {
        router = router.merge(admin_routes(config.admin_mode, config.admin_path));
    }

    router
}

/// Build router with custom configuration and production middleware.
pub fn build_router_with_config(config: RouteConfig, state: AppState) -> Router<()> {
    build_router_with_config_and_middleware(config, state)
}

/// Build router with custom configuration and apply middleware in correct order
pub fn build_router_with_config_and_middleware(config: RouteConfig, state: AppState) -> Router<()> {
    let mut router = build_route_tree(config.clone());

    router = router.layer(DefaultBodyLimit::max(config.max_request_size));

    // Apply auth middleware to protected API routes only. Public routes are
    // already bypassed by auth_middleware via its public endpoint allowlist.
    router = router.route_layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::auth_middleware,
    ));

    router = router.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::auth_rate_limit_middleware,
    ));

    // Apply logging middleware to all routes (should be applied before CORS and tracing)
    router = router.layer(axum::middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::request_logging_middleware,
    ));

    if config.require_https {
        router = router.layer(axum::middleware::from_fn(
            crate::middleware::require_https_middleware,
        ));
    }

    // Apply middleware based on configuration - CORS should be outermost
    if config.enable_tracing {
        router = router.layer(TraceLayer::new_for_http());
    }

    if config.enable_cors {
        router = router.layer(configured_cors_layer());
    }

    router.with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_admin_api_paths_are_matched_from_route_inventory() {
        assert!(is_registered_protected_admin_api_path("/admin/api-keys"));
        assert!(is_registered_protected_admin_api_path(
            "/admin/backups/restore"
        ));
        assert!(is_registered_protected_admin_api_path(
            "/admin/settings/security"
        ));
        assert!(is_registered_protected_admin_api_path(
            "/admin/settings/email/test"
        ));

        assert!(!is_registered_protected_admin_api_path("/admin"));
        assert!(!is_registered_protected_admin_api_path(
            "/admin/assets/app.js"
        ));
        assert!(!is_registered_protected_admin_api_path(
            "/adminish/settings"
        ));
    }

    #[test]
    fn route_pattern_matching_respects_path_segments() {
        assert!(route_pattern_matches(
            "/admin/settings/:section",
            "/admin/settings/auth"
        ));
        assert!(!route_pattern_matches(
            "/admin/settings",
            "/admin/settings/auth"
        ));
        assert!(!route_pattern_matches(
            "/admin/settings/:section",
            "/admin/settings/auth/extra"
        ));
    }
}
