//! Standardized API response types and utilities
//!
//! This module provides consistent response formats for the OxideDB API,
//! including success responses, pagination, and metadata.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
use ts_rs::TS;

/// Standard API response wrapper
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ApiResponse<T> {
    /// Response data
    pub data: T,
    /// Response metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ResponseMeta>,
    /// Success indicator
    pub success: bool,
}

/// Response metadata for additional context
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct ResponseMeta {
    /// Request processing time in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// API version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Request ID for tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Paginated response wrapper
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct PaginatedResponse<T> {
    /// Response data items
    pub data: Vec<T>,
    /// Pagination information
    pub pagination: PaginationInfo,
    /// Response metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<ResponseMeta>,
    /// Success indicator
    pub success: bool,
}

/// Pagination information
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct PaginationInfo {
    /// Current page number (1-based)
    pub page: u32,
    /// Number of items per page
    pub per_page: u32,
    /// Total number of items
    pub total: u64,
    /// Total number of pages
    pub total_pages: u32,
    /// Whether there is a next page
    pub has_next: bool,
    /// Whether there is a previous page
    pub has_prev: bool,
}

/// Empty response for operations that don't return data
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct EmptyResponse {
    /// Success message
    pub message: String,
}

impl<T> ApiResponse<T>
where
    T: Serialize,
{
    /// Create a successful response
    pub fn success(data: T) -> Self {
        Self {
            data,
            meta: None,
            success: true,
        }
    }
    
    /// Create a successful response with metadata
    pub fn success_with_meta(data: T, meta: ResponseMeta) -> Self {
        Self {
            data,
            meta: Some(meta),
            success: true,
        }
    }
}

impl<T> PaginatedResponse<T>
where
    T: Serialize,
{
    /// Create a paginated response
    pub fn new(
        data: Vec<T>,
        page: u32,
        per_page: u32,
        total: u64,
    ) -> Self {
        let total_pages = ((total as f64) / (per_page as f64)).ceil() as u32;
        let has_next = page < total_pages;
        let has_prev = page > 1;
        
        Self {
            data,
            pagination: PaginationInfo {
                page,
                per_page,
                total,
                total_pages,
                has_next,
                has_prev,
            },
            meta: None,
            success: true,
        }
    }
    
    /// Create a paginated response with metadata
    pub fn with_meta(
        data: Vec<T>,
        page: u32,
        per_page: u32,
        total: u64,
        meta: ResponseMeta,
    ) -> Self {
        let mut response = Self::new(data, page, per_page, total);
        response.meta = Some(meta);
        response
    }
}

impl EmptyResponse {
    /// Create an empty success response
    pub fn success(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
    
    /// Create a standard "created" response
    pub fn created() -> Self {
        Self {
            message: "Resource created successfully".to_string(),
        }
    }
    
    /// Create a standard "updated" response
    pub fn updated() -> Self {
        Self {
            message: "Resource updated successfully".to_string(),
        }
    }
    
    /// Create a standard "deleted" response
    pub fn deleted() -> Self {
        Self {
            message: "Resource deleted successfully".to_string(),
        }
    }
}

impl ResponseMeta {
    /// Create new response metadata
    pub fn new() -> Self {
        Self {
            duration_ms: None,
            version: None,
            request_id: None,
        }
    }
    
    /// Set the processing duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = Some(duration_ms);
        self
    }
    
    /// Set the API version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
    
    /// Set the request ID
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

impl Default for ResponseMeta {
    fn default() -> Self {
        Self::new()
    }
}

// Implement IntoResponse for our response types
impl<T> IntoResponse for ApiResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

impl<T> IntoResponse for PaginatedResponse<T>
where
    T: Serialize,
{
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

impl IntoResponse for EmptyResponse {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
} 