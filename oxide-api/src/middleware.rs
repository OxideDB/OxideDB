//! HTTP middleware for the API server
//!
//! This module contains middleware functions that process HTTP requests
//! and responses. Middleware handles cross-cutting concerns like logging,
//! authentication, CORS, rate limiting, etc.
//!
//! ## Available Middleware
//!
//! - [`LoggingMiddleware`] - Request/response logging with event dispatch
//! - [`AuthMiddleware`] - Authentication and authorization middleware
//! - [`RequestIdMiddleware`] - Request ID generation and tracking
//! - [`TimingMiddleware`] - Request timing and performance metrics

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use oxide_core::{BeforeEventContext, BeforeEventType, EventBus, AuthService, Claims};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::{errors::ApiError, server::AppState};

/// Extension key for storing claims in request extensions
#[derive(Clone)]
pub struct ClaimsExtension(pub Option<Claims>);

/// Logging middleware that dispatches API events
pub struct LoggingMiddleware {
    event_bus: Arc<dyn EventBus>,
}

impl LoggingMiddleware {
    /// Create a new logging middleware instance
    pub fn new(event_bus: Arc<dyn EventBus>) -> Self {
        Self { event_bus }
    }

    /// Process a request and log it
    pub async fn process_request(
        &self,
        method: String,
        path: String,
        headers: serde_json::Value,
    ) -> Result<(), ApiError> {
        debug!("Processing {} {}", method, path);

        // Create context for BeforeApiRequest event
        let mut context = BeforeEventContext {
            collection: "api".to_string(),
            data: serde_json::json!({
                "method": method,
                "path": path,
                "headers": headers
            }),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };

        // Dispatch BeforeApiRequest event
        self.event_bus
            .dispatch_before(BeforeEventType::ApiRequest, &mut context)
            .await?;

        info!("Incoming request: {} {}", 
            context.data.get("method").and_then(|m| m.as_str()).unwrap_or("UNKNOWN"),
            context.data.get("path").and_then(|p| p.as_str()).unwrap_or("/")
        );
        Ok(())
    }
}

/// List of public endpoints that don't require authentication
const PUBLIC_ENDPOINTS: &[&str] = &[
    "/health",
];

/// List of public endpoint prefixes that don't require authentication
const PUBLIC_ENDPOINT_PREFIXES: &[&str] = &[
    "/auth/",      // All auth endpoints should be public
    "/admin",      // Admin UI endpoints should be publicly accessible
];

/// Check if the given path is a public endpoint
fn is_public_endpoint(path: &str) -> bool {
    // Check exact matches first
    if PUBLIC_ENDPOINTS.iter().any(|&endpoint| path == endpoint) {
        return true;
    }
    
    // Check prefix matches for parameterized routes
    PUBLIC_ENDPOINT_PREFIXES.iter().any(|&prefix| path.starts_with(prefix))
}

/// Authentication and authorization middleware
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {

    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let path = uri.path();

    debug!("🔐 Processing auth middleware for {} {}", method, uri);

    // Check if this is a public endpoint that doesn't require authentication
    if is_public_endpoint(path) {
        debug!("🌐 Public endpoint accessed: {} {}", method, path);
        // Still extract claims if present (for optional authentication)
        let claims = extract_user_claims(&headers, &state.auth_service);
        request.extensions_mut().insert(ClaimsExtension(claims));
        return Ok(next.run(request).await);
    }

    // Extract user claims from headers and store in request extensions
    let claims = extract_user_claims(&headers, &state.auth_service);
    
    // Insert claims extension into the request
    request.extensions_mut().insert(ClaimsExtension(claims.clone()));

    // Convert headers to JSON for event dispatch
    let headers_json = headers_to_json(&headers);

    // Create context for BeforeApiRequest event (this will trigger authorization)
    let mut context = BeforeEventContext {
        collection: "api".to_string(),
        data: serde_json::json!({
            "method": method.to_string(),
            "path": path,
            "headers": headers_json
        }),
        metadata: serde_json::json!({}),
        record_id: None,
        old_data: None,
    };

    // Dispatch BeforeApiRequest event - this will trigger authorization hooks
    match state.event_bus.dispatch_before(BeforeEventType::ApiRequest, &mut context).await {
        Ok(()) => {
            // Authorization passed, continue with the request
            debug!("✅ Authorization passed for {} {}", method, uri);
            Ok(next.run(request).await)
        }
        Err(err) => {
            // Authorization failed
            warn!("🚫 Authorization failed for {} {}: {}", method, uri, err);
            
            // Convert AppError to appropriate ApiError with detailed information
            use oxide_core::AppError;
            let api_error = match &err {
                AppError::Auth { message } => {
                    warn!("Authentication error: {}", message);
                    ApiError::Core(err)
                },
                AppError::NotFound { resource_type, identifier } => {
                     warn!("Not found error: {} with identifier '{}'", resource_type, identifier);
                     ApiError::Core(err)
                 },
                AppError::Validation { field, message } => {
                     warn!("Validation error in field '{}': {}", field, message);
                     ApiError::Core(err)
                 },
                AppError::Conflict { message } => {
                    warn!("Conflict error: {}", message);
                    ApiError::Core(err)
                },
                AppError::RateLimit { message } => {
                    warn!("Rate limit error: {}", message);
                    ApiError::Core(err)
                },
                _ => {
                    // For other errors, log detailed information and check message for backward compatibility
                    warn!("Internal error in auth middleware: {:?}", err);
                    let err_msg = err.to_string().to_lowercase();
                    if err_msg.contains("permission") || err_msg.contains("forbidden") {
                        ApiError::forbidden(err.to_string())
                    } else {
                        ApiError::Core(err)
                    }
                }
            };
            
            Err(api_error)
        }
    }
}

/// Convert HTTP headers to JSON format
fn headers_to_json(headers: &HeaderMap) -> serde_json::Value {
    let mut headers_map = serde_json::Map::new();
    
    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            headers_map.insert(name.to_string(), serde_json::Value::String(value_str.to_string()));
        }
    }
    
    serde_json::Value::Object(headers_map)
}

/// Extract user claims from authorization header
pub fn extract_user_claims(headers: &HeaderMap, auth_service: &Arc<AuthService>) -> Option<Claims> {
    let auth_header = headers.get("authorization")
        .or_else(|| headers.get("Authorization"))
        .and_then(|h| h.to_str().ok())?;

    let token = if let Some(stripped) = auth_header.strip_prefix("Bearer ") {
        stripped
    } else {
        auth_header
    };

    auth_service.verify_token(token).ok()
}
