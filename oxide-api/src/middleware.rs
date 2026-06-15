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
    http::{header, HeaderMap, Method, StatusCode, Uri},
    middleware::Next,
    response::Response,
};
use oxide_core::{
    auth::UserRole,
    logging::{ApplicationLogger, LogContext, LogLevel, SecurityAuditor},
    site_settings::{GeneralSettings, MaintenanceSettings, SecuritySettings},
    AuthService, BeforeEventContext, BeforeEventType, Claims,
};
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::{
    errors::{insert_request_id_header, with_request_id, ApiError},
    handlers::auth::{extract_cookie_value, ACCESS_TOKEN_COOKIE, REFRESH_TOKEN_COOKIE},
    server::AppState,
};

/// Extension key for storing claims in request extensions
#[derive(Clone)]
pub struct ClaimsExtension(pub Option<Claims>);

/// Extension key for storing request start time
#[derive(Clone)]
pub struct RequestStartTime(pub Instant);

/// Extension key for storing correlation ID
#[derive(Clone)]
pub struct CorrelationId(pub String);

/// Attach a request ID to the request context and response headers.
pub async fn request_id_middleware(mut request: Request, next: Next) -> Response {
    let request_id =
        request_id_from_headers(request.headers()).unwrap_or_else(|| Uuid::new_v4().to_string());

    request
        .extensions_mut()
        .insert(CorrelationId(request_id.clone()));

    let mut response = with_request_id(request_id.clone(), next.run(request)).await;
    insert_request_id_header(response.headers_mut(), &request_id);
    response
}

fn request_id_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get(crate::errors::REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

const LOCAL_DEVELOPMENT_COOKIE_ORIGINS: &[&str] = &[
    "http://localhost:5173",
    "http://127.0.0.1:5173",
    "http://localhost:3000",
    "http://127.0.0.1:3000",
];

/// Shared fixed-window rate limiter for sensitive public endpoints.
#[derive(Clone)]
pub struct RateLimiter {
    entries: Arc<Mutex<HashMap<String, RateLimitEntry>>>,
}

#[derive(Clone)]
struct RateLimitEntry {
    window_started_at: Instant,
    request_count: u32,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimiter {
    /// Create a new in-memory rate limiter.
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check and record one request against a key.
    pub fn check(&self, key: String, max_requests: u32, window: Duration) -> Result<(), ApiError> {
        let now = Instant::now();
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| ApiError::internal("Rate limiter lock poisoned".to_string()))?;

        if entries.len() > 10_000 {
            entries.retain(|_, entry| now.duration_since(entry.window_started_at) < window);
        }

        let entry = entries.entry(key).or_insert_with(|| RateLimitEntry {
            window_started_at: now,
            request_count: 0,
        });

        if now.duration_since(entry.window_started_at) >= window {
            entry.window_started_at = now;
            entry.request_count = 0;
        }

        if entry.request_count >= max_requests {
            return Err(ApiError::Core(oxide_core::AppError::rate_limit(format!(
                "Too many requests; retry after {} seconds",
                window.as_secs()
            ))));
        }

        entry.request_count = entry.request_count.saturating_add(1);
        Ok(())
    }
}

/// Rate-limit public authentication endpoints before expensive auth work runs.
pub async fn auth_rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let settings = state.runtime_settings.current().await;
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let headers = request.headers().clone();

    if should_apply_general_api_rate_limit(&path) {
        let key = general_api_rate_limit_key(&headers, &state.auth_service);
        state.rate_limiter.check(
            key,
            settings.general.api_rate_limit.max(1),
            Duration::from_secs(60),
        )?;
    }

    if let Some((limit, window)) = auth_endpoint_rate_limit(&method, &path, &settings.security) {
        let client_ip = extract_client_ip(&headers);
        let key = format!(
            "{}:{}:{}",
            method,
            auth_rate_limit_path_key(&path),
            client_ip
        );
        state.rate_limiter.check(key, limit, window)?;
    }

    Ok(next.run(request).await)
}

