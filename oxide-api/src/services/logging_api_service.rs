//! Logging API service wrapper for oxide-api
//!
//! This service provides a bridge between the oxide-api HTTP layer
//! and the oxide-logging API service.

use oxide_logging::api::LogApiService;
use crate::handlers::logs::LoggingHealthResponse;
use oxide_logging::error::LoggingResult;

/// Wrapper service for logging API functionality
pub struct LoggingApiService {
    /// The underlying oxide-logging API service
    inner: LogApiService,
}

impl LoggingApiService {
    /// Create a new logging API service
    pub fn new(log_api_service: LogApiService) -> Self {
        Self {
            inner: log_api_service,
        }
    }

    /// Get the inner API service
    pub fn inner(&self) -> &LogApiService {
        &self.inner
    }

    /// Get health status of the logging system
    pub async fn get_health_status(&self) -> LoggingResult<LoggingHealthResponse> {
        // Get basic metrics to verify the system is working
        match self.inner.get_dashboard_metrics().await {
            Ok(metrics) => {
                let health = LoggingHealthResponse {
                    status: "healthy".to_string(),
                    total_entries: metrics.log_metrics.total_entries,
                    storage_size_mb: metrics.log_metrics.storage_size_bytes / (1024 * 1024),
                    error_rate_24h: metrics.log_metrics.error_rate_24h,
                };
                Ok(health)
            }
            Err(_) => {
                let health = LoggingHealthResponse {
                    status: "unhealthy".to_string(),
                    total_entries: 0,
                    storage_size_mb: 0,
                    error_rate_24h: 100.0,
                };
                Ok(health)
            }
        }
    }
}

// Delegate all methods to the inner service
impl std::ops::Deref for LoggingApiService {
    type Target = LogApiService;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
} 