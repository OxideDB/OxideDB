//! Site settings HTTP handlers
//!
//! This module provides HTTP handlers for managing system-wide site settings including
//! branding, SMTP configuration, version information, and other application preferences.

use axum::{
    extract::{Path, Query, State},
    Json,
};
use oxide_core::{
    site_settings::{
        settings_sections, SettingsHealthStatus, SiteSettingsResponse, UpdateSiteSettingsRequest,
    },
    AppError,
};
use oxide_db::Db;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};

/// Query parameters for settings operations
#[derive(Debug, Deserialize)]
pub struct SettingsQuery {
    /// Include health status in response
    pub include_health: Option<bool>,
}

/// Site settings handlers
pub struct SiteSettingsHandlers;

impl SiteSettingsHandlers {
    /// Get all site settings with optional health information
    pub async fn get_settings<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        is_superuser: bool,
        include_health: bool,
    ) -> Result<SiteSettingsResponse, ApiError> {
        debug!("Getting site settings for user: {}", user_id);

        // Check if user has admin role (required for viewing settings)
        Self::check_admin_permission(&user_id, is_superuser, "read site settings").await?;

        let settings = db.get_site_settings().await.map_err(|e| {
            warn!("Failed to get site settings: {}", e);
            ApiError::internal(format!("Failed to retrieve site settings: {}", e))
        })?;

        let mut response = SiteSettingsResponse {
            success: true,
            message: None,
            settings: Some(settings),
        };

        // Include health information if requested
        if include_health {
            match db.get_settings_health().await {
                Ok(health) => {
                    if !health.healthy {
                        response.message = Some(format!(
                            "Settings retrieved with {} warnings",
                            health.warnings.len()
                        ));
                    }
                }
                Err(e) => {
                    warn!("Failed to get settings health: {}", e);
                    response.message =
                        Some("Settings retrieved but health check failed".to_string());
                }
            }
        }

        info!("✅ Retrieved site settings for user: {}", user_id);
        Ok(response)
    }

    /// Update site settings (partial update)
    pub async fn update_settings<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        is_superuser: bool,
        request: UpdateSiteSettingsRequest,
    ) -> Result<SiteSettingsResponse, ApiError> {
        debug!("Updating site settings for user: {}", user_id);

        // Check if user has admin role (required for updating settings)
        Self::check_admin_permission(&user_id, is_superuser, "update site settings").await?;

        // Validate the update request
        Self::validate_update_request(&request)?;

        db.update_site_settings(request).await.map_err(|e| {
            warn!("Failed to update site settings: {}", e);
            ApiError::internal(format!("Failed to update site settings: {}", e))
        })?;

        info!("✅ Updated site settings for user: {}", user_id);

        Ok(SiteSettingsResponse {
            success: true,
            message: Some("Site settings updated successfully".to_string()),
            settings: None,
        })
    }

    /// Reset site settings to defaults
    pub async fn reset_settings<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        is_superuser: bool,
    ) -> Result<SiteSettingsResponse, ApiError> {
        debug!("Resetting site settings for user: {}", user_id);

        // Check if user has admin role (required for resetting settings)
        Self::check_admin_permission(&user_id, is_superuser, "reset site settings").await?;

        db.reset_site_settings().await.map_err(|e| {
            warn!("Failed to reset site settings: {}", e);
            ApiError::internal(format!("Failed to reset site settings: {}", e))
        })?;

        info!("✅ Reset site settings for user: {}", user_id);

        Ok(SiteSettingsResponse {
            success: true,
            message: Some("Site settings reset to defaults".to_string()),
            settings: None,
        })
    }

    /// Get a specific settings section
    pub async fn get_settings_section<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        is_superuser: bool,
        section: String,
    ) -> Result<Value, ApiError> {
        debug!(
            "Getting settings section '{}' for user: {}",
            section, user_id
        );

        // Check if user has admin role (required for viewing settings)
        Self::check_admin_permission(&user_id, is_superuser, "read site settings").await?;

        // Validate section name
        Self::validate_section_name(&section)?;

        let section_data = db.get_settings_section(&section).await.map_err(|e| {
            warn!("Failed to get settings section '{}': {}", section, e);
            match e {
                AppError::NotFound { .. } => {
                    ApiError::not_found(format!("Settings section '{}' not found", section))
                }
                _ => ApiError::internal(format!("Failed to retrieve settings section: {}", e)),
            }
        })?;

        info!(
            "✅ Retrieved settings section '{}' for user: {}",
            section, user_id
        );
        Ok(section_data)
    }

    /// Update a specific settings section
    pub async fn update_settings_section<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        is_superuser: bool,
        section: String,
        data: Value,
    ) -> Result<SiteSettingsResponse, ApiError> {
        debug!(
            "Updating settings section '{}' for user: {}",
            section, user_id
        );

        // Check if user has admin role (required for updating settings)
        Self::check_admin_permission(&user_id, is_superuser, "update site settings").await?;

        // Validate section name
        Self::validate_section_name(&section)?;

        // Validate and normalize section data based on section type.
        let data = Self::validate_section_data(&section, &data)?;

        db.update_settings_section(&section, &data)
            .await
            .map_err(|e| {
                warn!("Failed to update settings section '{}': {}", section, e);
                ApiError::internal(format!("Failed to update settings section: {}", e))
            })?;

        info!(
            "✅ Updated settings section '{}' for user: {}",
            section, user_id
        );

        Ok(SiteSettingsResponse {
            success: true,
            message: Some(format!(
                "Settings section '{}' updated successfully",
                section
            )),
            settings: None,
        })
    }

    /// Test email configuration
    pub async fn test_email_configuration<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        is_superuser: bool,
    ) -> Result<SiteSettingsResponse, ApiError> {
        debug!("Testing email configuration for user: {}", user_id);

        // Check if user has admin role (required for testing email)
        Self::check_admin_permission(&user_id, is_superuser, "test email configuration").await?;

        let test_result = db.test_email_configuration().await.map_err(|e| {
            warn!("Failed to test email configuration: {}", e);
            ApiError::internal(format!("Failed to test email configuration: {}", e))
        })?;

        let message = if test_result {
            "Email configuration test passed"
        } else {
            "Email configuration test failed - check SMTP settings"
        };

        info!(
            "Email configuration test result for user {}: {}",
            user_id, test_result
        );

        Ok(SiteSettingsResponse {
            success: test_result,
            message: Some(message.to_string()),
            settings: None,
        })
    }

    /// Get settings health status
    pub async fn get_settings_health<D: Db + ?Sized>(
        db: Arc<D>,
        user_id: String,
        is_superuser: bool,
    ) -> Result<SettingsHealthStatus, ApiError> {
        debug!("Getting settings health for user: {}", user_id);

        // Check if user has admin role (required for viewing health)
        Self::check_admin_permission(&user_id, is_superuser, "view settings health").await?;

        let health = db.get_settings_health().await.map_err(|e| {
            warn!("Failed to get settings health: {}", e);
            ApiError::internal(format!("Failed to get settings health: {}", e))
        })?;

        info!("✅ Retrieved settings health for user: {}", user_id);
        Ok(health)
    }

    /// Check if user has admin permission for the given operation
    async fn check_admin_permission(
        user_id: &str,
        is_superuser: bool,
        operation: &str,
    ) -> Result<(), ApiError> {
        debug!(
            "Checking admin permission for user {} to {}",
            user_id, operation
        );

        if is_superuser {
            Ok(())
        } else {
            warn!(
                "User {} attempted to {} without superuser privileges",
                user_id, operation
            );
            Err(ApiError::forbidden(format!(
                "Superuser privileges are required to {}",
                operation
            )))
        }
    }

    /// Validate the update request structure
    fn validate_update_request(request: &UpdateSiteSettingsRequest) -> Result<(), ApiError> {
        // Validate branding settings if provided
        if let Some(branding) = &request.branding {
            if branding.site_title.trim().is_empty() {
                return Err(ApiError::bad_request(
                    "Site title cannot be empty".to_string(),
                ));
            }

            // Validate color formats if provided
            if let Some(primary_color) = &branding.primary_color {
                if !Self::is_valid_hex_color(primary_color) {
                    return Err(ApiError::bad_request(
                        "Primary color must be a valid hex color".to_string(),
                    ));
                }
            }

            if let Some(secondary_color) = &branding.secondary_color {
                if !Self::is_valid_hex_color(secondary_color) {
                    return Err(ApiError::bad_request(
                        "Secondary color must be a valid hex color".to_string(),
                    ));
                }
            }
        }

        // Validate email settings if provided
        if let Some(email) = &request.email {
            if email.enabled {
                if email
                    .smtp_host
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
                {
                    return Err(ApiError::bad_request(
                        "SMTP host is required when email is enabled".to_string(),
                    ));
                }

                if email
                    .from_email
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or_default()
                    .is_empty()
                {
                    return Err(ApiError::bad_request(
                        "From email is required when email is enabled".to_string(),
                    ));
                }

                // Validate email format
                if let Some(from_email) = &email.from_email {
                    if !Self::is_valid_email(from_email) {
                        return Err(ApiError::bad_request(
                            "From email must be a valid email address".to_string(),
                        ));
                    }
                }
            }
        }

        // Validate general settings if provided
        if let Some(general) = &request.general {
            if general.max_upload_size > 1024 * 1024 * 1024 {
                return Err(ApiError::bad_request(
                    "Maximum upload size cannot exceed 1GB".to_string(),
                ));
            }

            if general.api_rate_limit == 0 {
                return Err(ApiError::bad_request(
                    "API rate limit must be greater than 0".to_string(),
                ));
            }
        }

        // Validate security settings if provided
        if let Some(security) = &request.security {
            if security.password_min_length < 4 {
                return Err(ApiError::bad_request(
                    "Password minimum length must be at least 4 characters".to_string(),
                ));
            }

            if security.password_min_length > 128 {
                return Err(ApiError::bad_request(
                    "Password minimum length cannot exceed 128 characters".to_string(),
                ));
            }

            if security.session_timeout_minutes == 0 {
                return Err(ApiError::bad_request(
                    "Session timeout must be greater than 0".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Validate settings section name
    fn validate_section_name(section: &str) -> Result<(), ApiError> {
        match section {
            settings_sections::BRANDING
            | settings_sections::EMAIL
            | settings_sections::SYSTEM_INFO
            | settings_sections::GENERAL
            | settings_sections::SECURITY => Ok(()),
            _ => Err(ApiError::bad_request(format!(
                "Invalid settings section: {}",
                section
            ))),
        }
    }

    /// Validate section data based on section type
    fn validate_section_data(section: &str, data: &Value) -> Result<Value, ApiError> {
        match section {
            settings_sections::BRANDING => {
                let branding: oxide_core::site_settings::BrandingSettings =
                    serde_json::from_value(data.clone()).map_err(|e| {
                        ApiError::bad_request(format!("Invalid branding data: {}", e))
                    })?;
                serde_json::to_value(&branding).map_err(|e| {
                    ApiError::internal(format!("Failed to normalize branding settings: {}", e))
                })
            }
            settings_sections::EMAIL => {
                let email: oxide_core::site_settings::EmailSettings =
                    serde_json::from_value(data.clone())
                        .map_err(|e| ApiError::bad_request(format!("Invalid email data: {}", e)))?;
                serde_json::to_value(&email).map_err(|e| {
                    ApiError::internal(format!("Failed to normalize email settings: {}", e))
                })
            }
            settings_sections::SYSTEM_INFO => {
                let system_info: oxide_core::site_settings::SystemInfoSettings =
                    serde_json::from_value(data.clone()).map_err(|e| {
                        ApiError::bad_request(format!("Invalid system info data: {}", e))
                    })?;
                serde_json::to_value(&system_info).map_err(|e| {
                    ApiError::internal(format!("Failed to normalize system info settings: {}", e))
                })
            }
            settings_sections::GENERAL => {
                let general: oxide_core::site_settings::GeneralSettings =
                    serde_json::from_value(data.clone()).map_err(|e| {
                        ApiError::bad_request(format!("Invalid general settings data: {}", e))
                    })?;
                serde_json::to_value(&general).map_err(|e| {
                    ApiError::internal(format!("Failed to normalize general settings: {}", e))
                })
            }
            settings_sections::SECURITY => {
                let security: oxide_core::site_settings::SecuritySettings =
                    serde_json::from_value(data.clone()).map_err(|e| {
                        ApiError::bad_request(format!("Invalid security settings data: {}", e))
                    })?;
                serde_json::to_value(&security).map_err(|e| {
                    ApiError::internal(format!("Failed to normalize security settings: {}", e))
                })
            }
            _ => Err(ApiError::bad_request(format!(
                "Unknown settings section: {}",
                section
            ))),
        }
    }

    /// Validate hex color format
    fn is_valid_hex_color(color: &str) -> bool {
        if !color.starts_with('#') || color.len() != 7 {
            return false;
        }

        color.chars().skip(1).all(|c| c.is_ascii_hexdigit())
    }

    /// Validate email format (basic validation)
    fn is_valid_email(email: &str) -> bool {
        email.contains('@') && email.contains('.') && email.len() > 5
    }
}

// HTTP Handler Functions

/// Get all site settings
///
/// GET /api/admin/settings
pub async fn get_site_settings(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Query(query): Query<SettingsQuery>,
) -> Result<Json<ApiResponse<SiteSettingsResponse>>, ApiError> {
    let response = SiteSettingsHandlers::get_settings(
        state.db,
        authenticated_user.user_id().to_string(),
        authenticated_user.is_superuser(),
        query.include_health.unwrap_or(false),
    )
    .await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Update site settings (partial update)
///
/// PUT /api/admin/settings
pub async fn update_site_settings(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Json(request): Json<UpdateSiteSettingsRequest>,
) -> Result<Json<ApiResponse<SiteSettingsResponse>>, ApiError> {
    let response = SiteSettingsHandlers::update_settings(
        state.db,
        authenticated_user.user_id().to_string(),
        authenticated_user.is_superuser(),
        request,
    )
    .await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Reset site settings to defaults
///
/// POST /api/admin/settings/reset
pub async fn reset_site_settings(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
) -> Result<Json<ApiResponse<SiteSettingsResponse>>, ApiError> {
    let response = SiteSettingsHandlers::reset_settings(
        state.db,
        authenticated_user.user_id().to_string(),
        authenticated_user.is_superuser(),
    )
    .await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Get a specific settings section
///
/// GET /api/admin/settings/{section}
pub async fn get_settings_section(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Path(section): Path<String>,
) -> Result<Json<ApiResponse<Value>>, ApiError> {
    let data = SiteSettingsHandlers::get_settings_section(
        state.db,
        authenticated_user.user_id().to_string(),
        authenticated_user.is_superuser(),
        section,
    )
    .await?;

    Ok(Json(ApiResponse::success(data)))
}

/// Update a specific settings section
///
/// PUT /api/admin/settings/{section}
pub async fn update_settings_section(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
    Path(section): Path<String>,
    Json(data): Json<Value>,
) -> Result<Json<ApiResponse<SiteSettingsResponse>>, ApiError> {
    let response = SiteSettingsHandlers::update_settings_section(
        state.db,
        authenticated_user.user_id().to_string(),
        authenticated_user.is_superuser(),
        section,
        data,
    )
    .await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Test email configuration
///
/// POST /api/admin/settings/email/test
pub async fn test_email_configuration(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
) -> Result<Json<ApiResponse<SiteSettingsResponse>>, ApiError> {
    let response = SiteSettingsHandlers::test_email_configuration(
        state.db,
        authenticated_user.user_id().to_string(),
        authenticated_user.is_superuser(),
    )
    .await?;

    Ok(Json(ApiResponse::success(response)))
}

/// Get settings health status
///
/// GET /api/admin/settings/health
pub async fn get_settings_health(
    State(state): State<AppState>,
    authenticated_user: AuthenticatedUser,
) -> Result<Json<ApiResponse<SettingsHealthStatus>>, ApiError> {
    let health = SiteSettingsHandlers::get_settings_health(
        state.db,
        authenticated_user.user_id().to_string(),
        authenticated_user.is_superuser(),
    )
    .await?;

    Ok(Json(ApiResponse::success(health)))
}
