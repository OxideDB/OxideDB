//! Dashboard statistics handlers
//!
//! This module provides HTTP handlers for dashboard statistics and metrics,
//! offering comprehensive insights into system usage, performance, and health.

use axum::{extract::State, Json};
use oxide_core::plugin_config::PluginStatus as ConfigPluginStatus;
use oxide_core::{AppError, DashboardStats, DashboardStatsService, HealthStatus};
use oxide_db::{
    AuthHealthBridge, DatabaseDashboardStatsService, LoggingStatsBridge, PluginHealthProvider,
    VfsStatsBridge,
};
use oxide_plugin_runtime::manager::PluginStatus as RuntimePluginStatus;
use std::{collections::HashSet, sync::Arc};
use tracing::{debug, warn};

use crate::{
    errors::ApiError, responses::ApiResponse, server::AppState, services::PluginConfigService,
};

struct DashboardPluginHealthBridge {
    plugin_manager: Option<Arc<oxide_plugin_runtime::PluginManager>>,
    plugin_config_service: Arc<PluginConfigService>,
}

impl DashboardPluginHealthBridge {
    fn new(
        plugin_manager: Option<Arc<oxide_plugin_runtime::PluginManager>>,
        plugin_config_service: Arc<PluginConfigService>,
    ) -> Self {
        Self {
            plugin_manager,
            plugin_config_service,
        }
    }
}

#[async_trait::async_trait]
impl PluginHealthProvider for DashboardPluginHealthBridge {
    async fn get_plugin_health(&self) -> Result<HealthStatus, AppError> {
        let plugin_configs = self.plugin_config_service.list_plugin_configs().await?;

        if plugin_configs.is_empty() {
            return Ok(HealthStatus::Healthy);
        }

        let enabled_count = plugin_configs
            .iter()
            .filter(|config| config.enabled)
            .count();
        let enabled_error_count = plugin_configs
            .iter()
            .filter(|config| config.enabled && matches!(config.status, ConfigPluginStatus::Error))
            .count();
        let transitional_count = plugin_configs
            .iter()
            .filter(|config| {
                matches!(
                    config.status,
                    ConfigPluginStatus::Loading | ConfigPluginStatus::Uninstalling
                )
            })
            .count();

        if enabled_count == 0 {
            return if enabled_error_count > 0 || transitional_count > 0 {
                Ok(HealthStatus::Warning)
            } else {
                Ok(HealthStatus::Healthy)
            };
        }

        let Some(plugin_manager) = &self.plugin_manager else {
            warn!(
                "{} plugin(s) are enabled but the plugin manager is unavailable",
                enabled_count
            );
            return Ok(HealthStatus::Degraded);
        };

        let runtime_stats = plugin_manager.get_plugin_statistics()?;
        let mut unhealthy_plugins = HashSet::new();

        for config in plugin_configs.iter().filter(|config| config.enabled) {
            if matches!(config.status, ConfigPluginStatus::Error) {
                unhealthy_plugins.insert(config.name.clone());
            }

            if !matches!(config.status, ConfigPluginStatus::Disabled)
                && !runtime_stats.iter().any(|stat| stat.name == config.name)
            {
                unhealthy_plugins.insert(config.name.clone());
            }
        }

        for stat in &runtime_stats {
            let is_enabled_plugin = plugin_configs
                .iter()
                .any(|config| config.enabled && config.name == stat.name);

            if is_enabled_plugin
                && matches!(
                    stat.status,
                    RuntimePluginStatus::Suspended | RuntimePluginStatus::Error
                )
            {
                unhealthy_plugins.insert(stat.name.clone());
            }
        }

        let unhealthy_signals = unhealthy_plugins.len();

        if unhealthy_signals == 0 {
            if transitional_count > 0 {
                Ok(HealthStatus::Warning)
            } else {
                Ok(HealthStatus::Healthy)
            }
        } else if unhealthy_signals >= enabled_count {
            Ok(HealthStatus::Unhealthy)
        } else {
            Ok(HealthStatus::Degraded)
        }
    }
}