/// Apply runtime site settings that affect every request.
pub async fn runtime_settings_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let settings = state.runtime_settings.current().await;

    enforce_max_upload_size(&request, &settings.general)?;
    enforce_session_timeout(&state, &request, &settings.security)?;
    enforce_maintenance_mode(&state, &request, &settings.general.maintenance)?;

    Ok(next.run(request).await)
}

/// Reject plain HTTP requests when HTTPS is required by deployment config.
pub async fn require_https_middleware(request: Request, next: Next) -> Result<Response, ApiError> {
    if request_is_https(request.headers()) {
        return Ok(next.run(request).await);
    }

    Err(ApiError::forbidden(
        "HTTPS is required for this deployment".to_string(),
    ))
}

fn enforce_max_upload_size(request: &Request, general: &GeneralSettings) -> Result<(), ApiError> {
    if !request_method_can_have_body(request.method()) {
        return Ok(());
    }

    let Some(content_length) = request.headers().get(header::CONTENT_LENGTH) else {
        return Ok(());
    };

    let content_length = content_length.to_str().map_err(|_| {
        ApiError::bad_request("Content-Length header must be valid ASCII".to_string())
    })?;
    let content_length = content_length.parse::<u64>().map_err(|_| {
        ApiError::bad_request("Content-Length header must be a valid byte count".to_string())
    })?;

    if content_length > general.max_upload_size {
        return Err(ApiError::PayloadTooLarge);
    }

    Ok(())
}

fn enforce_session_timeout(
    state: &AppState,
    request: &Request,
    security: &SecuritySettings,
) -> Result<(), ApiError> {
    let Some(claims) = extract_user_claims(request.headers(), &state.auth_service) else {
        return Ok(());
    };

    let now = current_unix_timestamp();
    let session_timeout_seconds = i64::from(security.session_timeout_minutes) * 60;
    if now.saturating_sub(claims.iat) > session_timeout_seconds {
        return Err(ApiError::auth("Session timed out".to_string()));
    }

    Ok(())
}

fn enforce_maintenance_mode(
    state: &AppState,
    request: &Request,
    maintenance: &MaintenanceSettings,
) -> Result<(), ApiError> {
    if !maintenance.enabled {
        return Ok(());
    }

    let path = request.uri().path();
    if maintenance_exempt_path(path) {
        return Ok(());
    }

    if maintenance.allow_admin_access
        && (maintenance_admin_bootstrap_path(path)
            || request_has_superuser_claims(request.headers(), &state.auth_service))
    {
        return Ok(());
    }

    Err(ApiError::service_unavailable(
        maintenance.message.clone().unwrap_or_else(|| {
            "System maintenance in progress. Please try again later.".to_string()
        }),
    ))
}

