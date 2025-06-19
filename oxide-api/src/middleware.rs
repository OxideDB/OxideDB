//! HTTP middleware for the API server
//!
//! This module contains middleware functions that process HTTP requests
//! and responses. Middleware handles cross-cutting concerns like logging,
//! authentication, CORS, rate limiting, etc.
//!
//! ## Available Middleware
//!
//! - [`request_logging_middleware`] - Comprehensive API access logging with event dispatch
//! - [`AuthMiddleware`] - Authentication and authorization middleware
//! - [`RequestIdMiddleware`] - Request ID generation and tracking
//! - [`TimingMiddleware`] - Request timing and performance metrics

use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use oxide_core::{
    BeforeEventContext, BeforeEventType, AuthService, Claims,
    logging::{LogContext, LogLevel, ApplicationLogger, SecurityAuditor},
};
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn, error};
use uuid::Uuid;

use crate::{errors::ApiError, server::AppState};

/// Extension key for storing claims in request extensions
#[derive(Clone)]
pub struct ClaimsExtension(pub Option<Claims>);

/// Extension key for storing request start time
#[derive(Clone)]
pub struct RequestStartTime(pub Instant);

/// Extension key for storing correlation ID
#[derive(Clone)]
pub struct CorrelationId(pub String);

/// Comprehensive request logging middleware that logs API access
///
/// This middleware:
/// - Logs incoming requests with details
/// - Tracks request duration
/// - Logs response status and timing
/// - Integrates with the logging service for audit trails
/// - Generates correlation IDs for request tracking
pub async fn request_logging_middleware(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let start_time = Instant::now();
    let correlation_id = Uuid::new_v4().to_string();
    
    // Extract request information
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path();
    let query = uri.query().unwrap_or("");
    let headers = request.headers().clone();
    let user_agent = headers
        .get("user-agent")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");
    let client_ip = extract_client_ip(&headers);
    
    // Extract user information if available for context
    let user_id = extract_user_claims(&headers, &state.auth_service)
        .map(|claims| claims.sub.clone());

    // Store correlation ID and start time in request extensions
    request.extensions_mut().insert(CorrelationId(correlation_id.clone()));
    request.extensions_mut().insert(RequestStartTime(start_time));

    // Create log context for the request
    let log_context = LogContext::new()
        .with_operation(format!("{} {}", method, path))
        .with_client_ip(client_ip.clone())
        .with_user_agent(user_agent.to_string())
        .with_user_id(user_id.clone().unwrap_or_else(|| "anonymous".to_string()))
        .with_metadata("correlation_id".to_string(), serde_json::Value::String(correlation_id.clone()))
        .with_metadata("query_string".to_string(), serde_json::Value::String(query.to_string()))
        .with_metadata("content_length".to_string(), 
            serde_json::Value::Number(
                serde_json::Number::from(
                    headers.get("content-length")
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0)
                )
            )
        );

    // Log the incoming request using the logging service if available
    if let Some(logging_service) = &state.logging_service {
        let message = format!(
            "API Request: {} {} - User: {} - IP: {} - Agent: {}",
            method,
            path,
            user_id.clone().unwrap_or_else(|| "anonymous".to_string()),
            client_ip,
            user_agent
        );

        // Log application-level request
        if let Err(e) = logging_service
            .log_with_context(
                LogLevel::Info,
                message,
                "api-access".to_string(),
                log_context.clone(),
            )
            .await
        {
            error!("Failed to log API request: {}", e);
        }

        // Log security audit event for data access
        if let Err(e) = logging_service
            .log_data_access(
                user_id.clone().unwrap_or_else(|| "anonymous".to_string()),
                format!("API:{}", path),
                method.to_string(),
                log_context.clone(),
            )
            .await
        {
            error!("Failed to log security audit event: {}", e);
        }
    }

    // Process the request
    let response_result = next.run(request).await;
    let duration = start_time.elapsed();

    // Extract response information
    let status = response_result.status();
    let status_class = get_status_class(status);

    // Create response log context with timing information
    let response_log_context = log_context
        .with_metadata("response_status".to_string(), serde_json::Value::Number(serde_json::Number::from(status.as_u16())))
        .with_metadata("response_time_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(duration.as_millis() as u64)))
        .with_metadata("status_class".to_string(), serde_json::Value::String(status_class.to_string()));

    // Log the response using the logging service if available
    if let Some(logging_service) = &state.logging_service {
        let log_level = match status.as_u16() {
            200..=299 => LogLevel::Info,
            300..=399 => LogLevel::Info,
            400..=499 => LogLevel::Warn,
            500..=599 => LogLevel::Error,
            _ => LogLevel::Debug,
        };

                 let message = format!(
             "API Response: {} {} -> {} in {:.2}ms - User: {} - IP: {}",
             method,
             path,
             status,
             duration.as_millis(),
             user_id.clone().unwrap_or_else(|| "anonymous".to_string()),
             client_ip
         );

        // Log application-level response
        if let Err(e) = logging_service
            .log_with_context(
                log_level,
                message,
                "api-access".to_string(),
                response_log_context.clone(),
            )
            .await
        {
            error!("Failed to log API response: {}", e);
        }

        // Log security events for failures
        if status.is_client_error() || status.is_server_error() {
            let violation_type = if status.is_client_error() {
                "client_error"
            } else {
                "server_error"
            };

                         if let Err(e) = logging_service
                 .log_security_violation(
                     user_id.clone().unwrap_or_else(|| "anonymous".to_string()),
                     violation_type.to_string(),
                     format!("API request failed with status {}: {} {}", status, method, path),
                     response_log_context,
                 )
                 .await
            {
                error!("Failed to log security violation: {}", e);
            }
        }
    }

    // Log to application tracing as well for immediate visibility
    match status.as_u16() {
        200..=299 => info!(
            correlation_id = %correlation_id,
            method = %method,
            path = %path,
            status = %status,
            duration_ms = %duration.as_millis(),
            user_id = %user_id.unwrap_or_else(|| "anonymous".to_string()),
            client_ip = %client_ip,
            "API request completed successfully"
        ),
        300..=399 => info!(
            correlation_id = %correlation_id,
            method = %method,
            path = %path,
            status = %status,
            duration_ms = %duration.as_millis(),
            user_id = %user_id.unwrap_or_else(|| "anonymous".to_string()),
            client_ip = %client_ip,
            "API request redirected"
        ),
        400..=499 => warn!(
            correlation_id = %correlation_id,
            method = %method,
            path = %path,
            status = %status,
            duration_ms = %duration.as_millis(),
            user_id = %user_id.unwrap_or_else(|| "anonymous".to_string()),
            client_ip = %client_ip,
            "API request failed with client error"
        ),
        500..=599 => error!(
            correlation_id = %correlation_id,
            method = %method,
            path = %path,
            status = %status,
            duration_ms = %duration.as_millis(),
            user_id = %user_id.unwrap_or_else(|| "anonymous".to_string()),
            client_ip = %client_ip,
            "API request failed with server error"
        ),
        _ => debug!(
            correlation_id = %correlation_id,
            method = %method,
            path = %path,
            status = %status,
            duration_ms = %duration.as_millis(),
            user_id = %user_id.unwrap_or_else(|| "anonymous".to_string()),
            client_ip = %client_ip,
            "API request completed with unusual status"
        ),
    }

    Ok(response_result)
}

