//! Health check and system status handlers
//!
//! This module provides endpoints for monitoring the health and status
//! of the OxideDB API server and its dependencies.

use axum::{extract::State, response::Json};
use oxide_db::Db;
use serde::Serialize;
use std::sync::Arc;
use tracing::{debug, warn};
use ts_rs::TS;

use crate::{errors::ApiError, responses::ApiResponse, server::AppState};

/// Health status response with comprehensive version information
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct HealthStatus {
    /// Overall system status
    pub status: String,
    /// Database connection status
    pub database: String,
    /// API version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// System uptime in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime: Option<u64>,
    /// Component version information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<ComponentVersions>,
}

/// Version information for all system components
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ComponentVersions {
    /// API server version
    pub api: String,
    /// Database layer version
    pub database: String,
    /// VFS version
    pub vfs: String,
    /// Plugin runtime version
    pub plugin_runtime: String,
}

/// Handlers for health-related operations
pub struct HealthHandlers;

impl HealthHandlers {
    /// Perform a comprehensive health check
    ///
    /// This checks the database connection and other critical system components
    /// to determine if the API is ready to serve requests.
    pub async fn health_check(
        db: Arc<dyn Db>,
        uptime: Option<u64>,
    ) -> Result<HealthStatus, ApiError> {
        debug!("Performing health check");

        // Check database connectivity
        let database_status = match Self::check_database_health(&db).await {
            Ok(_) => "healthy".to_string(),
            Err(e) => {
                warn!("Database health check failed: {}", e);
                format!("unhealthy: {}", e)
            }
        };

        let overall_status = if database_status.starts_with("healthy") {
            "healthy"
        } else {
            "unhealthy"
        };

        // Collect version information from all system components
        let component_versions = ComponentVersions {
            api: env!("CARGO_PKG_VERSION").to_string(),
            database: oxide_db::VERSION.to_string(),
            vfs: oxide_vfs::VERSION.to_string(),
            plugin_runtime: oxide_plugin_runtime::VERSION.to_string(),
        };

        let health_status = HealthStatus {
            status: overall_status.to_string(),
            database: database_status,
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            uptime,
            versions: Some(component_versions),
        };

        debug!("Health check completed: {}", overall_status);
        Ok(health_status)
    }

    /// Check database health by performing a simple query
    async fn check_database_health(db: &Arc<dyn Db>) -> Result<(), ApiError> {
        // Try to list collections as a basic connectivity test
        db.list_collections()
            .await
            .map_err(|e| ApiError::service_unavailable(format!("Database check failed: {}", e)))?;

        Ok(())
    }
}

/// HTTP handler for health check endpoint
///
/// GET /health
pub async fn health_check(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<HealthStatus>>, ApiError> {
    let uptime = Some(state.started_at.elapsed().as_secs());
    let health_status = HealthHandlers::health_check(state.db, uptime).await?;

    // Return 503 if unhealthy
    if health_status.status != "healthy" {
        return Err(ApiError::service_unavailable("System is unhealthy"));
    }

    Ok(Json(ApiResponse::success(health_status)))
}
