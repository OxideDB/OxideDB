//! Authentication handlers
//!
//! This module provides HTTP handlers for user authentication including
//! login, logout, registration, and token management using the collection-based
//! authentication system.

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    Json,
};
use oxide_core::{auth::RefreshClaims, Claims};
use oxide_db::{
    db::{AuthRequest, RefreshTokenRecord, RegisterRequest as DbRegisterRequest},
    Db,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};

/// Login request payload for collection-specific authentication
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub credential: String,
    #[serde(default)]
    pub cookie_session: bool,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub auth_collection: String,
    pub expires_in: i64,
    pub refresh_expires_in: Option<i64>,
    pub custom_claims: Option<serde_json::Value>,
}

/// Registration request payload for collection-specific authentication
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub identifier: String,
    pub credential: String,
    pub additional_data: Option<serde_json::Value>,
}

/// Registration response
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user_id: String,
    pub identifier: String,
    pub role: String,
    pub auth_collection: String,
    pub message: String,
}

/// Token validation request payload
#[derive(Debug, Deserialize)]
pub struct TokenValidationRequest {
    pub token: String,
}

/// Token validation response
#[derive(Debug, Serialize)]
pub struct TokenValidationResponse {
    pub valid: bool,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub auth_collection: Option<String>,
    pub expires_at: Option<i64>,
    pub custom_claims: Option<serde_json::Value>,
}

/// Current user response
#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub auth_collection: String,
    pub expires_at: i64,
    pub custom_claims: Option<serde_json::Value>,
}

/// Auth collections list response
#[derive(Debug, Serialize)]
pub struct AuthCollectionsResponse {
    pub collections: Vec<AuthCollectionInfo>,
}

/// Auth collection information
#[derive(Debug, Serialize)]
pub struct AuthCollectionInfo {
    pub name: String,
    pub identifier_field: String,
    pub registration_enabled: bool,
    pub email_verification_required: bool,
}

/// Refresh token request payload
#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub cookie_session: bool,
}

/// Logout request payload
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: Option<String>,
}

/// Refresh token response
#[derive(Debug, Serialize)]
pub struct RefreshTokenResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub expires_in: i64,
    pub refresh_expires_in: i64,
}

pub(crate) const ACCESS_TOKEN_COOKIE: &str = "oxidedb_access_token";
pub(crate) const REFRESH_TOKEN_COOKIE: &str = "oxidedb_refresh_token";

/// Handlers for authentication operations
pub struct AuthHandlers;