fn request_method_can_have_body(method: &Method) -> bool {
    matches!(*method, Method::POST | Method::PUT | Method::PATCH)
}

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
    let correlation_id = request
        .extensions()
        .get::<CorrelationId>()
        .map(|extension| extension.0.clone())
        .or_else(|| request_id_from_headers(&headers))
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Extract user information if available for context
    let user_id =
        extract_user_claims(&headers, &state.auth_service).map(|claims| claims.sub.clone());
    let audit_logging_enabled = state
        .runtime_settings
        .current()
        .await
        .security
        .enable_audit_logging;

    // Store correlation ID and start time in request extensions
    request
        .extensions_mut()
        .insert(CorrelationId(correlation_id.clone()));
    request
        .extensions_mut()
        .insert(RequestStartTime(start_time));

    // Create log context for the request
    let log_context = LogContext::new()
        .with_operation(format!("{} {}", method, path))
        .with_client_ip(client_ip.clone())
        .with_user_agent(user_agent.to_string())
        .with_user_id(user_id.clone().unwrap_or_else(|| "anonymous".to_string()))
        .with_metadata(
            "correlation_id".to_string(),
            serde_json::Value::String(correlation_id.clone()),
        )
        .with_metadata(
            "query_string".to_string(),
            serde_json::Value::String(query.to_string()),
        )
        .with_metadata(
            "content_length".to_string(),
            serde_json::Value::Number(serde_json::Number::from(
                headers
                    .get("content-length")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0),
            )),
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

        if audit_logging_enabled {
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
    }

    // Process the request
    let mut response_result = with_request_id(correlation_id.clone(), next.run(request)).await;
    insert_request_id_header(response_result.headers_mut(), &correlation_id);
    let duration = start_time.elapsed();

    // Extract response information
    let status = response_result.status();
    let status_class = get_status_class(status);

    // Create response log context with timing information
    let response_log_context = log_context
        .with_metadata(
            "response_status".to_string(),
            serde_json::Value::Number(serde_json::Number::from(status.as_u16())),
        )
        .with_metadata(
            "response_time_ms".to_string(),
            serde_json::Value::Number(serde_json::Number::from(duration.as_millis() as u64)),
        )
        .with_metadata(
            "status_class".to_string(),
            serde_json::Value::String(status_class.to_string()),
        );

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

            if audit_logging_enabled {
                if let Err(e) = logging_service
                    .log_security_violation(
                        user_id.clone().unwrap_or_else(|| "anonymous".to_string()),
                        violation_type.to_string(),
                        format!(
                            "API request failed with status {}: {} {}",
                            status, method, path
                        ),
                        response_log_context,
                    )
                    .await
                {
                    error!("Failed to log security violation: {}", e);
                }
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
const PUBLIC_ENDPOINTS: &[&str] = &["/health", "/settings/public"];

/// Public endpoints can either bypass policy entirely or allow anonymous
/// requests while still letting authorization hooks enforce configured policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicEndpointAccess {
    BypassAuthorization,
    AuthorizeAnonymously,
}

/// List of public auth endpoints that don't require authentication but do
/// participate in central authorization policy.
const PUBLIC_AUTH_ENDPOINTS: &[&str] = &[
    "/auth/collections", // List auth collections
    "/auth/validate",    // Token validation
    "/auth/logout",      // Logout (though this doesn't need auth anyway)
    "/auth/refresh",     // Token refresh
];

/// List of public auth endpoint patterns that don't require authentication but
/// do participate in central authorization policy.
const PUBLIC_AUTH_ENDPOINT_PATTERNS: &[&str] = &[
    "/auth/*/login",    // Collection-specific login endpoints
    "/auth/*/register", // Collection-specific register endpoints
];

