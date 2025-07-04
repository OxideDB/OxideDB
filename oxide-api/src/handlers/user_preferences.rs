//! User preferences HTTP handlers
//!
//! This module provides HTTP handlers for managing user preferences including
//! storing, retrieving, updating, and deleting user-specific settings.

use axum::{
    extract::{State, Path, Query},
    http::StatusCode,
    Json,
};
use oxide_core::{
    user_preferences::{
        UserPreferencesService, UpdateUserPreferenceRequest, UserPreferenceResponse,
        UserPreferencesListResponse, preference_keys,
    },
    AppError,
};
use oxide_db::Db;
use std::sync::Arc;
use tracing::{debug, info, warn};
use serde::{Deserialize, Serialize};

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
    extractors::AuthenticatedUser,
};

/// Query parameters for listing user preferences
#[derive(Debug, Deserialize)]
pub struct ListPreferencesQuery {
    /// Filter by preference key
    pub key: Option<String>,
}

/// User preferences handlers
pub struct UserPreferencesHandlers;

impl UserPreferencesHandlers {
    /// Store or update a user preference
    pub async fn store_preference<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        request: UpdateUserPreferenceRequest,
    ) -> Result<UserPreferenceResponse, ApiError> {
        debug!("Storing preference '{}' for user: {}", request.preference_key, user_id);

        // Validate preference key (optional - add custom validation if needed)
        if request.preference_key.is_empty() {
            return Err(ApiError::bad_request("Preference key cannot be empty".to_string()));
        }

        db.store_user_preference(&user_id, &request.preference_key, &request.preference_value)
            .await
            .map_err(|e| {
                warn!("Failed to store preference '{}' for user {}: {}", request.preference_key, user_id, e);
                ApiError::internal(format!("Failed to store preference: {}", e))
            })?;

        info!("✅ Stored preference '{}' for user: {}", request.preference_key, user_id);

        Ok(UserPreferenceResponse {
            success: true,
            message: Some("Preference stored successfully".to_string()),
            preference: None,
        })
    }

    /// Get a specific user preference by key
    pub async fn get_preference<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        preference_key: String,
    ) -> Result<UserPreferenceResponse, ApiError> {
        debug!("Getting preference '{}' for user: {}", preference_key, user_id);

        let preference_value = db.get_user_preference(&user_id, &preference_key)
            .await
            .map_err(|e| {
                warn!("Failed to get preference '{}' for user {}: {}", preference_key, user_id, e);
                ApiError::internal(format!("Failed to retrieve preference: {}", e))
            })?;

        match preference_value {
            Some(value) => {
                debug!("✅ Found preference '{}' for user: {}", preference_key, user_id);
                Ok(UserPreferenceResponse {
                    success: true,
                    message: None,
                    preference: Some(oxide_core::user_preferences::UserPreferences {
                        user_id,
                        preference_key,
                        preference_value: value,
                        created_at: 0, // Will be populated from DB in real implementation
                        updated_at: 0, // Will be populated from DB in real implementation
                    }),
                })
            }
            None => {
                debug!("No preference '{}' found for user: {}", preference_key, user_id);
                Ok(UserPreferenceResponse {
                    success: true,
                    message: Some("Preference not found".to_string()),
                    preference: None,
                })
            }
        }
    }

    /// Get all preferences for a user
    pub async fn list_preferences<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        query: Option<ListPreferencesQuery>,
    ) -> Result<UserPreferencesListResponse, ApiError> {
        debug!("Getting all preferences for user: {}", user_id);

        let mut preferences = db.get_user_preferences(&user_id)
            .await
            .map_err(|e| {
                warn!("Failed to get preferences for user {}: {}", user_id, e);
                ApiError::internal(format!("Failed to retrieve preferences: {}", e))
            })?;

        // Filter by key if specified
        if let Some(query) = query {
            if let Some(key_filter) = query.key {
                preferences.retain(|pref| pref.preference_key == key_filter);
            }
        }

        info!("✅ Retrieved {} preferences for user: {}", preferences.len(), user_id);

        Ok(UserPreferencesListResponse {
            total: preferences.len(),
            preferences,
        })
    }

    /// Delete a specific user preference
    pub async fn delete_preference<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        preference_key: String,
    ) -> Result<UserPreferenceResponse, ApiError> {
        debug!("Deleting preference '{}' for user: {}", preference_key, user_id);

        let deleted = db.delete_user_preference(&user_id, &preference_key)
            .await
            .map_err(|e| {
                warn!("Failed to delete preference '{}' for user {}: {}", preference_key, user_id, e);
                ApiError::internal(format!("Failed to delete preference: {}", e))
            })?;

        if deleted {
            info!("✅ Deleted preference '{}' for user: {}", preference_key, user_id);
            Ok(UserPreferenceResponse {
                success: true,
                message: Some("Preference deleted successfully".to_string()),
                preference: None,
            })
        } else {
            debug!("No preference '{}' found to delete for user: {}", preference_key, user_id);
            Ok(UserPreferenceResponse {
                success: true,
                message: Some("Preference not found".to_string()),
                preference: None,
            })
        }
    }

    /// Delete all preferences for a user
    pub async fn delete_all_preferences<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
    ) -> Result<UserPreferenceResponse, ApiError> {
        debug!("Deleting all preferences for user: {}", user_id);

        let deleted_count = db.delete_all_user_preferences(&user_id)
            .await
            .map_err(|e| {
                warn!("Failed to delete all preferences for user {}: {}", user_id, e);
                ApiError::internal(format!("Failed to delete preferences: {}", e))
            })?;

        info!("✅ Deleted {} preferences for user: {}", deleted_count, user_id);

        Ok(UserPreferenceResponse {
            success: true,
            message: Some(format!("Deleted {} preferences", deleted_count)),
            preference: None,
        })
    }
}

