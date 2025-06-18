//! HTTP API endpoints for accessing logs and metrics
//!
//! This module provides REST API endpoints for:
//! - Querying logs with filtering and pagination
//! - Retrieving audit events
//! - Getting logging metrics and statistics
//! - Managing retention policies
//! - Real-time log streaming

use crate::{
    error::{LoggingError, LoggingResult},
    models::{LogQuery, LogFilter, LogLevel, AuditEventType, CorrelationId, LogMetrics},
    service::LogService,
    retention::{RetentionService, RetentionStats},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Query parameters for log retrieval
#[derive(Debug, Deserialize)]
pub struct LogQueryParams {
    /// Minimum log level (error, warn, info, debug, trace)
    pub level: Option<String>,
    /// Start time for filtering (ISO 8601 format)
    pub start_time: Option<String>,
    /// End time for filtering (ISO 8601 format)  
    pub end_time: Option<String>,
    /// Correlation ID for filtering
    pub correlation_id: Option<String>,
    /// Module name for filtering
    pub module: Option<String>,
    /// User ID for filtering
    pub user_id: Option<String>,
    /// Collection name for filtering
    pub collection: Option<String>,
    /// Search text in log messages
    pub search: Option<String>,
    /// Number of entries to return (default: 100, max: 1000)
    pub limit: Option<usize>,
    /// Number of entries to skip (for pagination)
    pub offset: Option<usize>,
    /// Sort order: 'desc' for newest first, 'asc' for oldest first
    pub sort: Option<String>,
}

/// Query parameters for audit events
#[derive(Debug, Deserialize)]
pub struct AuditQueryParams {
    /// Minimum severity level
    pub severity: Option<String>,
    /// Start time for filtering
    pub start_time: Option<String>,
    /// End time for filtering
    pub end_time: Option<String>,
    /// Event type filter
    pub event_type: Option<String>,
    /// Actor filter
    pub actor: Option<String>,
    /// Target filter
    pub target: Option<String>,
    /// Correlation ID for filtering
    pub correlation_id: Option<String>,
    /// Minimum risk score filter
    pub min_risk_score: Option<u8>,
    /// Number of entries to return
    pub limit: Option<usize>,
    /// Number of entries to skip
    pub offset: Option<usize>,
    /// Sort order
    pub sort: Option<String>,
}

/// Response structure for paginated log results
#[derive(Debug, Serialize)]
pub struct LogResponse<T> {
    /// The log entries or audit events
    pub data: Vec<T>,
    /// Pagination information
    pub pagination: PaginationInfo,
    /// Query metadata
    pub metadata: QueryMetadata,
}

/// Pagination information
#[derive(Debug, Serialize)]
pub struct PaginationInfo {
    /// Current page offset
    pub offset: usize,
    /// Number of items in this response
    pub limit: usize,
    /// Total number of items matching the query
    pub total: Option<u64>,
    /// Whether there are more results available
    pub has_more: bool,
}

/// Query execution metadata
#[derive(Debug, Serialize)]
pub struct QueryMetadata {
    /// Query execution time in milliseconds
    pub execution_time_ms: u64,
    /// Timestamp when the query was executed
    pub executed_at: DateTime<Utc>,
    /// Applied filters summary
    pub filters_applied: Vec<String>,
}

/// Real-time log streaming configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct StreamConfig {
    /// Minimum log level to stream
    pub min_level: Option<String>,
    /// Filter by module
    pub module: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter by collection
    pub collection: Option<String>,
    /// Maximum number of logs to buffer
    pub buffer_size: Option<usize>,
}

/// Metrics dashboard data
#[derive(Debug, Serialize)]
pub struct DashboardMetrics {
    /// Basic log metrics
    pub log_metrics: LogMetrics,
    /// Recent error rate trends
    pub error_trends: Vec<ErrorTrendPoint>,
    /// Top error sources
    pub top_error_sources: Vec<ErrorSource>,
    /// Activity by user
    pub user_activity: Vec<UserActivity>,
    /// Collection access patterns
    pub collection_stats: Vec<CollectionStats>,
    /// System health indicators
    pub health_indicators: HealthIndicators,
}

/// Error trend data point
#[derive(Debug, Serialize)]
pub struct ErrorTrendPoint {
    /// Time bucket
    pub timestamp: DateTime<Utc>,
    /// Error count in this time bucket
    pub error_count: u64,
    /// Total log count in this time bucket
    pub total_count: u64,
    /// Error rate percentage
    pub error_rate: f64,
}

/// Error source information
#[derive(Debug, Serialize)]
pub struct ErrorSource {
    /// Module or component name
    pub module: String,
    /// Number of errors from this source
    pub error_count: u64,
    /// Most recent error message
    pub latest_error: Option<String>,
    /// Most recent error timestamp
    pub latest_timestamp: Option<DateTime<Utc>>,
}

/// User activity summary
#[derive(Debug, Serialize)]
pub struct UserActivity {
    /// User ID
    pub user_id: String,
    /// Number of actions
    pub action_count: u64,
    /// Number of errors
    pub error_count: u64,
    /// Most recent activity
    pub last_activity: DateTime<Utc>,
}

