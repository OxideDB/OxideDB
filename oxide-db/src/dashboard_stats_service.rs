//! Dashboard Statistics Service Implementation
//!
//! This module provides a concrete implementation of the DashboardStatsService trait
//! that collects real data from the database, logging system, and other components
//! to provide comprehensive dashboard statistics.

use async_trait::async_trait;
use chrono::Utc;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, warn};

use crate::Db;
use oxide_core::{
    ActivityEntry, ApiStats, AppError, AuthService, CollectionType, DashboardStats,
    DashboardStatsService, HealthStatus, SystemHealth, SystemStats, UserActivity, UserStats,
};

/// Dashboard statistics service that aggregates data from multiple sources
pub struct DatabaseDashboardStatsService {
    /// Database connection for querying statistics
    db: Arc<dyn Db>,
    /// Optional logging service for API and user activity metrics
    logging_service: Option<Arc<dyn LoggingStatsProvider>>,
    /// Optional VFS service for storage metrics
    vfs_service: Option<Arc<dyn VfsStatsProvider>>,
    /// Optional authentication service health provider
    auth_health_provider: Option<Arc<dyn AuthHealthProvider>>,
    /// Optional plugin system health provider
    plugin_health_provider: Option<Arc<dyn PluginHealthProvider>>,
    // Placeholder: each instance no longer tracks start; global used instead
}

/// Trait for providing logging-related statistics
#[async_trait]
pub trait LoggingStatsProvider: Send + Sync {
    /// Get API request statistics
    async fn get_api_stats(&self) -> Result<ApiStats, AppError>;

    /// Get user activity statistics
    async fn get_user_activity_stats(&self) -> Result<UserStats, AppError>;

    /// Get system health from logging perspective
    async fn get_logging_health(&self) -> Result<HealthStatus, AppError>;
}

/// Trait for providing VFS-related statistics
#[async_trait]
pub trait VfsStatsProvider: Send + Sync {
    /// Get VFS storage usage
    async fn get_vfs_storage_usage(&self) -> Result<u64, AppError>;

    /// Get VFS health status
    async fn get_vfs_health(&self) -> Result<HealthStatus, AppError>;
}

/// Trait for providing authentication health checks
#[async_trait]
pub trait AuthHealthProvider: Send + Sync {
    /// Get authentication subsystem health status
    async fn get_auth_health(&self) -> Result<HealthStatus, AppError>;
}

/// Trait for providing plugin-system health checks
#[async_trait]
pub trait PluginHealthProvider: Send + Sync {
    /// Get plugin subsystem health status
    async fn get_plugin_health(&self) -> Result<HealthStatus, AppError>;
}

impl DatabaseDashboardStatsService {
    /// Create a new dashboard stats service
    pub fn new(db: Arc<dyn Db>) -> Self {
        Self {
            db,
            logging_service: None,
            vfs_service: None,
            auth_health_provider: None,
            plugin_health_provider: None,
        }
    }

    /// Create a dashboard stats service with logging integration
    pub fn with_logging(db: Arc<dyn Db>, logging_service: Arc<dyn LoggingStatsProvider>) -> Self {
        Self {
            db,
            logging_service: Some(logging_service),
            vfs_service: None,
            auth_health_provider: None,
            plugin_health_provider: None,
        }
    }

    /// Create a dashboard stats service with full integration
    pub fn with_full_integration(
        db: Arc<dyn Db>,
        logging_service: Option<Arc<dyn LoggingStatsProvider>>,
        vfs_service: Option<Arc<dyn VfsStatsProvider>>,
    ) -> Self {
        Self {
            db,
            logging_service,
            vfs_service,
            auth_health_provider: None,
            plugin_health_provider: None,
        }
    }

    /// Create a dashboard stats service with all available integrations
    pub fn with_health_integrations(
        db: Arc<dyn Db>,
        logging_service: Option<Arc<dyn LoggingStatsProvider>>,
        vfs_service: Option<Arc<dyn VfsStatsProvider>>,
        auth_health_provider: Option<Arc<dyn AuthHealthProvider>>,
        plugin_health_provider: Option<Arc<dyn PluginHealthProvider>>,
    ) -> Self {
        Self {
            db,
            logging_service,
            vfs_service,
            auth_health_provider,
            plugin_health_provider,
        }
    }