/// Get comprehensive dashboard statistics
///
/// This endpoint provides all the statistics needed for the admin dashboard,
/// including system metrics, collection statistics, user activity, and
/// system health indicators. Uses the enhanced DashboardStatsService for
/// real data collection.
///
/// # Returns
/// Complete dashboard statistics or an error if collection fails
pub async fn get_dashboard_statistics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<DashboardStats>>, ApiError> {
    debug!("Getting comprehensive dashboard statistics via DashboardStatsService");

    // Create dashboard stats service with available integrations
    let logging_bridge = state.logging_api_service.as_ref().map(|logging_api| {
        // Create a new LogApiService instance using the underlying log service
        let log_api_service = Arc::new(oxide_logging::api::LogApiService::new(Arc::clone(
            logging_api.inner().log_service(),
        )));
        Arc::new(LoggingStatsBridge::with_service(log_api_service))
            as Arc<dyn oxide_db::LoggingStatsProvider>
    });

    let vfs_bridge = state.vfs_service.as_ref().map(|vfs_service| {
        Arc::new(VfsStatsBridge::with_service(Arc::clone(vfs_service)))
            as Arc<dyn oxide_db::VfsStatsProvider>
    });

    let auth_bridge = Some(Arc::new(AuthHealthBridge::new(
        state.db.clone(),
        state.auth_service.clone(),
    )) as Arc<dyn oxide_db::AuthHealthProvider>);

    let plugin_bridge = Some(Arc::new(DashboardPluginHealthBridge::new(
        state.plugin_manager.clone(),
        state.plugin_config_service.clone(),
    )) as Arc<dyn oxide_db::PluginHealthProvider>);

    let dashboard_service = DatabaseDashboardStatsService::with_health_integrations(
        state.db.clone(),
        logging_bridge,
        vfs_bridge,
        auth_bridge,
        plugin_bridge,
    );

    let stats = dashboard_service.get_dashboard_stats().await?;

    debug!("Successfully collected enhanced dashboard statistics with {} collections and {} total records", 
        stats.system_stats.total_collections, stats.system_stats.total_records);

    Ok(Json(ApiResponse::success(stats)))
}

/// Get basic system statistics
///
/// A lighter-weight endpoint that returns only core system metrics
/// for situations where full dashboard statistics are not needed.
/// Uses the enhanced DashboardStatsService for better data.
///
/// # Returns
/// Basic system statistics or an error if collection fails
pub async fn get_system_statistics(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<oxide_core::SystemStats>>, ApiError> {
    debug!("Getting enhanced system statistics via DashboardStatsService");

    // Create dashboard stats service with available integrations
    let logging_bridge = state.logging_api_service.as_ref().map(|logging_api| {
        // Create a new LogApiService instance using the underlying log service
        let log_api_service = Arc::new(oxide_logging::api::LogApiService::new(Arc::clone(
            logging_api.inner().log_service(),
        )));
        Arc::new(LoggingStatsBridge::with_service(log_api_service))
            as Arc<dyn oxide_db::LoggingStatsProvider>
    });

    let dashboard_service = DatabaseDashboardStatsService::with_full_integration(
        state.db.clone(),
        logging_bridge,
        None, // VFS not needed for system stats
    );

    let stats = dashboard_service.get_system_stats().await?;

    debug!("Successfully collected enhanced system statistics: {} collections, {} records, {} active users", 
        stats.total_collections, stats.total_records, stats.active_users);

    Ok(Json(ApiResponse::success(stats)))
}

/// Record a new dashboard activity
///
/// This endpoint allows recording user and system activities for display
/// in the dashboard activity feed.
///
/// # Arguments
/// * `activity` - The activity entry to record
///
/// # Returns
/// Success response or an error if recording fails
pub async fn record_dashboard_activity(
    State(state): State<AppState>,
    Json(activity): Json<oxide_core::ActivityEntry>,
) -> Result<Json<ApiResponse<&'static str>>, ApiError> {
    debug!("Recording dashboard activity: {:?}", activity.activity_type);

    let dashboard_service = DatabaseDashboardStatsService::new(state.db.clone());
    dashboard_service.record_activity(activity).await?;

    debug!("Successfully recorded dashboard activity");
    Ok(Json(ApiResponse::success("Activity recorded successfully")))
}

/// Get recent dashboard activities
///
/// This endpoint retrieves recent user and system activities for display
/// in the dashboard activity feed.
///
/// # Query Parameters
/// * `limit` - Maximum number of activities to return (default: 20, max: 100)
///
/// # Returns
/// Vector of recent activities or an error if retrieval fails
pub async fn get_recent_dashboard_activities(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<RecentActivitiesParams>,
) -> Result<Json<ApiResponse<Vec<oxide_core::ActivityEntry>>>, ApiError> {
    let limit = params.limit.unwrap_or(20).min(100);
    debug!("Getting recent dashboard activities with limit: {}", limit);

    let dashboard_service = DatabaseDashboardStatsService::new(state.db.clone());
    let activities = dashboard_service.get_recent_activities(limit).await?;

    debug!(
        "Successfully retrieved {} recent activities",
        activities.len()
    );
    Ok(Json(ApiResponse::success(activities)))
}

/// Query parameters for recent activities endpoint
#[derive(serde::Deserialize)]
pub struct RecentActivitiesParams {
    /// Number of recent activities to return (default: 20, max: 100)
    pub limit: Option<usize>,
}
