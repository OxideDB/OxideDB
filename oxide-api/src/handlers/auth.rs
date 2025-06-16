//! Authentication handlers
//!
//! This module provides HTTP handlers for user authentication including
//! login, logout, registration, and token management.

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use oxide_core::{Claims, UserRole};
use oxide_db::{Db, db::ListParams};
use std::sync::Arc;
use tracing::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
    extractors::AuthenticatedUser,
};

/// Login request payload
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub expires_in: i64,
}

/// Registration request payload
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub is_superuser: Option<bool>,
}

/// Registration response
#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub message: String,
}

/// Token validation response
#[derive(Debug, Serialize)]
pub struct TokenValidationResponse {
    pub valid: bool,
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub expires_at: Option<i64>,
}

/// Handlers for authentication operations
pub struct AuthHandlers;

impl AuthHandlers {
    /// Authenticate a user with email and password
    pub async fn login(
        db: Arc<dyn Db>,
        auth_service: Arc<oxide_core::AuthService>,
        email: String,
        password: String,
    ) -> Result<LoginResponse, ApiError> {
        debug!("🔑 Processing login request for: {}", email);

        // Check if users collection exists
        if !db.collection_exists("users").await.unwrap_or(false) {
            warn!("Users collection does not exist");
            return Err(ApiError::auth("Authentication system not initialized".to_string()));
        }

        // Find user by email in the users collection
        let users = db.list_records("users", ListParams::default()).await
            .map_err(|e| ApiError::internal(format!("Failed to query users: {}", e)))?;
        
        let user_record = users.iter().find(|record| {
            record.data.get("email").and_then(|e| e.as_str()) == Some(&email)
        }).ok_or_else(|| ApiError::auth("Invalid email or password".to_string()))?;

        // Verify password
        let stored_password_hash = user_record.data.get("password_hash")
            .and_then(|p| p.as_str())
            .ok_or_else(|| ApiError::internal("User record missing password hash".to_string()))?;

        let password_valid = auth_service.verify_password(&password, stored_password_hash)
            .map_err(|e| ApiError::internal(format!("Password verification failed: {}", e)))?;

        if !password_valid {
            warn!("Invalid password attempt for user: {}", email);
            return Err(ApiError::auth("Invalid email or password".to_string()));
        }

        // Determine user role
        let is_superuser = user_record.data.get("is_superuser")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        
        let role = if is_superuser { UserRole::Superuser } else { UserRole::User };
        let user_id = user_record.id.clone();

        // Generate JWT token
        let token = auth_service.generate_token(user_id.clone(), email.clone(), role.clone())
            .map_err(|e| ApiError::internal(format!("Failed to generate token: {}", e)))?;

        let response = LoginResponse {
            token,
            user_id,
            email,
            role: role.to_string(),
            expires_in: 24 * 60 * 60, // 24 hours in seconds
        };

        info!("✅ User login successful: {} ({})", response.email, response.role);
        Ok(response)
    }

    /// Register a new user
    pub async fn register(
        db: Arc<dyn Db>,
        auth_service: Arc<oxide_core::AuthService>,
        email: String,
        password: String,
        is_superuser: bool,
    ) -> Result<RegisterResponse, ApiError> {
        debug!("👤 Processing registration for: {} (superuser: {})", email, is_superuser);

        // Validate email format
        if !email.contains('@') || email.len() < 5 {
            return Err(ApiError::bad_request("Invalid email format".to_string()));
        }

        // Validate password strength
        if password.len() < 8 {
            return Err(ApiError::bad_request("Password must be at least 8 characters long".to_string()));
        }

        // Check if users collection exists, create if it doesn't
        if !db.collection_exists("users").await.unwrap_or(false) {
            return Err(ApiError::internal("Users collection not initialized".to_string()));
        }

        // Check if user already exists
        let existing_users = db.list_records("users", ListParams::default()).await
            .map_err(|e| ApiError::internal(format!("Failed to query users: {}", e)))?;
        
        if existing_users.iter().any(|record| {
            record.data.get("email").and_then(|e| e.as_str()) == Some(&email)
        }) {
            return Err(ApiError::bad_request("User with this email already exists".to_string()));
        }

        // Hash the password
        let password_hash = auth_service.hash_password(&password)
            .map_err(|e| ApiError::internal(format!("Failed to hash password: {}", e)))?;

        // Create user record
        let mut user_data = std::collections::HashMap::new();
        user_data.insert("email".to_string(), serde_json::Value::String(email.clone()));
        user_data.insert("password_hash".to_string(), serde_json::Value::String(password_hash));
        user_data.insert("is_superuser".to_string(), serde_json::Value::Bool(is_superuser));
        user_data.insert("created_at".to_string(), serde_json::Value::String(chrono::Utc::now().to_rfc3339()));

        let user_record = db.create_record("users", serde_json::Value::Object(user_data.into_iter().collect()))
            .await
            .map_err(|e| ApiError::internal(format!("Failed to create user: {}", e)))?;

        let role = if is_superuser { "superuser" } else { "user" };
        let response = RegisterResponse {
            user_id: user_record.id,
            email,
            role: role.to_string(),
            message: "User registered successfully".to_string(),
        };

        info!("✅ User registration successful: {} ({})", response.email, response.role);
        Ok(response)
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
                    expires_at: Some(claims.exp),
                };
                Ok(response)
            }
            Err(_) => {
                let response = TokenValidationResponse {
                    valid: false,
                    user_id: None,
                    email: None,
                    role: None,
                    expires_at: None,
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
            expires_at: claims.exp,
        };

        Ok(response)
    }
}

/// Current user response
#[derive(Debug, Serialize)]
pub struct CurrentUserResponse {
    pub user_id: String,
    pub email: String,
    pub role: String,
    pub expires_at: i64,
}

// HTTP Handler Functions

/// User login
///
/// POST /auth/login
pub async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<ApiResponse<LoginResponse>>, ApiError> {
    let response = AuthHandlers::login(state.db, state.auth_service, request.email, request.password).await?;
    Ok(Json(ApiResponse::success(response)))
}

/// User registration
///
/// POST /auth/register
pub async fn register(
    State(state): State<AppState>,
    Json(request): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<ApiResponse<RegisterResponse>>), ApiError> {
    let is_superuser = request.is_superuser.unwrap_or(false);
    let response = AuthHandlers::register(state.db, state.auth_service, request.email, request.password, is_superuser).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::success(response))))
}

/// Token validation
///
/// GET /auth/validate
pub async fn validate_token(
    authenticated_user: AuthenticatedUser,
) -> Result<Json<ApiResponse<TokenValidationResponse>>, ApiError> {
    let response = TokenValidationResponse {
        valid: true,
        user_id: Some(authenticated_user.claims.sub),
        email: Some(authenticated_user.claims.email),
        role: Some(authenticated_user.claims.role),
        expires_at: Some(authenticated_user.claims.exp),
    };
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