    /// Get enhanced system statistics with real database queries
    async fn get_enhanced_system_stats(&self) -> Result<SystemStats, AppError> {
        debug!("Collecting enhanced system statistics");

        // Get basic stats from database
        let mut system_stats = self.db.get_system_statistics().await?;

        // Enhance with real user activity data if logging is available
        if let Some(ref logging) = self.logging_service {
            match logging.get_user_activity_stats().await {
                Ok(user_stats) => {
                    system_stats.active_users = user_stats.active_24h;
                }
                Err(e) => {
                    warn!("Failed to get user activity stats: {}", e);
                }
            }

            // Get real API request data
            match logging.get_api_stats().await {
                Ok(api_stats) => {
                    system_stats.api_requests_24h = api_stats.requests_24h;

                    // Calculate growth trends based on 7-day vs 24-hour data
                    if api_stats.requests_7d > 0 {
                        let daily_average_7d = api_stats.requests_7d as f64 / 7.0;
                        let growth_percent = if daily_average_7d > 0.0 {
                            ((system_stats.api_requests_24h as f64 - daily_average_7d)
                                / daily_average_7d)
                                * 100.0
                        } else {
                            0.0
                        };
                        system_stats.trends.api_growth_percent = growth_percent;
                    }
                }
                Err(e) => {
                    warn!("Failed to get API stats: {}", e);
                }
            }
        }

        Ok(system_stats)
    }

    /// Get comprehensive system health status
    async fn get_comprehensive_health(&self) -> Result<SystemHealth, AppError> {
        debug!("Collecting comprehensive system health");

        let storage_usage = self.db.get_storage_usage().await?;

        // Check database health
        let database_status = match self.db.health_check().await {
            Ok(_) => HealthStatus::Healthy,
            Err(_) => HealthStatus::Unhealthy,
        };

        // Get authentication system health
        let auth_status = if let Some(ref auth_provider) = self.auth_health_provider {
            match auth_provider.get_auth_health().await {
                Ok(status) => status,
                Err(e) => {
                    warn!("Failed to get auth health: {}", e);
                    HealthStatus::Unhealthy
                }
            }
        } else {
            HealthStatus::Unknown
        };

        // Get plugin system health
        let plugin_status = if let Some(ref plugin_provider) = self.plugin_health_provider {
            match plugin_provider.get_plugin_health().await {
                Ok(status) => status,
                Err(e) => {
                    warn!("Failed to get plugin health: {}", e);
                    HealthStatus::Unhealthy
                }
            }
        } else {
            HealthStatus::Unknown
        };

        // Get API and logging health from logging service
        let api_status = if let Some(ref logging) = self.logging_service {
            match logging.get_logging_health().await {
                Ok(status) => status,
                Err(e) => {
                    warn!("Failed to get API/logging health: {}", e);
                    HealthStatus::Unhealthy
                }
            }
        } else {
            HealthStatus::Unknown
        };

        // Get VFS health
        let vfs_status = if let Some(ref vfs) = self.vfs_service {
            vfs.get_vfs_health().await.unwrap_or(HealthStatus::Unknown)
        } else {
            HealthStatus::Unknown
        };

        let uptime_seconds = PROCESS_START.elapsed().as_secs();

        Ok(SystemHealth {
            database_status,
            api_status,
            auth_status,
            plugin_status,
            vfs_status,
            storage_usage,
            uptime_seconds,
        })
    }

    /// Get real user statistics from logging data
    async fn get_real_user_stats(&self) -> Result<UserStats, AppError> {
        if let Some(ref logging) = self.logging_service {
            logging.get_user_activity_stats().await
        } else {
            // Return empty stats if no logging service available
            Ok(UserStats {
                total_users: 0,
                active_24h: 0,
                active_7d: 0,
                top_active_users: vec![],
            })
        }
    }

    /// Get real API statistics from logging data
    async fn get_real_api_stats(&self) -> Result<ApiStats, AppError> {
        if let Some(ref logging) = self.logging_service {
            logging.get_api_stats().await
        } else {
            // Return empty stats if no logging service available
            Ok(ApiStats {
                requests_24h: 0,
                requests_7d: 0,
                avg_response_time_ms: 0.0,
                error_rate_percent: 0.0,
                top_endpoints: vec![],
            })
        }
    }
}

