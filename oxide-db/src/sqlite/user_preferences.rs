//! User preferences storage implementation for SQLite
//!
//! This module implements user preference storage operations for the SQLite database.
//! User preferences are stored in a dedicated table and can be retrieved per user.

use super::SqliteDb;
use async_trait::async_trait;
use oxide_core::{
    user_preferences::{UserPreferences as CoreUserPreferences, UserPreferencesService},
    AppError,
};
use serde_json::Value;
use tokio::task::spawn_blocking;
use tracing::{debug, info};

/// User preference data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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

impl SqliteDb {
    /// Create the user preferences table if it doesn't exist
    pub(super) async fn create_user_preferences_table(&self) -> Result<(), AppError> {
        let pool = self.pool.clone();

        spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get pooled connection: {}", e))
            })?;

            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS user_preferences (
                    user_id TEXT NOT NULL,
                    preference_key TEXT NOT NULL,
                    preference_value TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    PRIMARY KEY (user_id, preference_key)
                )
                "#,
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to create user preferences table: {}", e))
            })?;

            debug!("✅ User preferences table ready");
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Store a user preference
    pub async fn store_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
        preference_value: &Value,
    ) -> Result<(), AppError> {
        debug!(
            "Storing user preference: {} for user: {}",
            preference_key, user_id
        );

        // Ensure preferences table exists
        self.create_user_preferences_table().await?;

        let preference_json = serde_json::to_string(preference_value)
            .map_err(|e| AppError::internal(format!("Failed to serialize preference: {}", e)))?;

        let user_id = user_id.to_string();
        let preference_key = preference_key.to_string();
        let pool = self.pool.clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        spawn_blocking(move || {
            let conn = pool.get().map_err(|e| AppError::database(format!("Failed to get pooled connection: {}", e)))?;

            // Use INSERT OR REPLACE to handle both new and existing preferences
            conn.execute(
                r#"
                INSERT OR REPLACE INTO user_preferences 
                (user_id, preference_key, preference_value, created_at, updated_at) 
                VALUES (?1, ?2, ?3, 
                    COALESCE((SELECT created_at FROM user_preferences WHERE user_id = ?1 AND preference_key = ?2), ?4), 
                    ?4)
                "#,
                [&user_id, &preference_key, &preference_json, &now.to_string()],
            )
            .map_err(|e| AppError::database(format!("Failed to store preference: {}", e)))?;

            info!("✅ Stored preference '{}' for user: {}", &preference_key, &user_id);
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Get a user preference by key
    pub async fn get_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
    ) -> Result<Option<Value>, AppError> {
        debug!(
            "Getting user preference '{}' for user: {}",
            preference_key, user_id
        );

        // Ensure preferences table exists
        self.create_user_preferences_table().await?;

        // Clone for logging purposes before moving into spawn_blocking
        let user_id_for_log = user_id.to_string();
        let preference_key_for_log = preference_key.to_string();

        let user_id = user_id.to_string();
        let preference_key = preference_key.to_string();
        let pool = self.pool.clone();

        let preference_json = spawn_blocking(move || {
            let conn = pool.get().map_err(|e| AppError::database(format!("Failed to get pooled connection: {}", e)))?;

            let mut stmt = conn
                .prepare("SELECT preference_value FROM user_preferences WHERE user_id = ?1 AND preference_key = ?2")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let result = stmt.query_row([&user_id, &preference_key], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            });

            match result {
                Ok(json) => Ok(Some(json)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(format!("Failed to get preference: {}", e))),
            }
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        match preference_json {
            Some(json) => {
                let preference_value: Value = serde_json::from_str(&json).map_err(|e| {
                    AppError::internal(format!("Failed to deserialize preference: {}", e))
                })?;

                debug!(
                    "✅ Found preference '{}' for user: {}",
                    preference_key_for_log, user_id_for_log
                );
                Ok(Some(preference_value))
            }
            None => {
                debug!(
                    "No preference '{}' found for user: {}",
                    preference_key_for_log, user_id_for_log
                );
                Ok(None)
            }
        }
    }

    /// Get all preferences for a user
    pub async fn get_user_preferences(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserPreferences>, AppError> {
        debug!("Getting all preferences for user: {}", user_id);

        // Ensure preferences table exists
        self.create_user_preferences_table().await?;

        let user_id = user_id.to_string();
        let pool = self.pool.clone();

        spawn_blocking(move || {
            let conn = pool.get().map_err(|e| AppError::database(format!("Failed to get pooled connection: {}", e)))?;

            let mut stmt = conn
                .prepare("SELECT user_id, preference_key, preference_value, created_at, updated_at FROM user_preferences WHERE user_id = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let rows = stmt.query_map([&user_id], |row| {
                let preference_value_str: String = row.get(2)?;
                let preference_value: Value = serde_json::from_str(&preference_value_str)
                    .map_err(|_| rusqlite::Error::InvalidColumnType(2, "Invalid JSON".to_string(), rusqlite::types::Type::Text))?;

                Ok(UserPreferences {
                    user_id: row.get(0)?,
                    preference_key: row.get(1)?,
                    preference_value,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })
            .map_err(|e| AppError::database(format!("Failed to query preferences: {}", e)))?;

            let mut preferences = Vec::new();
            for row in rows {
                let preference = row.map_err(|e| AppError::database(format!("Failed to parse preference row: {}", e)))?;
                preferences.push(preference);
            }

            debug!("✅ Found {} preferences for user: {}", preferences.len(), user_id);
            Ok(preferences)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Delete a specific user preference
    pub async fn delete_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
    ) -> Result<bool, AppError> {
        debug!(
            "Deleting user preference '{}' for user: {}",
            preference_key, user_id
        );

        // Ensure preferences table exists
        self.create_user_preferences_table().await?;

        let user_id = user_id.to_string();
        let preference_key = preference_key.to_string();
        let pool = self.pool.clone();

        spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get pooled connection: {}", e))
            })?;

            // Clone for logging before moving into SQL query
            let preference_key_for_log = preference_key.clone();
            let user_id_for_log = user_id.clone();

            let rows_affected = conn
                .execute(
                    "DELETE FROM user_preferences WHERE user_id = ?1 AND preference_key = ?2",
                    [&user_id, &preference_key],
                )
                .map_err(|e| AppError::database(format!("Failed to delete preference: {}", e)))?;

            let deleted = rows_affected > 0;
            if deleted {
                info!(
                    "✅ Deleted preference '{}' for user: {}",
                    preference_key_for_log, user_id_for_log
                );
            } else {
                debug!(
                    "No preference '{}' found to delete for user: {}",
                    preference_key_for_log, user_id_for_log
                );
            }

            Ok(deleted)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Delete all preferences for a user
    pub async fn delete_all_user_preferences(&self, user_id: &str) -> Result<u32, AppError> {
        debug!("Deleting all preferences for user: {}", user_id);

        // Ensure preferences table exists
        self.create_user_preferences_table().await?;

        let user_id = user_id.to_string();
        let pool = self.pool.clone();

        spawn_blocking(move || {
            let conn = pool.get().map_err(|e| {
                AppError::database(format!("Failed to get pooled connection: {}", e))
            })?;

            let rows_affected = conn
                .execute(
                    "DELETE FROM user_preferences WHERE user_id = ?1",
                    [&user_id],
                )
                .map_err(|e| {
                    AppError::database(format!("Failed to delete user preferences: {}", e))
                })?;

            info!(
                "✅ Deleted {} preferences for user: {}",
                rows_affected, user_id
            );
            Ok(rows_affected as u32)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }
}

// Implement the UserPreferencesService trait for SqliteDb
#[async_trait]
impl UserPreferencesService for SqliteDb {
    async fn store_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
        preference_value: &Value,
    ) -> Result<(), AppError> {
        self.store_user_preference(user_id, preference_key, preference_value)
            .await
    }

    async fn get_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
    ) -> Result<Option<Value>, AppError> {
        self.get_user_preference(user_id, preference_key).await
    }

    async fn get_user_preferences(
        &self,
        user_id: &str,
    ) -> Result<Vec<CoreUserPreferences>, AppError> {
        let db_prefs = self.get_user_preferences(user_id).await?;

        // Convert from DB UserPreferences to Core UserPreferences
        let core_prefs = db_prefs
            .into_iter()
            .map(|db_pref| CoreUserPreferences {
                user_id: db_pref.user_id,
                preference_key: db_pref.preference_key,
                preference_value: db_pref.preference_value,
                created_at: db_pref.created_at,
                updated_at: db_pref.updated_at,
            })
            .collect();

        Ok(core_prefs)
    }

    async fn delete_user_preference(
        &self,
        user_id: &str,
        preference_key: &str,
    ) -> Result<bool, AppError> {
        self.delete_user_preference(user_id, preference_key).await
    }

    async fn delete_all_user_preferences(&self, user_id: &str) -> Result<u32, AppError> {
        self.delete_all_user_preferences(user_id).await
    }
}