impl AuthHandlers {
    /// Authenticate a user against a specific auth collection
    pub async fn login(
        db: Arc<dyn Db>,
        auth_service: Arc<oxide_core::AuthService>,
        collection: String,
        identifier: String,
        credential: String,
    ) -> Result<LoginResponse, ApiError> {
        debug!(
            "🔑 Processing login request for collection '{}': {}",
            collection, identifier
        );

        // Get auth configuration for the collection
        let auth_config = auth_service
            .config()
            .get_auth_collection(&collection)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "Collection '{}' is not configured for authentication",
                    collection
                ))
            })?;

        // Create auth request
        let auth_request = AuthRequest {
            collection: collection.clone(),
            identifier: identifier.clone(),
            credential,
        };

        // Authenticate user
        let auth_response = db
            .authenticate_user(auth_request, &auth_config)
            .await
            .map_err(|e| {
                warn!(
                    "Authentication failed for user: {} in collection '{}' - {}",
                    identifier, collection, e
                );
                ApiError::auth("Invalid credentials".to_string())
            })?;

        if let Some(refresh_token) = &auth_response.refresh_token {
            persist_refresh_token(Arc::clone(&db), Arc::clone(&auth_service), refresh_token)
                .await?;
        }

        let response = LoginResponse {
            token: Some(auth_response.token),
            refresh_token: auth_response.refresh_token.clone(),
            user_id: auth_response.user_id,
            email: identifier.clone(), // For now, using identifier as email
            role: auth_response.role,
            auth_collection: auth_response.auth_collection,
            expires_in: auth_service.config().token_expiry_hours * 3600,
            refresh_expires_in: if auth_response.refresh_token.is_some() {
                Some(auth_service.config().refresh_token_expiry_days * 24 * 3600)
            } else {
                None
            },
            custom_claims: if auth_response
                .user_data
                .as_object()
                .is_some_and(|obj| !obj.is_empty())
            {
                Some(auth_response.user_data)
            } else {
                None
            },
        };

        info!(
            "✅ User login successful: {} from collection '{}' ({})",
            identifier, collection, response.role
        );
        Ok(response)
    }

    /// Register a new user in a specific auth collection
    pub async fn register(
        db: Arc<dyn Db>,
        auth_service: Arc<oxide_core::AuthService>,
        collection: String,
        identifier: String,
        credential: String,
        additional_data: Option<serde_json::Value>,
    ) -> Result<RegisterResponse, ApiError> {
        debug!(
            "👤 Processing registration for collection '{}': {}",
            collection, identifier
        );

        // Get auth configuration for the collection
        let auth_config = auth_service
            .config()
            .get_auth_collection(&collection)
            .ok_or_else(|| {
                ApiError::bad_request(format!(
                    "Collection '{}' is not configured for authentication",
                    collection
                ))
            })?;

        // Validate identifier format (basic validation)
        if identifier.is_empty() || identifier.len() < 3 {
            return Err(ApiError::bad_request(
                "Identifier must be at least 3 characters long".to_string(),
            ));
        }

        // Validate credential strength (basic validation)
        if credential.len() < 8 {
            return Err(ApiError::bad_request(
                "Credential must be at least 8 characters long".to_string(),
            ));
        }

        // Create registration request
        let register_request = DbRegisterRequest {
            collection: collection.clone(),
            identifier: identifier.clone(),
            credential,
            additional_data,
        };

        // Register user
        let user_id = db
            .register_user(register_request, &auth_config)
            .await
            .map_err(|e| {
                warn!(
                    "Registration failed for user: {} in collection '{}' - {}",
                    identifier, collection, e
                );
                match e {
                    oxide_core::AppError::Conflict { .. } => ApiError::bad_request(
                        "User with this identifier already exists".to_string(),
                    ),
                    oxide_core::AppError::Validation { field, message } => {
                        ApiError::bad_request(format!("Validation error in {}: {}", field, message))
                    }
                    oxide_core::AppError::Auth { .. } => ApiError::forbidden(
                        "Registration is not enabled for this collection".to_string(),
                    ),
                    _ => ApiError::internal(format!("Failed to register user: {}", e)),
                }
            })?;

        let response = RegisterResponse {
            user_id,
            identifier,
            role: auth_config.default_role.to_string(),
            auth_collection: collection.clone(),
            message: format!(
                "User registered successfully in collection '{}'",
                collection
            ),
        };

        info!(
            "✅ User registration successful: {} in collection '{}' ({})",
            response.identifier, collection, response.role
        );
        Ok(response)
    }

    /// List available auth collections
    pub async fn list_auth_collections(
        db: Arc<dyn Db>,
        auth_service: Arc<oxide_core::AuthService>,
    ) -> Result<AuthCollectionsResponse, ApiError> {
        debug!("📋 Listing auth collections");

        let auth_collections = db.list_auth_collections().await?;
        let mut collections = Vec::new();

        for schema in auth_collections {
            if let Some(auth_config) = auth_service.config().get_auth_collection(&schema.name) {
                collections.push(AuthCollectionInfo {
                    name: schema.name,
                    identifier_field: auth_config.identifier_field.clone(),
                    registration_enabled: auth_config.registration_enabled,
                    email_verification_required: auth_config.email_verification_required,
                });
            }
        }

        Ok(AuthCollectionsResponse { collections })
    }

    /// Validate a JWT token
    pub async fn validate_token(
        auth_service: Arc<oxide_core::AuthService>,
        token: String,
    ) -> Result<TokenValidationResponse, ApiError> {
        debug!("🔍 Validating token");

        match auth_service.verify_token(&token) {
            Ok(claims) => {
                let response = TokenValidationResponse {
                    valid: true,
                    user_id: Some(claims.sub),
                    email: Some(claims.email),
                    role: Some(claims.role),
                    auth_collection: Some(claims.auth_collection),
                    expires_at: Some(claims.exp),
                    custom_claims: if !claims.custom.is_empty() {
                        Some(serde_json::to_value(claims.custom).unwrap_or(serde_json::Value::Null))
                    } else {
                        None
                    },
                };
                Ok(response)
            }
            Err(_) => {
                let response = TokenValidationResponse {
                    valid: false,
                    user_id: None,
                    email: None,
                    role: None,
                    auth_collection: None,
                    expires_at: None,
                    custom_claims: None,
                };
                Ok(response)
            }
        }
    }

    /// Get current user information from token
    /// Refresh an access token using a refresh token
    pub async fn refresh_token(
        db: Arc<dyn Db>,
        auth_service: Arc<oxide_core::AuthService>,
        refresh_token: String,
    ) -> Result<RefreshTokenResponse, ApiError> {
        debug!("🔄 Processing token refresh request");

        let refresh_claims = auth_service
            .verify_refresh_token(&refresh_token)
            .map_err(|e| {
                warn!("Token refresh failed: {}", e);
                ApiError::auth("Invalid or expired refresh token".to_string())
            })?;
        let old_token_hash = hash_refresh_token(&refresh_token);

        // Refresh the token pair
        let token_pair = auth_service
            .refresh_token_pair(&refresh_token)
            .map_err(|e| {
                warn!("Token refresh failed: {}", e);
                ApiError::auth("Invalid or expired refresh token".to_string())
            })?;

        let new_refresh_claims = auth_service
            .verify_refresh_token(&token_pair.refresh_token)
            .map_err(|e| {
                warn!("Generated refresh token could not be verified: {}", e);
                ApiError::internal("Failed to generate refresh token".to_string())
            })?;
        let new_token_record = refresh_token_record(
            hash_refresh_token(&token_pair.refresh_token),
            &new_refresh_claims,
        );

        db.rotate_refresh_token(&old_token_hash, new_token_record)
            .await
            .map_err(|e| {
                warn!(
                    "Token refresh failed for user {} from collection {}: {}",
                    refresh_claims.sub, refresh_claims.auth_collection, e
                );
                ApiError::auth("Invalid or expired refresh token".to_string())
            })?;

        let response = RefreshTokenResponse {
            access_token: Some(token_pair.access_token),
            refresh_token: Some(token_pair.refresh_token),
            expires_in: token_pair.access_token_expires_in,
            refresh_expires_in: token_pair.refresh_token_expires_in,
        };

        info!("✅ Token refresh successful");
        Ok(response)
    }

    pub async fn logout(
        db: Arc<dyn Db>,
        refresh_token: Option<String>,
    ) -> Result<String, ApiError> {
        if let Some(refresh_token) = refresh_token.filter(|token| !token.trim().is_empty()) {
            db.revoke_refresh_token(&hash_refresh_token(&refresh_token))
                .await
                .map_err(|e| {
                    warn!("Refresh token revocation failed during logout: {}", e);
                    ApiError::internal("Failed to revoke refresh token".to_string())
                })?;
        }

        info!("👋 User logout processed");
        Ok("Logged out successfully".to_string())
    }

    pub async fn get_current_user(claims: Claims) -> Result<CurrentUserResponse, ApiError> {
        debug!("👤 Getting current user info for: {}", claims.email);

        let response = CurrentUserResponse {
            user_id: claims.sub,
            email: claims.email,
            role: claims.role,
            auth_collection: claims.auth_collection,
            expires_at: claims.exp,
            custom_claims: if !claims.custom.is_empty() {
                Some(serde_json::to_value(claims.custom).unwrap_or(serde_json::Value::Null))
            } else {
                None
            },
        };

        Ok(response)
    }
}

