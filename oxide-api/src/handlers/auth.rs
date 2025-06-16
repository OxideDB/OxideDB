//! Authentication handlers
//!
//! This module provides HTTP handlers for user authentication including
//! login, logout, registration, and token management using the collection-based
//! authentication system.

use axum::{
    extract::{State, Path},
    http::StatusCode,
    Json,
};
use oxide_core::Claims;
use oxide_db::{Db, db::{AuthRequest, RegisterRequest as DbRegisterRequest}};
use std::sync::Arc;
use tracing::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
    extractors::AuthenticatedUser,
};

/// Login request payload for collection-specific authentication
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub identifier: String,
    pub credential: String,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub auth_collection: String,
    pub expires_in: i64,
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
        debug!("🔑 Processing login request for collection '{}': {}", collection, identifier);

        // Get auth configuration for the collection
        let auth_config = auth_service.config().get_auth_collection(&collection)
            .ok_or_else(|| ApiError::bad_request(format!("Collection '{}' is not configured for authentication", collection)))?;

        // Create auth request
        let auth_request = AuthRequest {
            collection: collection.clone(),
            identifier: identifier.clone(),
            credential,
        };

        // Authenticate user
        let auth_response = db.authenticate_user(auth_request, auth_config).await
            .map_err(|e| {
                warn!("Authentication failed for user: {} in collection '{}' - {}", identifier, collection, e);
                ApiError::auth("Invalid credentials".to_string())
            })?;

        let response = LoginResponse {
            token: auth_response.token,
            user_id: auth_response.user_id,
            email: identifier.clone(), // For now, using identifier as email
            role: auth_response.role,
            auth_collection: auth_response.auth_collection,
            expires_in: auth_service.config().token_expiry_hours * 3600,
            custom_claims: if auth_response.user_data.as_object().map_or(false, |obj| !obj.is_empty()) {
                Some(auth_response.user_data)
            } else {
                None
            },
        };

        info!("✅ User login successful: {} from collection '{}' ({})", identifier, collection, response.role);
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
        debug!("👤 Processing registration for collection '{}': {}", collection, identifier);

        // Get auth configuration for the collection
        let auth_config = auth_service.config().get_auth_collection(&collection)
            .ok_or_else(|| ApiError::bad_request(format!("Collection '{}' is not configured for authentication", collection)))?;

        // Validate identifier format (basic validation)
        if identifier.is_empty() || identifier.len() < 3 {
            return Err(ApiError::bad_request("Identifier must be at least 3 characters long".to_string()));
        }

        // Validate credential strength (basic validation)
        if credential.len() < 8 {
            return Err(ApiError::bad_request("Credential must be at least 8 characters long".to_string()));
        }

        // Create registration request
        let register_request = DbRegisterRequest {
            collection: collection.clone(),
            identifier: identifier.clone(),
            credential,
            additional_data,
        };

        // Register user
        let user_id = db.register_user(register_request, auth_config).await
            .map_err(|e| {
                warn!("Registration failed for user: {} in collection '{}' - {}", identifier, collection, e);
                match e {
                    oxide_core::AppError::Conflict { .. } => ApiError::bad_request("User with this identifier already exists".to_string()),
                    oxide_core::AppError::Validation { field, message } => ApiError::bad_request(format!("Validation error in {}: {}", field, message)),
                    oxide_core::AppError::Auth { .. } => ApiError::forbidden("Registration is not enabled for this collection".to_string()),
                    _ => ApiError::internal(format!("Failed to register user: {}", e)),
                }
            })?;

        let response = RegisterResponse {
            user_id,
            identifier,
            role: auth_config.default_role.to_string(),
            auth_collection: collection.clone(),
            message: format!("User registered successfully in collection '{}'", collection),
        };

        info!("✅ User registration successful: {} in collection '{}' ({})", response.identifier, collection, response.role);
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
    pub async fn get_current_user(
        claims: Claims,
    ) -> Result<CurrentUserResponse, ApiError> {
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
) -> Result<Json<ApiResponse<LoginResponse>>, ApiError> {
    let response = AuthHandlers::login(
        state.db,
        state.auth_service,
        collection,
        request.identifier,
        request.credential,
    ).await?;
    Ok(Json(ApiResponse::success(response)))
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
    ).await?;
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

/// User logout (client-side token removal)
///
/// POST /auth/logout
pub async fn logout() -> Result<Json<ApiResponse<String>>, ApiError> {
    // For JWT tokens, logout is typically handled client-side by removing the token
    // In a real implementation, you might want to maintain a blacklist of revoked tokens
    info!("👋 User logout processed");
    Ok(Json(ApiResponse::success("Logged out successfully".to_string())))
}