fn public_endpoint_access(path: &str) -> Option<PublicEndpointAccess> {
    // Check exact matches first
    if PUBLIC_ENDPOINTS.contains(&path) {
        return Some(PublicEndpointAccess::BypassAuthorization);
    }

    if is_public_admin_ui_endpoint(path) {
        return Some(PublicEndpointAccess::BypassAuthorization);
    }

    if PUBLIC_AUTH_ENDPOINTS.contains(&path) {
        return Some(PublicEndpointAccess::AuthorizeAnonymously);
    }

    // Check pattern matches for parameterized auth routes
    for pattern in PUBLIC_AUTH_ENDPOINT_PATTERNS {
        if pattern.contains('*') {
            // Simple pattern matching for /auth/*/login and /auth/*/register
            let pattern_parts: Vec<&str> = pattern.split('/').collect();
            let path_parts: Vec<&str> = path.split('/').collect();

            if pattern_parts.len() == path_parts.len() {
                let mut matches = true;
                for (i, &pattern_part) in pattern_parts.iter().enumerate() {
                    if pattern_part != "*" && pattern_part != path_parts[i] {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    return Some(PublicEndpointAccess::AuthorizeAnonymously);
                }
            }
        } else if path == *pattern {
            return Some(PublicEndpointAccess::AuthorizeAnonymously);
        }
    }

    None
}

fn is_public_admin_ui_endpoint(path: &str) -> bool {
    (path == "/admin" || path.starts_with("/admin/"))
        && !crate::routes::is_registered_protected_admin_api_path(path)
}

fn is_public_api_candidate(path: &str) -> bool {
    path == "/collections"
        || path.starts_with("/collections/")
        || path == "/plugin"
        || path.starts_with("/plugin/")
}

fn maintenance_exempt_path(path: &str) -> bool {
    path == "/health" || path == "/settings/public"
}

fn maintenance_admin_bootstrap_path(path: &str) -> bool {
    is_public_admin_ui_endpoint(path) || path == "/auth/collections" || path.starts_with("/auth/")
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

    if request_uses_cookie_auth(&headers) {
        validate_cookie_auth_request(&method, &headers)?;
    }

    // Extract user claims from headers and store in request extensions
    let claims = extract_user_claims(&headers, &state.auth_service);
    let settings = state.runtime_settings.current().await;

    // Insert claims extension into the request
    request
        .extensions_mut()
        .insert(ClaimsExtension(claims.clone()));

    // Public endpoints may still need central policy checks. Health and static
    // admin UI routes bypass policy, while public auth routes are authorized
    // anonymously so configured auth-operation permissions are honored.
    if let Some(access) = public_endpoint_access(path) {
        debug!("🌐 Public endpoint accessed: {} {}", method, path);
        if access == PublicEndpointAccess::AuthorizeAnonymously {
            authorize_api_request(&state, &method, &uri, path, &headers, &claims).await?;
        }
        return Ok(next.run(request).await);
    }

    if claims.is_none() && is_public_api_candidate(path) && !settings.general.allow_public_api {
        return Err(ApiError::auth(
            "Authentication required; public API access is disabled".to_string(),
        ));
    }

    authorize_api_request(&state, &method, &uri, path, &headers, &claims).await?;

    Ok(next.run(request).await)
}

async fn authorize_api_request(
    state: &AppState,
    method: &Method,
    uri: &Uri,
    path: &str,
    headers: &HeaderMap,
    claims: &Option<Claims>,
) -> Result<(), ApiError> {
    let claims_json = match claims {
        Some(claims) => serde_json::to_value(claims).map_err(|e| {
            ApiError::internal(format!("Failed to serialize authenticated claims: {}", e))
        })?,
        None => serde_json::Value::Null,
    };

    // Convert headers to JSON for event dispatch
    let headers_json = headers_to_json(headers);

    // Create context for BeforeApiRequest event (this will trigger authorization)
    let mut context = BeforeEventContext::new_create(
        "api".to_string(),
        serde_json::json!({
            "method": method.to_string(),
            "path": path,
            "headers": headers_json,
            "claims": claims_json
        }),
    );

    // Dispatch BeforeApiRequest event - this will trigger authorization hooks
    match state
        .event_bus
        .dispatch_before(BeforeEventType::ApiRequest, &mut context)
        .await
    {
        Ok(results) => {
            if let Some(failed_result) = results
                .iter()
                .find(|result| !result.success && !result.skipped)
            {
                let message = failed_result
                    .error
                    .clone()
                    .unwrap_or_else(|| format!("Handler {} failed", failed_result.handler_id));
                warn!(
                    "🚫 Authorization handler {} failed for {} {}: {}",
                    failed_result.handler_id, method, uri, message
                );
                return Err(ApiError::auth(message));
            }

            // Authorization passed, continue with the request
            debug!("✅ Authorization passed for {} {}", method, uri);
            Ok(())
        }
        Err(err) => {
            // Authorization failed
            warn!("🚫 Authorization failed for {} {}: {}", method, uri, err);

            Err(app_error_to_api_error(err))
        }
    }
}

fn app_error_to_api_error(err: oxide_core::AppError) -> ApiError {
    use oxide_core::AppError;

    match &err {
        AppError::Auth { message } => {
            warn!("Authentication error: {}", message);
            ApiError::Core(err)
        }
        AppError::NotFound {
            resource_type,
            identifier,
        } => {
            warn!(
                "Not found error: {} with identifier '{}'",
                resource_type, identifier
            );
            ApiError::Core(err)
        }
        AppError::Validation { field, message } => {
            warn!("Validation error in field '{}': {}", field, message);
            ApiError::Core(err)
        }
        AppError::Conflict { message } => {
            warn!("Conflict error: {}", message);
            ApiError::Core(err)
        }
        AppError::RateLimit { message } => {
            warn!("Rate limit error: {}", message);
            ApiError::Core(err)
        }
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
    }
}

/// Convert HTTP headers to JSON format
fn headers_to_json(headers: &HeaderMap) -> serde_json::Value {
    let mut headers_map = serde_json::Map::new();

    for (name, value) in headers.iter() {
        if let Ok(value_str) = value.to_str() {
            headers_map.insert(
                name.to_string(),
                serde_json::Value::String(value_str.to_string()),
            );
        }
    }

    serde_json::Value::Object(headers_map)
}

/// Extract user claims from authorization header
pub fn extract_user_claims(headers: &HeaderMap, auth_service: &Arc<AuthService>) -> Option<Claims> {
    let bearer_token = headers
        .get("authorization")
        .or_else(|| headers.get("Authorization"))
        .and_then(|h| h.to_str().ok())
        .map(|auth_header| {
            auth_header
                .strip_prefix("Bearer ")
                .unwrap_or(auth_header)
                .to_string()
        });

    let token = bearer_token.or_else(|| extract_cookie_value(headers, ACCESS_TOKEN_COOKIE))?;
    auth_service.verify_token(&token).ok()
}

fn request_has_superuser_claims(headers: &HeaderMap, auth_service: &Arc<AuthService>) -> bool {
    extract_user_claims(headers, auth_service)
        .as_ref()
        .is_some_and(claims_are_superuser)
}

fn claims_are_superuser(claims: &Claims) -> bool {
    matches!(claims.user_role(), Ok(UserRole::Superuser))
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn should_apply_general_api_rate_limit(path: &str) -> bool {
    path != "/health" && !is_public_admin_ui_endpoint(path)
}

fn general_api_rate_limit_key(headers: &HeaderMap, auth_service: &Arc<AuthService>) -> String {
    if let Some(claims) = extract_user_claims(headers, auth_service) {
        return format!("api:user:{}", claims.sub);
    }

    format!("api:ip:{}", extract_client_ip(headers))
}

fn auth_endpoint_rate_limit(
    method: &Method,
    path: &str,
    security: &SecuritySettings,
) -> Option<(u32, Duration)> {
    if *method != Method::POST {
        return None;
    }

    if matches_auth_endpoint(path, "login") {
        let limit = u32::from(security.max_login_attempts.max(1));
        let window = Duration::from_secs(u64::from(security.lockout_duration_minutes.max(1)) * 60);
        return Some((limit, window));
    }

    None
}

fn auth_rate_limit_path_key(path: &str) -> String {
    if matches_auth_endpoint(path, "login") {
        return "/auth/*/login".to_string();
    }

    if matches_auth_endpoint(path, "register") {
        return "/auth/*/register".to_string();
    }

    path.to_string()
}

fn matches_auth_endpoint(path: &str, action: &str) -> bool {
    let mut parts = path.split('/').filter(|part| !part.is_empty());
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some("auth"), Some(_collection), Some(endpoint), None) if endpoint == action
    )
}