async fn persist_refresh_token(
    db: Arc<dyn Db>,
    auth_service: Arc<oxide_core::AuthService>,
    refresh_token: &str,
) -> Result<(), ApiError> {
    let claims = auth_service
        .verify_refresh_token(refresh_token)
        .map_err(|e| {
            warn!("Generated refresh token could not be verified: {}", e);
            ApiError::internal("Failed to generate refresh token".to_string())
        })?;

    db.store_refresh_token(refresh_token_record(
        hash_refresh_token(refresh_token),
        &claims,
    ))
    .await
    .map_err(|e| {
        warn!("Failed to persist refresh token metadata: {}", e);
        ApiError::internal("Failed to store refresh token".to_string())
    })
}

fn refresh_token_record(token_hash: String, claims: &RefreshClaims) -> RefreshTokenRecord {
    RefreshTokenRecord {
        token_hash,
        user_id: claims.sub.clone(),
        auth_collection: claims.auth_collection.clone(),
        jti: claims.jti.clone(),
        expires_at: claims.exp,
    }
}

fn hash_refresh_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

pub(crate) fn extract_cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    cookie_header.split(';').find_map(|part| {
        let (cookie_name, cookie_value) = part.trim().split_once('=')?;
        (cookie_name == name).then(|| cookie_value.to_string())
    })
}