/// Collection usage statistics
#[derive(Debug, Serialize)]
pub struct CollectionStats {
    /// Collection name
    pub name: String,
    /// Number of access events
    pub access_count: u64,
    /// Number of modification events
    pub modification_count: u64,
    /// Most recent access
    pub last_accessed: DateTime<Utc>,
}

/// System health indicators
#[derive(Debug, Serialize)]
pub struct HealthIndicators {
    /// Current error rate
    pub current_error_rate: f64,
    /// Log ingestion rate (logs per minute)
    pub ingestion_rate: f64,
    /// Storage utilization percentage
    pub storage_utilization: f64,
    /// Average response time for queries
    pub avg_query_time_ms: f64,
    /// Number of active users
    pub active_users: u64,
}

/// HTTP API service for log access
pub struct LogApiService {
    /// Log service instance
    log_service: Arc<LogService>,
    /// Retention service instance
    retention_service: Option<Arc<RetentionService>>,
}

impl LogApiService {
    /// Create a new API service
    pub fn new(log_service: Arc<LogService>) -> Self {
        Self {
            log_service,
            retention_service: None,
        }
    }

    /// Create API service with retention service
    pub fn with_retention(log_service: Arc<LogService>, retention_service: Arc<RetentionService>) -> Self {
        Self {
            log_service,
            retention_service: Some(retention_service),
        }
    }

    /// Query logs with parameters
    pub async fn query_logs(&self, params: LogQueryParams) -> LoggingResult<LogResponse<crate::models::LogEntry>> {
        let start_time = std::time::Instant::now();
        
        // Parse and validate parameters
        let query = self.parse_log_query_params(params)?;
        
        // Execute query
        let logs = self.log_service.query(query.clone()).await?;
        
        // Calculate metadata
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let filters_applied = self.get_applied_filters(&query.filter);
        
        let response = LogResponse {
            data: logs,
            pagination: PaginationInfo {
                offset: query.offset.unwrap_or(0),
                limit: query.limit.unwrap_or(100),
                total: None, // Would require a separate count query
                has_more: false, // Would be calculated based on total and current results
            },
            metadata: QueryMetadata {
                execution_time_ms,
                executed_at: Utc::now(),
                filters_applied,
            },
        };

        Ok(response)
    }

    /// Query audit events
    pub async fn query_audit_events(&self, params: AuditQueryParams) -> LoggingResult<LogResponse<crate::models::SecurityAuditEvent>> {
        let start_time = std::time::Instant::now();
        
        // Parse parameters into LogQuery (reusing the same structure)
        let query = self.parse_audit_query_params(params)?;
        
        // Execute query
        let events = self.log_service.query_audit_events(query.clone()).await?;
        
        let execution_time_ms = start_time.elapsed().as_millis() as u64;
        let filters_applied = self.get_applied_filters(&query.filter);
        
        let response = LogResponse {
            data: events,
            pagination: PaginationInfo {
                offset: query.offset.unwrap_or(0),
                limit: query.limit.unwrap_or(100),
                total: None,
                has_more: false,
            },
            metadata: QueryMetadata {
                execution_time_ms,
                executed_at: Utc::now(),
                filters_applied,
            },
        };

        Ok(response)
    }

    /// Get logging metrics for dashboard
    pub async fn get_dashboard_metrics(&self) -> LoggingResult<DashboardMetrics> {
        let log_metrics = self.log_service.get_metrics().await?;
        
        // For now, return basic metrics
        // In a full implementation, you would calculate trends and detailed statistics
        Ok(DashboardMetrics {
            log_metrics,
            error_trends: Vec::new(),
            top_error_sources: Vec::new(),
            user_activity: Vec::new(),
            collection_stats: Vec::new(),
            health_indicators: HealthIndicators {
                current_error_rate: 0.0,
                ingestion_rate: 0.0,
                storage_utilization: 0.0,
                avg_query_time_ms: 0.0,
                active_users: 0,
            },
        })
    }

    /// Get retention statistics
    pub async fn get_retention_stats(&self) -> LoggingResult<Option<RetentionStats>> {
        if let Some(ref retention_service) = self.retention_service {
            Ok(Some(retention_service.get_retention_stats().await?))
        } else {
            Ok(None)
        }
    }

    /// Get recent logs from memory cache
    pub fn get_recent_logs(&self, limit: Option<usize>) -> Vec<crate::models::LogEntry> {
        self.log_service.get_recent_logs(limit.unwrap_or(50))
    }