fn request_uses_cookie_auth(headers: &HeaderMap) -> bool {
    let has_bearer = headers
        .get(header::AUTHORIZATION)
        .or_else(|| headers.get("Authorization"))
        .and_then(|h| h.to_str().ok())
        .is_some_and(|value| value.starts_with("Bearer "));

    !has_bearer
        && (extract_cookie_value(headers, ACCESS_TOKEN_COOKIE).is_some()
            || extract_cookie_value(headers, REFRESH_TOKEN_COOKIE).is_some())
}

fn validate_cookie_auth_request(method: &Method, headers: &HeaderMap) -> Result<(), ApiError> {
    if matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    ) {
        return Ok(());
    }

    if cookie_request_has_trusted_origin(headers) {
        return Ok(());
    }

    Err(ApiError::forbidden(
        "Cookie-authenticated write requests require a trusted Origin or Referer".to_string(),
    ))
}

fn cookie_request_has_trusted_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = request_origin(headers) else {
        return false;
    };

    trusted_cookie_origins(headers)
        .iter()
        .any(|trusted_origin| trusted_origin == &origin)
}

fn request_origin(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .and_then(normalize_origin)
        .or_else(|| {
            headers
                .get(header::REFERER)
                .and_then(|value| value.to_str().ok())
                .and_then(origin_from_referer)
        })
}