/// Bridge implementation for authentication health checks
pub struct AuthHealthBridge {
    /// Database service for validating auth collection schemas
    db: Arc<dyn Db>,
    /// Authentication service for validating configured collections and JWT signing
    auth_service: Arc<AuthService>,
}

impl AuthHealthBridge {
    pub fn new(db: Arc<dyn Db>, auth_service: Arc<AuthService>) -> Self {
        Self { db, auth_service }
    }
}

#[async_trait]
impl AuthHealthProvider for AuthHealthBridge {
    async fn get_auth_health(&self) -> Result<HealthStatus, AppError> {
        let auth_schemas = self.db.list_auth_collections().await?;
        let configured_collections = self.auth_service.config().list_auth_collections();

        if auth_schemas.is_empty() && configured_collections.is_empty() {
            return Ok(HealthStatus::Warning);
        }

        if auth_schemas.is_empty() && !configured_collections.is_empty() {
            warn!(
                "Auth service has {} configured collections but database has no auth collections",
                configured_collections.len()
            );
            return Ok(HealthStatus::Unhealthy);
        }

        let mut missing_config_count = 0usize;
        let mut missing_schema = false;
        let mut invalid_schema = false;

        for schema in &auth_schemas {
            if schema.collection_type != CollectionType::Auth {
                invalid_schema = true;
                warn!(
                    "Collection '{}' returned as auth collection but has type {:?}",
                    schema.name, schema.collection_type
                );
                continue;
            }

            let Some(config) = self.auth_service.config().get_auth_collection(&schema.name) else {
                missing_config_count += 1;
                warn!(
                    "Auth collection '{}' exists in database but is not configured in AuthService",
                    schema.name
                );
                continue;
            };

            if !schema.fields.contains_key(&config.identifier_field) {
                invalid_schema = true;
                warn!(
                    "Auth collection '{}' is missing configured identifier field '{}'",
                    schema.name, config.identifier_field
                );
            }

            if !schema.fields.contains_key(&config.credential_field) {
                invalid_schema = true;
                warn!(
                    "Auth collection '{}' is missing configured credential field '{}'",
                    schema.name, config.credential_field
                );
            }
        }

        for configured_name in &configured_collections {
            if !auth_schemas
                .iter()
                .any(|schema| schema.name == *configured_name)
            {
                missing_schema = true;
                warn!(
                    "AuthService collection '{}' is configured but missing from database schemas",
                    configured_name
                );
            }
        }

        if invalid_schema || missing_schema {
            return Ok(HealthStatus::Unhealthy);
        }

        if let Some(collection_name) = configured_collections.first() {
            if let Some(config) = self
                .auth_service
                .config()
                .get_auth_collection(collection_name)
            {
                let token = self.auth_service.generate_token(
                    "health-check".to_string(),
                    "health@example.invalid".to_string(),
                    config.default_role.clone(),
                    collection_name.clone(),
                )?;
                self.auth_service.verify_token(&token)?;
            }
        }

        if missing_config_count == auth_schemas.len() {
            Ok(HealthStatus::Unhealthy)
        } else if missing_config_count > 0 {
            Ok(HealthStatus::Degraded)
        } else {
            Ok(HealthStatus::Healthy)
        }
    }
}

#[async_trait]
impl DashboardStatsService for DatabaseDashboardStatsService {
    async fn get_dashboard_stats(&self) -> Result<DashboardStats, oxide_core::error::AppError> {
        debug!("Collecting comprehensive dashboard statistics");

        // Collect all statistics in parallel for better performance
        let (
            system_stats_result,
            collection_stats_result,
            user_stats_result,
            api_stats_result,
            health_result,
            activities_result,
        ) = tokio::join!(
            self.get_enhanced_system_stats(),
            self.db.get_collection_statistics(),
            self.get_real_user_stats(),
            self.get_real_api_stats(),
            self.get_comprehensive_health(),
            self.db.get_recent_dashboard_activities(20)
        );

        let system_stats = system_stats_result.map_err(|e| {
            oxide_core::error::AppError::internal(format!("Failed to get system stats: {}", e))
        })?;
        let collection_stats = collection_stats_result.map_err(|e| {
            oxide_core::error::AppError::internal(format!("Failed to get collection stats: {}", e))
        })?;
        let user_stats = user_stats_result.map_err(|e| {
            oxide_core::error::AppError::internal(format!("Failed to get user stats: {}", e))
        })?;
        let api_stats = api_stats_result.map_err(|e| {
            oxide_core::error::AppError::internal(format!("Failed to get API stats: {}", e))
        })?;
        let system_health = health_result.map_err(|e| {
            oxide_core::error::AppError::internal(format!("Failed to get system health: {}", e))
        })?;

        let recent_activity = activities_result.map_err(|e| {
            oxide_core::error::AppError::internal(format!("Failed to get recent activities: {}", e))
        })?;

        debug!("Successfully collected dashboard statistics: {} collections, {} records, {} activities", 
            system_stats.total_collections, system_stats.total_records, recent_activity.len());

        Ok(DashboardStats {
            system_stats,
            collection_stats,
            user_stats,
            api_stats,
            recent_activity,
            system_health,
            generated_at: Utc::now().to_rfc3339(),
        })
    }