fn auth_cookie_headers(
    access_token: &str,
    refresh_token: Option<&str>,
    access_max_age_seconds: i64,
    refresh_max_age_seconds: Option<i64>,
) -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    append_set_cookie_header(
        &mut headers,
        build_auth_cookie(ACCESS_TOKEN_COOKIE, access_token, access_max_age_seconds)?,
    );

    if let (Some(refresh_token), Some(max_age)) = (refresh_token, refresh_max_age_seconds) {
        append_set_cookie_header(
            &mut headers,
            build_auth_cookie(REFRESH_TOKEN_COOKIE, refresh_token, max_age)?,
        );
    }

    Ok(headers)
}

fn clear_auth_cookie_headers() -> Result<HeaderMap, ApiError> {
    let mut headers = HeaderMap::new();
    append_set_cookie_header(
        &mut headers,
        build_expired_auth_cookie(ACCESS_TOKEN_COOKIE)?,
    );
    append_set_cookie_header(
        &mut headers,
        build_expired_auth_cookie(REFRESH_TOKEN_COOKIE)?,
    );
    Ok(headers)
}

fn append_set_cookie_header(headers: &mut HeaderMap, cookie: HeaderValue) {
    headers.append(header::SET_COOKIE, cookie);
}

fn build_auth_cookie(
    name: &str,
    value: &str,
    max_age_seconds: i64,
) -> Result<HeaderValue, ApiError> {
    cookie_header_value(name, value, Some(max_age_seconds))
}

fn build_expired_auth_cookie(name: &str) -> Result<HeaderValue, ApiError> {
    cookie_header_value(name, "", Some(0))
}

fn cookie_header_value(
    name: &str,
    value: &str,
    max_age_seconds: Option<i64>,
) -> Result<HeaderValue, ApiError> {
    let mut cookie = format!(
        "{name}={value}; Path=/; HttpOnly; SameSite={}",
        cookie_same_site()
    );

    if let Some(max_age_seconds) = max_age_seconds {
        cookie.push_str(&format!("; Max-Age={max_age_seconds}"));
    }

    if cookie_secure() {
        cookie.push_str("; Secure");
    }

    HeaderValue::from_str(&cookie)
        .map_err(|e| ApiError::internal(format!("Failed to build auth cookie: {}", e)))
}

fn cookie_same_site() -> &'static str {
    match std::env::var("OXIDEDB_COOKIE_SAME_SITE")
        .unwrap_or_else(|_| "Lax".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "strict" => "Strict",
        "none" => "None",
        _ => "Lax",
    }
}

fn cookie_secure() -> bool {
    if let Ok(value) = std::env::var("OXIDEDB_COOKIE_SECURE") {
        return matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        );
    }

    ["OXIDEDB_ENV", "OXIDE_ENV", "APP_ENV", "ENV", "NODE_ENV"]
        .iter()
        .any(|name| {
            std::env::var(name)
                .map(|value| value.trim().eq_ignore_ascii_case("production"))
                .unwrap_or(false)
        })
}

// HTTP Handler Functions

/// List available auth collections
///
/// GET /auth/collections
pub async fn list_auth_collections(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<AuthCollectionsResponse>>, ApiError> {
    let response = AuthHandlers::list_auth_collections(state.db, state.auth_service).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// User login for specific collection
///
/// POST /auth/{collection}/login
pub async fn login_collection(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(request): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<ApiResponse<LoginResponse>>), ApiError> {
    let cookie_session = request.cookie_session;
    let mut response = AuthHandlers::login(
        state.db,
        state.auth_service,
        collection,
        request.identifier,
        request.credential,
    )
    .await?;

    let headers = if cookie_session {
        let headers = auth_cookie_headers(
            response
                .token
                .as_deref()
                .ok_or_else(|| ApiError::internal("Missing access token".to_string()))?,
            response.refresh_token.as_deref(),
            response.expires_in,
            response.refresh_expires_in,
        )?;
        response.token = None;
        response.refresh_token = None;
        headers
    } else {
        HeaderMap::new()
    };

    Ok((headers, Json(ApiResponse::success(response))))
}

/// User registration for specific collection
///
/// POST /auth/{collection}/register
pub async fn register_collection(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<ApiResponse<RegisterResponse>>), ApiError> {
    let response = AuthHandlers::register(
        state.db,
        state.auth_service,
        collection,
        request.identifier,
        request.credential,
        request.additional_data,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::success(response))))
}