fn trusted_cookie_origins(headers: &HeaderMap) -> Vec<String> {
    let mut origins = Vec::new();

    if let Some(host_origin) = origin_from_host(headers) {
        origins.push(host_origin);
    }

    if request_host_is_loopback(headers) {
        if let Some(local_host_origin) = local_http_origin_from_host(headers) {
            origins.push(local_host_origin);
        }

        origins.extend(
            LOCAL_DEVELOPMENT_COOKIE_ORIGINS
                .iter()
                .filter_map(|origin| normalize_origin(origin)),
        );
    }

    if let Ok(configured) = std::env::var("OXIDEDB_CORS_ALLOWED_ORIGINS") {
        origins.extend(configured.split(',').filter_map(normalize_origin));
    }

    origins.sort();
    origins.dedup();
    origins
}

fn origin_from_host(headers: &HeaderMap) -> Option<String> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())?;
    let scheme = forwarded_proto(headers)
        .or_else(|| forwarded_header_proto(headers))
        .unwrap_or_else(|| default_scheme_for_host(host));
    normalize_origin(&format!("{scheme}://{host}"))
}

fn default_scheme_for_host(host: &str) -> &'static str {
    if host_is_loopback(host) {
        "http"
    } else {
        "https"
    }
}

fn local_http_origin_from_host(headers: &HeaderMap) -> Option<String> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())?;
    if host_is_loopback(host) {
        normalize_origin(&format!("http://{host}"))
    } else {
        None
    }
}

fn origin_from_referer(referer: &str) -> Option<String> {
    let normalized = referer.trim();
    let scheme_end = normalized.find("://")?;
    let after_scheme = scheme_end + 3;
    let path_start = normalized[after_scheme..]
        .find('/')
        .map(|offset| after_scheme + offset)
        .unwrap_or(normalized.len());
    normalize_origin(&normalized[..path_start])
}

fn normalize_origin(origin: &str) -> Option<String> {
    let origin = origin.trim().trim_end_matches('/').to_ascii_lowercase();
    if origin.starts_with("https://") || origin.starts_with("http://") {
        Some(origin)
    } else {
        None
    }
}

fn request_host_is_loopback(headers: &HeaderMap) -> bool {
    headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .is_some_and(host_is_loopback)
}

fn host_is_loopback(host: &str) -> bool {
    let Some(hostname) = hostname_from_host(host) else {
        return false;
    };

    if hostname.eq_ignore_ascii_case("localhost") {
        return true;
    }

    hostname
        .parse::<IpAddr>()
        .is_ok_and(|addr| addr.is_loopback())
}

fn hostname_from_host(host: &str) -> Option<&str> {
    let host = host.trim();
    if host.is_empty() {
        return None;
    }

    if let Some(rest) = host.strip_prefix('[') {
        let (hostname, _) = rest.split_once(']')?;
        return Some(hostname);
    }

    Some(host.split_once(':').map_or(host, |(hostname, _)| hostname))
}

fn request_is_https(headers: &HeaderMap) -> bool {
    forwarded_proto(headers).is_some_and(|proto| proto.eq_ignore_ascii_case("https"))
        || forwarded_header_proto(headers).is_some_and(|proto| proto.eq_ignore_ascii_case("https"))
}