// HTTP Handler Functions

/// Store or update a user preference
///
/// PUT /api/user/preferences/{key}
pub async fn store_user_preference(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Path(preference_key): Path<String>,
    Json(request_body): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<UserPreferenceResponse>>, ApiError> {
    let request = UpdateUserPreferenceRequest {
        preference_key,
        preference_value: request_body,
    };

    let response = UserPreferencesHandlers::store_preference(
        state.db,
        authenticated_user.user_id().to_string(),
        request,
    ).await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Get a specific user preference by key
///
/// GET /api/user/preferences/{key}
pub async fn get_user_preference(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Path(preference_key): Path<String>,
) -> Result<Json<ApiResponse<UserPreferenceResponse>>, ApiError> {
    let response = UserPreferencesHandlers::get_preference(
        state.db,
        authenticated_user.user_id().to_string(),
        preference_key,
    ).await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Get all preferences for the authenticated user
///
/// GET /api/user/preferences
pub async fn list_user_preferences(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Query(query): Query<ListPreferencesQuery>,
) -> Result<Json<ApiResponse<UserPreferencesListResponse>>, ApiError> {
    let response = UserPreferencesHandlers::list_preferences(
        state.db,
        authenticated_user.user_id().to_string(),
        Some(query),
    ).await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Delete a specific user preference
///
/// DELETE /api/user/preferences/{key}
pub async fn delete_user_preference(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Path(preference_key): Path<String>,
) -> Result<Json<ApiResponse<UserPreferenceResponse>>, ApiError> {
    let response = UserPreferencesHandlers::delete_preference(
        state.db,
        authenticated_user.user_id().to_string(),
        preference_key,
    ).await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Delete all preferences for the authenticated user
///
/// DELETE /api/user/preferences
pub async fn delete_all_user_preferences(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
) -> Result<Json<ApiResponse<UserPreferenceResponse>>, ApiError> {
    let response = UserPreferencesHandlers::delete_all_preferences(
        state.db,
        authenticated_user.user_id().to_string(),
    ).await?;

    Ok(Json(ApiResponse::success(response)))
} 