    async fn get_system_stats(&self) -> Result<SystemStats, oxide_core::error::AppError> {
        self.get_enhanced_system_stats().await.map_err(|e| {
            oxide_core::error::AppError::internal(format!("Failed to get system stats: {}", e))
        })
    }

    async fn record_activity(
        &self,
        activity: ActivityEntry,
    ) -> Result<(), oxide_core::error::AppError> {
        debug!("Recording dashboard activity: {:?}", activity.activity_type);

        self.db
            .record_dashboard_activity(activity)
            .await
            .map_err(|e| {
                oxide_core::error::AppError::internal(format!("Failed to record activity: {}", e))
            })
    }

    async fn get_recent_activities(
        &self,
        limit: usize,
    ) -> Result<Vec<ActivityEntry>, oxide_core::error::AppError> {
        debug!("Getting recent dashboard activities with limit: {}", limit);

        self.db
            .get_recent_dashboard_activities(limit)
            .await
            .map_err(|e| {
                oxide_core::error::AppError::internal(format!(
                    "Failed to get recent activities: {}",
                    e
                ))
            })
    }
}

/// Bridge implementation for logging stats integration
pub struct LoggingStatsBridge {
    /// Logging API service for querying statistics
    logging_service: Option<Arc<oxide_logging::api::LogApiService>>,
}

impl LoggingStatsBridge {
    pub fn new() -> Self {
        Self {
            logging_service: None,
        }
    }

    /// Create with actual logging service integration
    pub fn with_service(logging_service: Arc<oxide_logging::api::LogApiService>) -> Self {
        Self {
            logging_service: Some(logging_service),
        }
    }
}

impl Default for LoggingStatsBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LoggingStatsProvider for LoggingStatsBridge {
    async fn get_api_stats(&self) -> Result<ApiStats, AppError> {
        if let Some(ref logging_service) = self.logging_service {
            // Logging exposes aggregate request-adjacent activity metrics. It
            // does not currently expose per-route telemetry, so endpoint stats
            // remain empty instead of synthesized.
            match logging_service.get_dashboard_metrics().await {
                Ok(dashboard_metrics) => {
                    let log_metrics = &dashboard_metrics.log_metrics;

                    Ok(ApiStats {
                        requests_24h: log_metrics.entries_24h,
                        requests_7d: (log_metrics.avg_entries_per_day * 7.0).round() as u64,
                        avg_response_time_ms: dashboard_metrics.health_indicators.avg_query_time_ms,
                        error_rate_percent: log_metrics.error_rate_24h,
                        top_endpoints: vec![],
                    })
                }
                Err(e) => {
                    warn!(
                        "Failed to get dashboard metrics from logging service: {}",
                        e
                    );
                    self.get_fallback_api_stats().await
                }
            }
        } else {
            self.get_fallback_api_stats().await
        }
    }