/// Token validation
///
/// POST /auth/validate
pub async fn validate_token(
    State(state): State<AppState>,
    Json(request): Json<TokenValidationRequest>,
) -> Result<Json<ApiResponse<TokenValidationResponse>>, ApiError> {
    let response = AuthHandlers::validate_token(state.auth_service, request.token).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// Get current user information
///
/// GET /auth/me
pub async fn get_current_user(
    authenticated_user: AuthenticatedUser,
) -> Result<Json<ApiResponse<CurrentUserResponse>>, ApiError> {
    let response = AuthHandlers::get_current_user(authenticated_user.claims).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// User logout
///
/// POST /auth/logout
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(HeaderMap, Json<ApiResponse<String>>), ApiError> {
    let request =
        if body.is_empty() {
            None
        } else {
            Some(serde_json::from_slice::<LogoutRequest>(&body).map_err(|e| {
                ApiError::bad_request(format!("Invalid logout request body: {}", e))
            })?)
        };

    let refresh_token = request
        .and_then(|request| request.refresh_token)
        .or_else(|| extract_cookie_value(&headers, REFRESH_TOKEN_COOKIE));
    let response = AuthHandlers::logout(state.db, refresh_token).await?;
    Ok((
        clear_auth_cookie_headers()?,
        Json(ApiResponse::success(response)),
    ))
}

/// Refresh token handler
///
/// POST /auth/refresh
pub async fn refresh_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(HeaderMap, Json<ApiResponse<RefreshTokenResponse>>), ApiError> {
    let request = if body.is_empty() {
        RefreshTokenRequest {
            refresh_token: None,
            cookie_session: true,
        }
    } else {
        serde_json::from_slice::<RefreshTokenRequest>(&body)
            .map_err(|e| ApiError::bad_request(format!("Invalid refresh request body: {}", e)))?
    };

    let cookie_refresh_token = extract_cookie_value(&headers, REFRESH_TOKEN_COOKIE);
    let cookie_session = request.cookie_session || cookie_refresh_token.is_some();
    let refresh_token = request
        .refresh_token
        .or(cookie_refresh_token)
        .ok_or_else(|| ApiError::auth("Missing refresh token".to_string()))?;

    let mut response =
        AuthHandlers::refresh_token(state.db, state.auth_service, refresh_token).await?;

    let headers = if cookie_session {
        let headers = auth_cookie_headers(
            response
                .access_token
                .as_deref()
                .ok_or_else(|| ApiError::internal("Missing refreshed access token".to_string()))?,
            response.refresh_token.as_deref(),
            response.expires_in,
            Some(response.refresh_expires_in),
        )?;
        response.access_token = None;
        response.refresh_token = None;
        headers
    } else {
        HeaderMap::new()
    };

    Ok((headers, Json(ApiResponse::success(response))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_token_hash_is_deterministic_and_non_plaintext() {
        let hash = hash_refresh_token("refresh-token-value");

        assert_eq!(hash, hash_refresh_token("refresh-token-value"));
        assert_eq!(hash.len(), 64);
        assert_ne!(hash, "refresh-token-value");
    }

    #[test]
    fn refresh_claims_are_mapped_to_persisted_token_metadata() {
        let claims = RefreshClaims {
            sub: "user-1".to_string(),
            email: "user@example.com".to_string(),
            role: "user".to_string(),
            auth_collection: "_users".to_string(),
            exp: 12345,
            iat: 10000,
            typ: "refresh".to_string(),
            iss: None,
            aud: None,
            jti: "jti-1".to_string(),
        };

        let record = refresh_token_record("hash".to_string(), &claims);

        assert_eq!(record.token_hash, "hash");
        assert_eq!(record.user_id, "user-1");
        assert_eq!(record.auth_collection, "_users");
        assert_eq!(record.jti, "jti-1");
        assert_eq!(record.expires_at, 12345);
    }
}