/// Extract client IP from headers
fn extract_client_ip(headers: &HeaderMap) -> String {
    // Try X-Forwarded-For first (most common proxy header)
    if let Some(forwarded_for) = headers.get("x-forwarded-for") {
        if let Ok(value) = forwarded_for.to_str() {
            // X-Forwarded-For can contain multiple IPs, take the first one
            if let Some(first_ip) = value.split(',').next() {
                return first_ip.trim().to_string();
            }
        }
    }

    // Try X-Real-IP (Nginx)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(value) = real_ip.to_str() {
            return value.to_string();
        }
    }

    // Try CF-Connecting-IP (Cloudflare)
    if let Some(cf_ip) = headers.get("cf-connecting-ip") {
        if let Ok(value) = cf_ip.to_str() {
            return value.to_string();
        }
    }

    // Try X-Forwarded-Proto (some load balancers)
    if let Some(client_ip) = headers.get("x-client-ip") {
        if let Ok(value) = client_ip.to_str() {
            return value.to_string();
        }
    }

    // Fallback to "unknown" if no IP headers found
    "unknown".to_string()
}

/// Get status class for categorization
fn get_status_class(status: StatusCode) -> &'static str {
    match status.as_u16() {
        200..=299 => "success",
        300..=399 => "redirect",
        400..=499 => "client_error",
        500..=599 => "server_error",
        _ => "unknown",
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
    let mut context = BeforeEventContext::new_create(
        "api".to_string(),
        serde_json::json!({
            "method": method.to_string(),
            "path": path,
            "headers": headers_json
        }),
    );

    // Dispatch BeforeApiRequest event - this will trigger authorization hooks
    match state.event_bus.dispatch_before(BeforeEventType::ApiRequest, &mut context).await {
        Ok(_results) => {
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
