//! API-specific error handling and HTTP status mapping
//!
//! This module provides error types and utilities for converting internal
//! application errors into appropriate HTTP responses with proper status codes.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use oxide_core::AppError;
use serde::Serialize;
use tracing::error;

/// API-specific error type that wraps core application errors
/// and provides HTTP status code mapping
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Wraps a core application error
    #[error(transparent)]
    Core(#[from] AppError),

    /// Invalid request format or parameters
    #[error("Invalid request: {message}")]
    BadRequest { message: String },

    /// Authentication required
    #[error("Authentication required")]
    Unauthorized,

    /// Access forbidden
    #[error("Access forbidden: {message}")]
    Forbidden { message: String },

    /// Resource not found
    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    /// Request conflicts with current state
    #[error("Conflict: {message}")]
    Conflict { message: String },

    /// Request payload too large
    #[error("Request payload too large")]
    PayloadTooLarge,

    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    TooManyRequests,

    /// Internal server error
    #[error("Internal server error: {message}")]
    Internal { message: String },

    /// Service temporarily unavailable
    #[error("Service unavailable: {message}")]
    ServiceUnavailable { message: String },
}

/// Standardized error response format
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Error type identifier
    pub error: String,
    /// Human-readable error message
    pub message: String,
    /// Optional additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

impl ApiError {
    /// Create a bad request error
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest {
            message: message.into(),
        }
    }

    /// Create an authentication error
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Core(AppError::Auth {
            message: message.into(),
        })
    }

    /// Create a forbidden error
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::Forbidden {
            message: message.into(),
        }
    }

    /// Create a not found error
    pub fn not_found(resource: impl Into<String>) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Create a conflict error
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict {
            message: message.into(),
        }
    }

    /// Create an internal server error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create a service unavailable error
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            message: message.into(),
        }
    }

    /// Get the appropriate HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Core(app_error) => match app_error {
                AppError::NotFound { .. } => StatusCode::NOT_FOUND,
                AppError::Validation { .. } => StatusCode::BAD_REQUEST,
                AppError::Auth { .. } => StatusCode::UNAUTHORIZED,
                AppError::Database { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AppError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AppError::Plugin { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AppError::Io { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AppError::Config { .. } => StatusCode::INTERNAL_SERVER_ERROR,
                AppError::Conflict { .. } => StatusCode::CONFLICT,
                AppError::RateLimit { .. } => StatusCode::TOO_MANY_REQUESTS,
                AppError::Security { .. } => StatusCode::FORBIDDEN,
                AppError::VirtualFileSystem { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            },
            ApiError::BadRequest { .. } => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden { .. } => StatusCode::FORBIDDEN,
            ApiError::NotFound { .. } => StatusCode::NOT_FOUND,
            ApiError::Conflict { .. } => StatusCode::CONFLICT,
            ApiError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ServiceUnavailable { .. } => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Get the error type identifier
    pub fn error_type(&self) -> &'static str {
        match self {
            ApiError::Core(app_error) => match app_error {
                AppError::NotFound { .. } => "not_found",
                AppError::Validation { .. } => "validation_error",
                AppError::Auth { .. } => "authentication_error",
                AppError::Database { .. } => "database_error",
                AppError::Internal { .. } => "internal_error",
                AppError::Plugin { .. } => "plugin_error",
                AppError::Io { .. } => "io_error",
                AppError::Config { .. } => "config_error",
                AppError::Conflict { .. } => "conflict_error",
                AppError::RateLimit { .. } => "rate_limit_error",
                AppError::Security { .. } => "security_error",
                AppError::VirtualFileSystem { .. } => "virtual_file_system_error",
            },
            ApiError::BadRequest { .. } => "bad_request",
            ApiError::Unauthorized => "unauthorized",
            ApiError::Forbidden { .. } => "forbidden",
            ApiError::NotFound { .. } => "not_found",
            ApiError::Conflict { .. } => "conflict",
            ApiError::PayloadTooLarge => "payload_too_large",
            ApiError::TooManyRequests => "too_many_requests",
            ApiError::Internal { .. } => "internal_error",
            ApiError::ServiceUnavailable { .. } => "service_unavailable",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let error_type = self.error_type();
        let message = self.to_string();

        // Log internal errors
        if status.is_server_error() {
            error!("API Error: {} - {}", error_type, message);
        }

        let error_response = ErrorResponse {
            error: error_type.to_string(),
            message,
            details: None,
            request_id: None, // TODO: Extract from request context
        };

        (status, Json(error_response)).into_response()
    }
}

/// Convert a tuple of (StatusCode, String) to ApiError for backward compatibility
impl From<(StatusCode, String)> for ApiError {
    fn from((status, message): (StatusCode, String)) -> Self {
        match status {
            StatusCode::BAD_REQUEST => ApiError::bad_request(message),
            StatusCode::UNAUTHORIZED => ApiError::Unauthorized,
            StatusCode::FORBIDDEN => ApiError::forbidden(message),
            StatusCode::NOT_FOUND => ApiError::not_found(message),
            StatusCode::CONFLICT => ApiError::conflict(message),
            StatusCode::PAYLOAD_TOO_LARGE => ApiError::PayloadTooLarge,
            StatusCode::TOO_MANY_REQUESTS => ApiError::TooManyRequests,
            StatusCode::SERVICE_UNAVAILABLE => ApiError::service_unavailable(message),
            _ => ApiError::internal(message),
        }
    }
}