fn forwarded_proto(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn forwarded_header_proto(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("forwarded")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value.split(',').find_map(|element| {
                element.split(';').find_map(|part| {
                    let (key, value) = part.trim().split_once('=')?;
                    if key.trim().eq_ignore_ascii_case("proto") {
                        let proto = value.trim().trim_matches('"');
                        if !proto.is_empty() {
                            return Some(proto);
                        }
                    }
                    None
                })
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::RuntimeSettings;
    use crate::services::{DatabasePermissionService, PluginConfigService};
    use axum::{
        body::Body,
        http::{HeaderName, HeaderValue, Request},
        routing::{get, post},
        Router,
    };
    use oxide_core::{
        auth::AuthServiceConfig, AppError, EventBus, HandlerMetadata, InMemoryEventBus,
    };
    use oxide_db::{Db, SqliteDb};
    use std::{path::PathBuf, sync::Arc, time::Instant};
    use tower::Service;

    fn test_state(event_bus: Arc<dyn EventBus>) -> AppState {
        let auth_service = Arc::new(AuthService::new(AuthServiceConfig::new(
            "test_secret".to_string(),
        )));
        let db: Arc<dyn Db> = Arc::new(
            SqliteDb::new(":memory:", event_bus.clone(), auth_service.clone())
                .expect("test database should initialize"),
        );
        let database_permission_service = Arc::new(DatabasePermissionService::new(Arc::clone(&db)));
        let plugin_config_service = Arc::new(PluginConfigService::new(
            Arc::clone(&db),
            PathBuf::from("target/test-plugins"),
        ));

        AppState {
            db,
            event_bus,
            auth_service,
            logging_service: None,
            logging_api_service: None,
            plugin_manager: None,
            database_permission_service,
            plugin_config_service,
            vfs_service: None,
            rate_limiter: Arc::new(RateLimiter::new()),
            runtime_settings: Arc::new(RuntimeSettings::new(
                oxide_core::site_settings::SiteSettings::default(),
            )),
            started_at: Instant::now(),
        }
    }

    #[test]
    fn forwarded_header_proto_handles_case_quotes_and_proxy_hops() {
        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("forwarded"),
            HeaderValue::from_static(
                r#"for=192.0.2.1;host=api.example.com, for=10.0.0.1;Proto="https""#,
            ),
        );

        assert_eq!(forwarded_header_proto(&headers), Some("https"));
        assert!(request_is_https(&headers));
    }

    #[test]
    fn origin_from_host_uses_forwarded_proto_when_present() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
        headers.insert(
            HeaderName::from_static("forwarded"),
            HeaderValue::from_static("for=192.0.2.1;proto=http"),
        );

        assert_eq!(
            origin_from_host(&headers).as_deref(),
            Some("http://api.example.com")
        );
    }

    #[test]
    fn origin_from_host_defaults_to_http_for_loopback_hosts() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("localhost:8080"));

        assert_eq!(
            origin_from_host(&headers).as_deref(),
            Some("http://localhost:8080")
        );
    }

    #[test]
    fn cookie_auth_write_requires_origin_or_referer() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oxidedb_access_token=token"),
        );

        assert!(request_uses_cookie_auth(&headers));
        assert!(validate_cookie_auth_request(&Method::POST, &headers).is_err());
    }

    #[test]
    fn cookie_auth_write_allows_same_origin_host() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oxidedb_access_token=token"),
        );
        headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://api.example.com"),
        );

        assert!(validate_cookie_auth_request(&Method::POST, &headers).is_ok());
    }

    #[test]
    fn cookie_auth_write_allows_local_dev_origin_for_loopback_api_host() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oxidedb_access_token=token"),
        );
        headers.insert(header::HOST, HeaderValue::from_static("localhost:8080"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://localhost:3000"),
        );

        assert!(validate_cookie_auth_request(&Method::POST, &headers).is_ok());
    }

    #[test]
    fn cookie_auth_write_allows_http_same_origin_for_loopback_api_host() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oxidedb_access_token=token"),
        );
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:8080"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://127.0.0.1:8080"),
        );

        assert!(validate_cookie_auth_request(&Method::POST, &headers).is_ok());
    }

    #[test]
    fn cookie_auth_write_rejects_local_dev_origin_for_remote_api_host() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oxidedb_access_token=token"),
        );
        headers.insert(header::HOST, HeaderValue::from_static("api.example.com"));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("http://localhost:3000"),
        );

        assert!(validate_cookie_auth_request(&Method::POST, &headers).is_err());
    }

    #[test]
    fn auth_public_endpoints_are_policy_aware() {
        assert_eq!(
            public_endpoint_access("/auth/_users/login"),
            Some(PublicEndpointAccess::AuthorizeAnonymously)
        );
        assert_eq!(
            public_endpoint_access("/auth/_users/register"),
            Some(PublicEndpointAccess::AuthorizeAnonymously)
        );
        assert_eq!(
            public_endpoint_access("/auth/refresh"),
            Some(PublicEndpointAccess::AuthorizeAnonymously)
        );
        assert_eq!(
            public_endpoint_access("/health"),
            Some(PublicEndpointAccess::BypassAuthorization)
        );
        assert_eq!(
            public_endpoint_access("/admin"),
            Some(PublicEndpointAccess::BypassAuthorization)
        );
        assert_eq!(
            public_endpoint_access("/admin/assets/app.js"),
            Some(PublicEndpointAccess::BypassAuthorization)
        );
        assert_eq!(public_endpoint_access("/admin/api-keys"), None);
        assert_eq!(public_endpoint_access("/admin/settings/security"), None);
        assert_eq!(public_endpoint_access("/admin/plugin-pages"), None);
        assert_eq!(
            public_endpoint_access("/admin/plugin-pages/assets/hello-plugin/index.html"),
            None
        );
        assert_eq!(
            public_endpoint_access("/admin/plugin-pages/style.css"),
            Some(PublicEndpointAccess::BypassAuthorization)
        );
        assert_eq!(public_endpoint_access("/adminish"), None);
    }

    #[tokio::test]
    async fn public_auth_route_dispatches_authorization_policy() {
        let event_bus = Arc::new(InMemoryEventBus::new());
        event_bus
            .subscribe_before(
                BeforeEventType::ApiRequest.name(),
                Arc::new(|context| {
                    assert_eq!(
                        context.data.get("path").and_then(|value| value.as_str()),
                        Some("/auth/_users/login")
                    );
                    Box::pin(async { Err(AppError::auth("blocked by test policy")) })
                }),
                HandlerMetadata::new(
                    "deny-public-auth".to_string(),
                    "Deny public auth".to_string(),
                ),
            )
            .await
            .expect("test handler should register");

        let state = test_state(event_bus);
        let mut app = Router::new()
            .route("/auth/:collection/login", post(|| async { StatusCode::OK }))
            .route_layer(axum::middleware::from_fn_with_state(state, auth_middleware));

        let response = app
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/auth/_users/login")
                    .body(Body::empty())
                    .expect("test request should build"),
            )
            .await
            .expect("middleware response should be generated");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    async fn failing_handler() -> Result<&'static str, ApiError> {
        Err(ApiError::bad_request("boom"))
    }

    #[tokio::test]
    async fn request_id_middleware_adds_header_and_error_body_request_id() {
        let mut app = Router::new()
            .route("/fail", get(failing_handler))
            .layer(axum::middleware::from_fn(request_id_middleware));

        let response = app
            .call(
                Request::builder()
                    .method(Method::GET)
                    .uri("/fail")
                    .header("x-request-id", "request-123")
                    .body(Body::empty())
                    .expect("test request should build"),
            )
            .await
            .expect("middleware response should be generated");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers().get("x-request-id"),
            Some(&HeaderValue::from_static("request-123"))
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body should be readable");
        let error_body: serde_json::Value =
            serde_json::from_slice(&body).expect("error response should be valid JSON");
        assert_eq!(error_body["request_id"], "request-123");
    }
}