    /// Parse log query parameters into LogQuery
    fn parse_log_query_params(&self, params: LogQueryParams) -> LoggingResult<LogQuery> {
        let mut filter = LogFilter::default();

        // Parse log level
        if let Some(level_str) = params.level {
            filter.min_level = LogLevel::from_str(&level_str);
            if filter.min_level.is_none() {
                return Err(LoggingError::invalid_input(format!("Invalid log level: {}", level_str)));
            }
        }

        // Parse timestamps
        if let Some(start_str) = params.start_time {
            filter.start_time = Some(DateTime::parse_from_rfc3339(&start_str)
                .map_err(|_| LoggingError::invalid_input("Invalid start_time format"))?
                .with_timezone(&Utc));
        }

        if let Some(end_str) = params.end_time {
            filter.end_time = Some(DateTime::parse_from_rfc3339(&end_str)
                .map_err(|_| LoggingError::invalid_input("Invalid end_time format"))?
                .with_timezone(&Utc));
        }

        // Parse correlation ID
        if let Some(corr_str) = params.correlation_id {
            filter.correlation_id = Some(CorrelationId::from_str(&corr_str)
                .map_err(|_| LoggingError::invalid_input("Invalid correlation_id format"))?);
        }

        // Set other filters
        filter.module = params.module;
        filter.user_id = params.user_id;
        filter.collection = params.collection;
        filter.message_contains = params.search;

        // Validate and set pagination
        let limit = params.limit.map(|l| l.min(1000)); // Cap at 1000
        let offset = params.offset;

        // Parse sort order
        let sort_desc = params.sort.as_deref() != Some("asc");

        Ok(LogQuery {
            filter,
            limit,
            offset,
            sort_desc,
        })
    }

    /// Parse audit query parameters
    fn parse_audit_query_params(&self, params: AuditQueryParams) -> LoggingResult<LogQuery> {
        let mut filter = LogFilter::default();

        // Parse severity level
        if let Some(severity_str) = params.severity {
            filter.min_level = LogLevel::from_str(&severity_str);
            if filter.min_level.is_none() {
                return Err(LoggingError::invalid_input(format!("Invalid severity level: {}", severity_str)));
            }
        }

        // Parse timestamps
        if let Some(start_str) = params.start_time {
            filter.start_time = Some(DateTime::parse_from_rfc3339(&start_str)
                .map_err(|_| LoggingError::invalid_input("Invalid start_time format"))?
                .with_timezone(&Utc));
        }

        if let Some(end_str) = params.end_time {
            filter.end_time = Some(DateTime::parse_from_rfc3339(&end_str)
                .map_err(|_| LoggingError::invalid_input("Invalid end_time format"))?
                .with_timezone(&Utc));
        }

        // Parse event type
        if let Some(event_type_str) = params.event_type {
            filter.audit_event_type = match event_type_str.as_str() {
                "authentication" => Some(AuditEventType::Authentication),
                "authorization" => Some(AuditEventType::Authorization),
                "data_access" => Some(AuditEventType::DataAccess),
                "data_modification" => Some(AuditEventType::DataModification),
                "configuration_change" => Some(AuditEventType::ConfigurationChange),
                "security_violation" => Some(AuditEventType::SecurityViolation),
                "plugin_event" => Some(AuditEventType::PluginEvent),
                "system_event" => Some(AuditEventType::SystemEvent),
                _ => return Err(LoggingError::invalid_input(format!("Invalid event type: {}", event_type_str))),
            };
        }

        // Parse correlation ID
        if let Some(corr_str) = params.correlation_id {
            filter.correlation_id = Some(CorrelationId::from_str(&corr_str)
                .map_err(|_| LoggingError::invalid_input("Invalid correlation_id format"))?);
        }

        // Set pagination
        let limit = params.limit.map(|l| l.min(1000));
        let offset = params.offset;
        let sort_desc = params.sort.as_deref() != Some("asc");

        Ok(LogQuery {
            filter,
            limit,
            offset,
            sort_desc,
        })
    }

    /// Get list of applied filters for metadata
    fn get_applied_filters(&self, filter: &LogFilter) -> Vec<String> {
        let mut filters = Vec::new();

        if filter.min_level.is_some() {
            filters.push("min_level".to_string());
        }
        if filter.start_time.is_some() {
            filters.push("start_time".to_string());
        }
        if filter.end_time.is_some() {
            filters.push("end_time".to_string());
        }
        if filter.correlation_id.is_some() {
            filters.push("correlation_id".to_string());
        }
        if filter.module.is_some() {
            filters.push("module".to_string());
        }
        if filter.user_id.is_some() {
            filters.push("user_id".to_string());
        }
        if filter.collection.is_some() {
            filters.push("collection".to_string());
        }
        if filter.message_contains.is_some() {
            filters.push("message_search".to_string());
        }
        if filter.audit_event_type.is_some() {
            filters.push("audit_event_type".to_string());
        }

        filters
    }
}

/// WebSocket message types for real-time log streaming
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// Subscribe to log stream
    Subscribe { config: StreamConfig },
    /// Unsubscribe from log stream
    Unsubscribe,
    /// New log entry
    LogEntry { entry: crate::models::LogEntry },
    /// New audit event
    AuditEvent { event: crate::models::SecurityAuditEvent },
    /// Error message
    Error { message: String },
    /// Heartbeat/ping
    Ping,
    /// Heartbeat/pong
    Pong,
} 