    async fn get_user_activity_stats(&self) -> Result<UserStats, AppError> {
        if let Some(ref logging_service) = self.logging_service {
            // Get real user statistics from logging data
            match logging_service.get_dashboard_metrics().await {
                Ok(dashboard_metrics) => {
                    let log_metrics = &dashboard_metrics.log_metrics;

                    // Convert top users from log metrics
                    let top_active_users: Vec<UserActivity> = log_metrics
                        .top_users
                        .iter()
                        .take(5)
                        .map(|(username, action_count)| UserActivity {
                            username: username.clone(),
                            action_count: *action_count as u32,
                            last_activity: chrono::Utc::now().to_rfc3339(),
                        })
                        .collect();

                    // Get user activity data from dashboard metrics
                    let total_unique_users = log_metrics.top_users.len() as u32;
                    let active_24h = dashboard_metrics.health_indicators.active_users as u32;
                    let active_7d = (active_24h as f64 * 1.5) as u32; // Estimate 7-day active users

                    Ok(UserStats {
                        total_users: total_unique_users.max(active_7d), // Ensure total >= active
                        active_24h,
                        active_7d,
                        top_active_users,
                    })
                }
                Err(e) => {
                    warn!("Failed to get user activity from logging service: {}", e);
                    self.get_fallback_user_stats().await
                }
            }
        } else {
            self.get_fallback_user_stats().await
        }
    }

    async fn get_logging_health(&self) -> Result<HealthStatus, AppError> {
        if let Some(ref logging_service) = self.logging_service {
            // Check if logging service is responsive
            match logging_service.get_dashboard_metrics().await {
                Ok(_) => Ok(HealthStatus::Healthy),
                Err(_) => Ok(HealthStatus::Unhealthy),
            }
        } else {
            Ok(HealthStatus::Unknown)
        }
    }
}

impl LoggingStatsBridge {
    /// Fallback API stats when logging service is unavailable
    async fn get_fallback_api_stats(&self) -> Result<ApiStats, AppError> {
        // Return empty data when logging service is unavailable
        Ok(ApiStats {
            requests_24h: 0,
            requests_7d: 0,
            avg_response_time_ms: 0.0,
            error_rate_percent: 0.0,
            top_endpoints: vec![],
        })
    }

    /// Fallback user stats when logging service is unavailable
    async fn get_fallback_user_stats(&self) -> Result<UserStats, AppError> {
        // Return empty data when logging service is unavailable
        Ok(UserStats {
            total_users: 0,
            active_24h: 0,
            active_7d: 0,
            top_active_users: vec![],
        })
    }
}

/// Bridge implementation for VFS stats integration
pub struct VfsStatsBridge {
    /// VFS service for querying statistics
    vfs_service: Option<Arc<dyn oxide_core::VirtualFileSystem>>,
}

impl VfsStatsBridge {
    pub fn new() -> Self {
        Self { vfs_service: None }
    }

    /// Create with actual VFS service integration
    pub fn with_service(vfs_service: Arc<dyn oxide_core::VirtualFileSystem>) -> Self {
        Self {
            vfs_service: Some(vfs_service),
        }
    }
}

impl Default for VfsStatsBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VfsStatsProvider for VfsStatsBridge {
    async fn get_vfs_storage_usage(&self) -> Result<u64, AppError> {
        if let Some(ref vfs_service) = self.vfs_service {
            // Get real VFS storage usage from all namespaces
            // For now, we'll use a default namespace as an example
            let default_namespace = "default".to_string();
            match vfs_service.get_usage_stats(&default_namespace).await {
                Ok(usage_stats) => Ok(usage_stats.storage_used),
                Err(e) => {
                    warn!("Failed to get VFS usage stats: {}", e);
                    self.get_fallback_storage_usage().await
                }
            }
        } else {
            self.get_fallback_storage_usage().await
        }
    }

    async fn get_vfs_health(&self) -> Result<HealthStatus, AppError> {
        if let Some(ref vfs_service) = self.vfs_service {
            // Check if VFS service is responsive by trying to get usage stats
            let default_namespace = "default".to_string();
            match vfs_service.get_usage_stats(&default_namespace).await {
                Ok(_) => Ok(HealthStatus::Healthy),
                Err(_) => Ok(HealthStatus::Unhealthy),
            }
        } else {
            Ok(HealthStatus::Unknown)
        }
    }
}

impl VfsStatsBridge {
    /// Fallback storage usage when VFS service is unavailable
    async fn get_fallback_storage_usage(&self) -> Result<u64, AppError> {
        // Return zero when VFS service is unavailable
        Ok(0)
    }
}

// Global process start time used for uptime calculations
pub static PROCESS_START: Lazy<Instant> = Lazy::new(Instant::now);
