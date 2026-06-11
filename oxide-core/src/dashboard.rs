//! Dashboard statistics and metrics
//!
//! This module defines the data structures for dashboard statistics
//! that provide insights into system usage, performance, and health.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Comprehensive dashboard statistics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DashboardStats {
    /// Basic system statistics
    pub system_stats: SystemStats,
    /// Collection-related statistics
    pub collection_stats: Vec<CollectionStatsEntry>,
    /// User activity statistics
    pub user_stats: UserStats,
    /// API usage statistics
    pub api_stats: ApiStats,
    /// Recent activity entries
    pub recent_activity: Vec<ActivityEntry>,
    /// System health indicators
    pub system_health: SystemHealth,
    /// Timestamp when these stats were generated (ISO 8601 string)
    #[ts(type = "string")]
    pub generated_at: String,
}

/// Basic system statistics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemStats {
    /// Total number of collections
    pub total_collections: u32,
    /// Total number of records across all collections
    pub total_records: u64,
    /// Number of active users (users with recent activity)
    pub active_users: u32,
    /// Number of API requests in the last 24 hours
    pub api_requests_24h: u64,
    /// Growth trends
    pub trends: GrowthTrends,
}

/// Growth trends for various metrics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GrowthTrends {
    /// Collections added this month
    pub collections_this_month: i32,
    /// Records growth percentage from last month
    pub records_growth_percent: f64,
    /// New users added recently
    pub new_users_count: u32,
    /// API requests growth percentage from yesterday
    pub api_growth_percent: f64,
}

/// Statistics for a specific collection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CollectionStatsEntry {
    /// Collection name
    pub name: String,
    /// Number of records in this collection
    pub record_count: u64,
    /// Collection size in bytes
    pub size_bytes: u64,
    /// When the collection was created (ISO 8601 string)
    #[ts(type = "string | null")]
    pub created_at: Option<String>,
    /// When the collection was last modified (ISO 8601 string)
    #[ts(type = "string | null")]
    pub last_modified: Option<String>,
    /// Whether this is a system collection
    pub is_system: bool,
}

/// User activity statistics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UserStats {
    /// Total number of registered users
    pub total_users: u32,
    /// Users active in the last 24 hours
    pub active_24h: u32,
    /// Users active in the last 7 days
    pub active_7d: u32,
    /// Most active users
    pub top_active_users: Vec<UserActivity>,
}

/// Individual user activity information
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UserActivity {
    /// Username or identifier
    pub username: String,
    /// Number of actions performed
    pub action_count: u32,
    /// Last activity timestamp (ISO 8601 string)
    #[ts(type = "string")]
    pub last_activity: String,
}

/// API usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiStats {
    /// Total requests in the last 24 hours
    pub requests_24h: u64,
    /// Total requests in the last 7 days
    pub requests_7d: u64,
    /// Average response time in milliseconds
    pub avg_response_time_ms: f64,
    /// Error rate percentage
    pub error_rate_percent: f64,
    /// Most frequently accessed endpoints
    pub top_endpoints: Vec<EndpointStats>,
}

/// Statistics for individual API endpoints
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EndpointStats {
    /// Endpoint path
    pub path: String,
    /// HTTP method
    pub method: String,
    /// Number of requests
    pub request_count: u64,
    /// Average response time
    pub avg_response_time_ms: f64,
}

/// Recent activity entry
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActivityEntry {
    /// Activity timestamp (ISO 8601 string)
    #[ts(type = "string")]
    pub timestamp: String,
    /// Type of activity
    pub activity_type: ActivityType,
    /// User who performed the activity
    pub user: String,
    /// Description of the activity
    pub description: String,
    /// Related collection (if applicable)
    pub collection: Option<String>,
    /// Additional metadata (JSON object as string)
    #[ts(type = "any")]
    pub metadata: Option<serde_json::Value>,
}

/// Types of activities that can be tracked
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ActivityType {
    /// Collection was created
    CollectionCreated,
    /// Collection was deleted
    CollectionDeleted,
    /// Collection schema was modified
    CollectionModified,
    /// Record was created
    RecordCreated,
    /// Record was updated
    RecordUpdated,
    /// Record was deleted
    RecordDeleted,
    /// User was registered
    UserRegistered,
    /// User logged in
    UserLogin,
    /// User logged out
    UserLogout,
    /// Authentication failed
    AuthenticationFailed,
    /// Permission was granted
    PermissionGranted,
    /// Permission was revoked
    PermissionRevoked,
    /// Plugin was installed
    PluginInstalled,
    /// Plugin was enabled/disabled
    PluginToggled,
    /// System maintenance
    SystemMaintenance,
    /// Other activity
    Other(String),
}

/// System health indicators
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemHealth {
    /// Database connection status
    pub database_status: HealthStatus,
    /// API endpoints status
    pub api_status: HealthStatus,
    /// Authentication service status
    pub auth_status: HealthStatus,
    /// Plugin system status
    pub plugin_status: HealthStatus,
    /// VFS status
    pub vfs_status: HealthStatus,
    /// Storage usage information
    pub storage_usage: StorageUsage,
    /// System uptime in seconds
    pub uptime_seconds: u64,
}

/// Health status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum HealthStatus {
    /// Service is healthy and operational
    Healthy,
    /// Service has minor issues but is operational
    Warning,
    /// Service is experiencing significant issues
    Degraded,
    /// Service is not operational
    Unhealthy,
    /// Service status is unknown
    Unknown,
}

/// Storage usage information
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StorageUsage {
    /// Used storage in bytes
    pub used_bytes: u64,
    /// Total available storage in bytes
    pub total_bytes: u64,
    /// Usage percentage
    pub usage_percent: f64,
    /// Database file size in bytes
    pub database_size_bytes: u64,
    /// Log files size in bytes
    pub logs_size_bytes: u64,
    /// VFS files size in bytes
    pub vfs_size_bytes: u64,
}

/// Service trait for gathering dashboard statistics
///
/// This trait defines the interface for collecting dashboard statistics
/// from various system components. Implementations should gather data
/// efficiently and provide accurate metrics.
#[async_trait::async_trait]
pub trait DashboardStatsService: Send + Sync {
    /// Get comprehensive dashboard statistics
    ///
    /// This method collects statistics from all system components
    /// and returns a complete dashboard view.
    ///
    /// # Returns
    /// Complete dashboard statistics or an error if collection fails
    async fn get_dashboard_stats(&self) -> Result<DashboardStats, crate::error::AppError>;

    /// Get basic system statistics only
    ///
    /// This is a lighter-weight version that returns only core metrics
    /// for situations where full statistics are not needed.
    ///
    /// # Returns
    /// Basic system statistics or an error if collection fails
    async fn get_system_stats(&self) -> Result<SystemStats, crate::error::AppError>;

    /// Record an activity for the activity feed
    ///
    /// This method records user and system activities for display
    /// in the dashboard activity feed.
    ///
    /// # Arguments
    /// * `activity` - The activity entry to record
    ///
    /// # Returns
    /// Success or an error if recording fails
    async fn record_activity(&self, activity: ActivityEntry) -> Result<(), crate::error::AppError>;

    /// Get recent activities for the dashboard
    ///
    /// # Arguments
    /// * `limit` - Maximum number of activities to return
    ///
    /// # Returns
    /// Vector of recent activities or an error if retrieval fails
    async fn get_recent_activities(
        &self,
        limit: usize,
    ) -> Result<Vec<ActivityEntry>, crate::error::AppError>;
}
