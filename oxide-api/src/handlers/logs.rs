//! HTTP handlers for logging and audit endpoints
//!
//! This module provides REST API endpoints for:
//! - Querying logs with filtering and pagination
//! - Retrieving audit events
//! - Getting logging metrics and statistics
//! - Dashboard data for frontend

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json, Json as AxumJson,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::responses::ApiResponse;
use crate::server::AppState;
use oxide_core::{ApplicationLogger, LogContext, LogLevel, SecurityAuditor};
use oxide_logging::api::{AuditQueryParams, DashboardMetrics, LogQueryParams, LogResponse};

/// Request body for creating a manual log entry
#[derive(Debug, Deserialize)]
pub struct CreateLogRequest {
    /// Log level
    pub level: String,
    /// Log message
    pub message: String,
    /// Module or component name
    pub module: String,
    /// Optional context
    pub context: Option<LogContext>,
}

/// Request body for creating a manual audit event
#[derive(Debug, Deserialize)]
pub struct CreateAuditRequest {
    /// Event type
    pub event_type: String,
    /// Severity level
    pub severity: String,
    /// Event description
    pub description: String,
    /// Actor who performed the action
    pub actor: String,
    /// Target of the action (optional)
    pub target: Option<String>,
    /// Action performed
    pub action: String,
    /// Result of the action
    pub result: String,
    /// Optional context
    pub context: Option<LogContext>,
    /// Optional risk score (0-100)
    pub risk_score: Option<u8>,
}

/// Response for successful log/audit creation
#[derive(Debug, Serialize)]
pub struct CreateLogResponse {
    /// ID of the created entry
    pub id: Uuid,
    /// Success message
    pub message: String,
}

