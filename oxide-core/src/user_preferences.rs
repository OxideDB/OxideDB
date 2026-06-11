//! User preferences management
//!
//! This module provides types and traits for managing user preferences
//! in a database-agnostic way.

use crate::AppError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// User preference data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    /// User ID (from JWT claims)
    pub user_id: String,
    /// Preference key (e.g., "field_customization", "ui_settings")
    pub preference_key: String,
    /// Preference value as JSON
    pub preference_value: Value,
    /// When the preference was created
    pub created_at: i64,
    /// When the preference was last updated
    pub updated_at: i64,
}

/// Request to update user preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserPreferenceRequest {
    /// Preference key
    pub preference_key: String,
    /// New preference value
    pub preference_value: Value,
}

/// Response for user preference operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferenceResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// Optional message
    pub message: Option<String>,
    /// The preference data (for get operations)
    pub preference: Option<UserPreferences>,
}

/// Response for listing user preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferencesListResponse {
    /// List of user preferences
    pub preferences: Vec<UserPreferences>,
    /// Total count
    pub total: usize,
}

/// Trait for user preferences service
#[async_trait]
pub trait UserPreferencesService {
    /// Store a user preference
    async fn store_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
        preference_value: &Value,
    ) -> Result<(), AppError>;

    /// Get a user preference by key
    async fn get_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
    ) -> Result<Option<Value>, AppError>;

    /// Get all preferences for a user
    async fn get_user_preferences(&self, user_id: &str) -> Result<Vec<UserPreferences>, AppError>;

    /// Delete a specific user preference
    async fn delete_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
    ) -> Result<bool, AppError>;

    /// Delete all preferences for a user
    async fn delete_all_user_preferences(&self, user_id: &str) -> Result<u32, AppError>;
}

/// Common preference keys used throughout the system
pub mod preference_keys {
    /// Field customization settings for collection forms
    pub const FIELD_CUSTOMIZATION: &str = "field_customization";
    /// General UI settings and preferences
    pub const UI_SETTINGS: &str = "ui_settings";
    /// Dashboard layout preferences
    pub const DASHBOARD_LAYOUT: &str = "dashboard_layout";
    /// Collection view preferences
    pub const COLLECTION_VIEW: &str = "collection_view";
}