/// Get logs with filtering and pagination
pub async fn get_logs(
    State(state): State<AppState>,
    Query(params): Query<LogQueryParams>,
) -> Result<AxumJson<ApiResponse<LogResponse<oxide_logging::LogEntry>>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting logs with params: {:?}", params);

    match service.query_logs(params).await {
        Ok(response) => Ok(AxumJson(ApiResponse::success(response))),
        Err(e) => {
            error!("Failed to query logs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get audit events with filtering and pagination
pub async fn get_audit_events(
    State(state): State<AppState>,
    Query(params): Query<AuditQueryParams>,
) -> Result<AxumJson<ApiResponse<LogResponse<oxide_logging::SecurityAuditEvent>>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting audit events with params: {:?}", params);

    match service.query_audit_events(params).await {
        Ok(response) => Ok(AxumJson(ApiResponse::success(response))),
        Err(e) => {
            error!("Failed to query audit events: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get dashboard metrics for the admin UI
pub async fn get_dashboard_metrics(
    State(state): State<AppState>,
) -> Result<AxumJson<ApiResponse<DashboardMetrics>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting dashboard metrics");

    match service.get_dashboard_metrics().await {
        Ok(metrics) => Ok(AxumJson(ApiResponse::success(metrics))),
        Err(e) => {
            error!("Failed to get dashboard metrics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get recent logs for quick viewing
pub async fn get_recent_logs(
    State(state): State<AppState>,
    Query(params): Query<RecentLogsParams>,
) -> Result<AxumJson<ApiResponse<Vec<oxide_logging::LogEntry>>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting recent logs with limit: {:?}", params.limit);

    let logs = service.get_recent_logs(params.limit);
    Ok(AxumJson(ApiResponse::success(logs)))
}

/// Get retention statistics
pub async fn get_retention_stats(
    State(state): State<AppState>,
) -> Result<AxumJson<ApiResponse<Option<oxide_logging::retention::RetentionStats>>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting retention statistics");

    match service.get_retention_stats().await {
        Ok(stats) => Ok(AxumJson(ApiResponse::success(stats))),
        Err(e) => {
            error!("Failed to get retention stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Create a manual log entry (admin only)
pub async fn create_log_entry(
    State(state): State<AppState>,
    Json(request): Json<CreateLogRequest>,
) -> Result<AxumJson<ApiResponse<CreateLogResponse>>, StatusCode> {
    let logger = match &state.logging_service {
        Some(logger) => logger,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Creating manual log entry: {:?}", request);

    // Parse log level
    let level = match request.level.parse::<LogLevel>() {
        Ok(level) => level,
        Err(_) => {
            warn!("Invalid log level: {}", request.level);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    // Create log entry
    let result = if let Some(context) = request.context {
        logger
            .log_with_context(level, request.message, request.module, context)
            .await
    } else {
        match level {
            LogLevel::Error => logger.error(request.message, request.module).await,
            LogLevel::Warn => logger.warn(request.message, request.module).await,
            LogLevel::Info => logger.info(request.message, request.module).await,
            LogLevel::Debug => logger.debug(request.message, request.module).await,
            LogLevel::Trace => logger.trace(request.message, request.module).await,
        }
    };

    match result {
        Ok(_) => {
            let response = CreateLogResponse {
                id: Uuid::new_v4(), // In practice, you'd get this from the service
                message: "Log entry created successfully".to_string(),
            };
            Ok(AxumJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to create log entry: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Create a manual audit event (admin only)
pub async fn create_audit_event(
    State(state): State<AppState>,
    Json(request): Json<CreateAuditRequest>,
) -> Result<AxumJson<ApiResponse<CreateLogResponse>>, StatusCode> {
    let auditor = match &state.logging_service {
        Some(auditor) => auditor,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Creating manual audit event: {:?}", request);

    let context = request.context.unwrap_or_default();

    // Route to appropriate audit function based on event type
    let result = match request.event_type.as_str() {
        "authentication" => {
            auditor
                .log_authentication(
                    request.actor,
                    request.action,
                    request.result,
                    context,
                    request.risk_score,
                )
                .await
        }
        "authorization" => {
            auditor
                .log_authorization(
                    request.actor,
                    request.target.unwrap_or_default(),
                    request.action,
                    request.result,
                    context,
                    request.risk_score,
                )
                .await
        }
        "data_access" => {
            auditor
                .log_data_access(
                    request.actor,
                    request.target.unwrap_or_default(),
                    request.action,
                    context,
                )
                .await
        }
        "data_modification" => {
            auditor
                .log_data_modification(
                    request.actor,
                    request.target.unwrap_or_default(),
                    request.action,
                    context,
                )
                .await
        }
        "configuration_change" => {
            auditor
                .log_configuration_change(
                    request.actor,
                    request.target.unwrap_or_default(),
                    request.action,
                    context,
                )
                .await
        }
        "security_violation" => {
            auditor
                .log_security_violation(request.actor, request.action, request.description, context)
                .await
        }
        "plugin_event" => {
            auditor
                .log_plugin_event(request.actor, request.action, request.result, context)
                .await
        }
        "system_event" => {
            auditor
                .log_system_event(request.action, request.description, context)
                .await
        }
        _ => {
            warn!("Invalid audit event type: {}", request.event_type);
            return Err(StatusCode::BAD_REQUEST);
        }
    };

    match result {
        Ok(event_id) => {
            let response = CreateLogResponse {
                id: event_id,
                message: "Audit event created successfully".to_string(),
            };
            Ok(AxumJson(ApiResponse::success(response)))
        }
        Err(e) => {
            error!("Failed to create audit event: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Force flush all pending logs to storage
pub async fn flush_logs(
    State(state): State<AppState>,
) -> Result<AxumJson<ApiResponse<&'static str>>, StatusCode> {
    let logger = match &state.logging_service {
        Some(logger) => logger,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Flushing logs to storage");

    match logger.flush().await {
        Ok(_) => Ok(AxumJson(ApiResponse::success("Logs flushed successfully"))),
        Err(e) => {
            error!("Failed to flush logs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Search logs by correlation ID
pub async fn get_logs_by_correlation(
    State(state): State<AppState>,
    Path(correlation_id): Path<String>,
) -> Result<AxumJson<ApiResponse<LogResponse<oxide_logging::LogEntry>>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting logs by correlation ID: {}", correlation_id);

    let params = LogQueryParams {
        level: None,
        start_time: None,
        end_time: None,
        correlation_id: Some(correlation_id),
        module: None,
        user_id: None,
        collection: None,
        search: None,
        limit: Some(100),
        offset: None,
        sort: Some("desc".to_string()),
    };

    match service.query_logs(params).await {
        Ok(response) => Ok(AxumJson(ApiResponse::success(response))),
        Err(e) => {
            error!("Failed to query logs by correlation ID: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get logs for a specific user
pub async fn get_user_logs(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Query(mut params): Query<LogQueryParams>,
) -> Result<AxumJson<ApiResponse<LogResponse<oxide_logging::LogEntry>>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting logs for user: {}", user_id);

    // Set the user_id filter
    params.user_id = Some(user_id);

    match service.query_logs(params).await {
        Ok(response) => Ok(AxumJson(ApiResponse::success(response))),
        Err(e) => {
            error!("Failed to query user logs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get logs for a specific collection
pub async fn get_collection_logs(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Query(mut params): Query<LogQueryParams>,
) -> Result<AxumJson<ApiResponse<LogResponse<oxide_logging::LogEntry>>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Getting logs for collection: {}", collection);

    // Set the collection filter
    params.collection = Some(collection);

    match service.query_logs(params).await {
        Ok(response) => Ok(AxumJson(ApiResponse::success(response))),
        Err(e) => {
            error!("Failed to query collection logs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Query parameters for recent logs
#[derive(Debug, Deserialize)]
pub struct RecentLogsParams {
    /// Number of recent logs to return (default: 50, max: 200)
    pub limit: Option<usize>,
}

/// Health check for logging system
pub async fn logging_health(
    State(state): State<AppState>,
) -> Result<AxumJson<ApiResponse<LoggingHealthResponse>>, StatusCode> {
    let service = match &state.logging_api_service {
        Some(service) => service,
        None => return Err(StatusCode::SERVICE_UNAVAILABLE),
    };

    debug!("Checking logging system health");

    match service.get_health_status().await {
        Ok(health) => Ok(AxumJson(ApiResponse::success(health))),
        Err(e) => {
            error!("Failed to get logging health status: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Response for logging health check
#[derive(Debug, Serialize)]
pub struct LoggingHealthResponse {
    /// Health status
    pub status: String,
    /// Total log entries
    pub total_entries: u64,
    /// Storage size in MB
    pub storage_size_mb: u64,
    /// Error rate in last 24 hours
    pub error_rate_24h: f64,
}
