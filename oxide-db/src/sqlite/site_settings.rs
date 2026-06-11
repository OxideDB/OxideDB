//! Site settings storage implementation for SQLite
//!
//! This module implements site settings storage operations for the SQLite database.
//! Site settings are stored in a dedicated system collection and manage system-wide configuration.

use super::SqliteDb;
use crate::db::{Db, ListParams};
use async_trait::async_trait;
use oxide_core::{
    site_settings::{
        settings_sections, BrandingSettings, EmailSettings, GeneralSettings, SecuritySettings,
        SettingsHealthStatus, SiteSettings, SiteSettingsService, SystemInfoSettings,
        UpdateSiteSettingsRequest,
    },
    AppError,
};
use serde_json::Value;
use tracing::{debug, info, warn};

impl SqliteDb {
    /// Create the site settings system collection if it doesn't exist
    pub(super) async fn create_site_settings_collection(&self) -> Result<(), AppError> {
        debug!("Creating site settings system collection");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Define the site settings collection schema
        let schema_json = serde_json::json!({
            "id": "_site_settings",
            "name": "_site_settings",
            "collection_type": "base",
            "fields": {
                "setting_key": {
                    "field_type": "text",
                    "required": true,
                    "unique": true,
                    "index": true,
                    "validation": {
                        "pattern": "^[a-z_]+$",
                        "message": "Setting key must contain only lowercase letters and underscores"
                    }
                },
                "setting_value": {
                    "field_type": "json",
                    "required": true,
                    "unique": false
                },
                "setting_section": {
                    "field_type": "text",
                    "required": true,
                    "index": true,
                    "unique": false,
                    "validation": {
                        "enum": ["branding", "email", "system_info", "general", "security"],
                        "message": "Invalid settings section"
                    }
                }
            },
            "indexes": [],
            "created_at": now,
            "updated_at": now
        });

        // Check if collection already exists
        let existing_schema = self.get_collection_schema("_site_settings").await;
        if existing_schema.is_ok() {
            debug!("✅ Site settings collection already exists");
            return Ok(());
        }

        // Create the collection
        let schema: oxide_core::collection::CollectionSchema = serde_json::from_value(schema_json)
            .map_err(|e| {
                AppError::internal(format!("Failed to parse site settings schema: {}", e))
            })?;

        self.create_collection_with_schema(schema).await?;

        // Initialize with default settings if no settings exist
        self.initialize_default_settings().await?;

        info!("✅ Site settings system collection created and initialized");
        Ok(())
    }

    /// Initialize default site settings
    async fn initialize_default_settings(&self) -> Result<(), AppError> {
        debug!("Initializing default site settings");

        let default_settings = SiteSettings::default();
        // Store each settings section separately
        let sections = vec![
            (
                settings_sections::BRANDING,
                serde_json::to_value(&default_settings.branding)?,
            ),
            (
                settings_sections::EMAIL,
                serde_json::to_value(&default_settings.email)?,
            ),
            (
                settings_sections::SYSTEM_INFO,
                serde_json::to_value(&default_settings.system_info)?,
            ),
            (
                settings_sections::GENERAL,
                serde_json::to_value(&default_settings.general)?,
            ),
            (
                settings_sections::SECURITY,
                serde_json::to_value(&default_settings.security)?,
            ),
        ];

        for (section, value) in sections {
            let record_data = serde_json::json!({
                "setting_key": section,
                "setting_value": value,
                "setting_section": section
            });

            self.create_record("_site_settings", record_data).await?;
        }

        info!("✅ Default site settings initialized");
        Ok(())
    }

    /// Get a settings section from storage
    async fn get_settings_section_from_db(&self, section: &str) -> Result<Value, AppError> {
        debug!("Getting settings section: {}", section);

        let list_params = ListParams::default();
        let records = self.list_records("_site_settings", list_params).await?;

        for record in records {
            if let Some(setting_key) = record.data.get("setting_key") {
                if setting_key.as_str() == Some(section) {
                    if let Some(setting_value) = record.data.get("setting_value") {
                        return Ok(setting_value.clone());
                    }
                }
            }
        }

        Err(AppError::not_found("settings_section", section))
    }

    /// Update a settings section in storage
    async fn update_settings_section_in_db(
        &self,
        section: &str,
        data: &Value,
    ) -> Result<(), AppError> {
        debug!("Updating settings section: {}", section);

        let list_params = ListParams::default();
        let records = self.list_records("_site_settings", list_params).await?;

        // Find the record for this section
        let mut found = false;
        for record in records {
            if let Some(setting_key) = record.data.get("setting_key") {
                if setting_key.as_str() == Some(section) {
                    let update_data = serde_json::json!({
                        "setting_key": section,
                        "setting_value": data,
                        "setting_section": section
                    });

                    self.update_record("_site_settings", &record.id, update_data)
                        .await?;
                    info!("✅ Updated settings section: {}", section);
                    found = true;
                    break;
                }
            }
        }

        if !found {
            // Create new section if it doesn't exist
            let record_data = serde_json::json!({
                "setting_key": section,
                "setting_value": data,
                "setting_section": section
            });

            self.create_record("_site_settings", record_data).await?;
            info!("✅ Created new settings section: {}", section);
        }

        Ok(())
    }

    /// Get the latest update timestamp from all settings sections
    async fn get_latest_settings_timestamp(&self) -> Result<i64, AppError> {
        let list_params = ListParams::default();
        let records = self.list_records("_site_settings", list_params).await?;

        let mut latest_timestamp = 0i64;
        for record in records {
            if record.updated_at > latest_timestamp {
                latest_timestamp = record.updated_at;
            }
        }

        Ok(latest_timestamp)
    }

    /// Validate email configuration
    async fn validate_email_configuration(
        &self,
        email_settings: &EmailSettings,
    ) -> Result<bool, AppError> {
        if !email_settings.enabled {
            return Ok(true); // Valid if disabled
        }

        // Check required fields for SMTP
        if email_settings.smtp_host.is_none() {
            return Ok(false);
        }

        if email_settings.smtp_username.is_none() {
            return Ok(false);
        }

        if email_settings.from_email.is_none() {
            return Ok(false);
        }

        // TODO: In a real implementation, you might want to test the SMTP connection
        // For now, we'll just validate that required fields are present
        Ok(true)
    }

    /// Validate license information
    async fn validate_license(&self, system_info: &SystemInfoSettings) -> Result<bool, AppError> {
        match &system_info.oxidedb_edition {
            oxide_core::site_settings::OxideDbEdition::Community => Ok(true), // No license required
            oxide_core::site_settings::OxideDbEdition::Professional
            | oxide_core::site_settings::OxideDbEdition::Enterprise => {
                if system_info.license_key.is_none() {
                    return Ok(false);
                }

                // Check license expiration
                if let Some(expires_at) = system_info.license_expires_at {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;

                    if expires_at < now {
                        return Ok(false); // License expired
                    }
                }

                // TODO: In a real implementation, validate the license key signature
                Ok(true)
            }
        }
    }
}

// Implement the SiteSettingsService trait for SqliteDb
#[async_trait]
impl SiteSettingsService for SqliteDb {
    async fn get_site_settings(&self) -> Result<SiteSettings, AppError> {
        debug!("Getting complete site settings");

        // Ensure settings collection exists
        self.create_site_settings_collection().await?;

        // Get all settings sections, falling back to defaults if a section is missing/invalid
        let branding_value = self
            .get_settings_section_from_db(settings_sections::BRANDING)
            .await
            .unwrap_or_default();
        let email_value = self
            .get_settings_section_from_db(settings_sections::EMAIL)
            .await
            .unwrap_or_default();
        let system_info_value = self
            .get_settings_section_from_db(settings_sections::SYSTEM_INFO)
            .await
            .unwrap_or_default();
        let general_value = self
            .get_settings_section_from_db(settings_sections::GENERAL)
            .await
            .unwrap_or_default();
        let security_value = self
            .get_settings_section_from_db(settings_sections::SECURITY)
            .await
            .unwrap_or_default();

        // Deserialize sections
        let branding: BrandingSettings = serde_json::from_value(branding_value)
            .map_err(|e| {
                warn!("Branding settings invalid, using default: {}", e);
                AppError::internal(e.to_string())
            })
            .unwrap_or_else(|_| {
                warn!("Could not deserialize branding settings, falling back to default.");
                BrandingSettings::default()
            });

        let email: EmailSettings = serde_json::from_value(email_value)
            .map_err(|e| {
                warn!("Email settings invalid, using default: {}", e);
                AppError::internal(e.to_string())
            })
            .unwrap_or_else(|_| {
                warn!("Could not deserialize email settings, falling back to default.");
                EmailSettings::default()
            });

        let mut system_info: SystemInfoSettings = serde_json::from_value(system_info_value)
            .map_err(|e| {
                warn!("System info settings invalid, using default: {}", e);
                AppError::internal(e.to_string())
            })
            .unwrap_or_else(|_| {
                warn!("Could not deserialize system info, falling back to default.");
                SystemInfoSettings::default()
            });

        let general: GeneralSettings = serde_json::from_value(general_value)
            .map_err(|e| {
                warn!("General settings invalid, using default: {}", e);
                AppError::internal(e.to_string())
            })
            .unwrap_or_else(|_| {
                warn!("Could not deserialize general settings, falling back to default.");
                GeneralSettings::default()
            });

        let security: SecuritySettings = serde_json::from_value(security_value)
            .map_err(|e| {
                warn!("Security settings invalid, using default: {}", e);
                AppError::internal(e.to_string())
            })
            .unwrap_or_else(|_| {
                warn!("Could not deserialize security settings, falling back to default.");
                SecuritySettings::default()
            });

        // Always update version from current binary
        system_info.oxidedb_version = env!("CARGO_PKG_VERSION").to_string();

        // Get timestamps
        let latest_timestamp = self.get_latest_settings_timestamp().await?;

        Ok(SiteSettings {
            branding,
            email,
            system_info,
            general,
            security,
            created_at: latest_timestamp, // Use as both for simplicity
            updated_at: latest_timestamp,
        })
    }

    async fn update_site_settings(
        &self,
        request: UpdateSiteSettingsRequest,
    ) -> Result<(), AppError> {
        debug!("Updating site settings");

        // Ensure settings collection exists
        self.create_site_settings_collection().await?;

        // Update each provided section
        if let Some(branding) = request.branding {
            let value = serde_json::to_value(&branding)
                .map_err(|e| AppError::internal(format!("Failed to serialize branding: {}", e)))?;
            self.update_settings_section_in_db(settings_sections::BRANDING, &value)
                .await?;
        }

        if let Some(email) = request.email {
            let value = serde_json::to_value(&email).map_err(|e| {
                AppError::internal(format!("Failed to serialize email settings: {}", e))
            })?;
            self.update_settings_section_in_db(settings_sections::EMAIL, &value)
                .await?;
        }

        if let Some(system_info_update) = request.system_info {
            // Get current system info and apply updates. If the record is missing or cannot be deserialized
            // we fall back to defaults to avoid breaking the update request altogether.
            let current_value = self
                .get_settings_section_from_db(settings_sections::SYSTEM_INFO)
                .await
                .unwrap_or_default();

            let mut current_system_info: SystemInfoSettings =
                serde_json::from_value(current_value.clone()).unwrap_or_else(|e| {
                    warn!(
                        "Current system info invalid, falling back to default: {}",
                        e
                    );
                    SystemInfoSettings::default()
                });

            // Apply updates (only allow certain fields to be changed)
            if let Some(edition) = system_info_update.oxidedb_edition {
                current_system_info.oxidedb_edition = edition;
            }
            if let Some(environment) = system_info_update.environment {
                current_system_info.environment = environment;
            }
            if let Some(instance_name) = system_info_update.instance_name {
                current_system_info.instance_name = Some(instance_name);
            }
            if let Some(license_key) = system_info_update.license_key {
                current_system_info.license_key = Some(license_key);
            }

            // Always update version from current binary
            current_system_info.oxidedb_version = env!("CARGO_PKG_VERSION").to_string();

            let value = serde_json::to_value(&current_system_info).map_err(|e| {
                AppError::internal(format!("Failed to serialize system info: {}", e))
            })?;
            self.update_settings_section_in_db(settings_sections::SYSTEM_INFO, &value)
                .await?;
        }

        if let Some(general) = request.general {
            let value = serde_json::to_value(&general).map_err(|e| {
                AppError::internal(format!("Failed to serialize general settings: {}", e))
            })?;
            self.update_settings_section_in_db(settings_sections::GENERAL, &value)
                .await?;
        }

        if let Some(security) = request.security {
            let value = serde_json::to_value(&security).map_err(|e| {
                AppError::internal(format!("Failed to serialize security settings: {}", e))
            })?;
            self.update_settings_section_in_db(settings_sections::SECURITY, &value)
                .await?;
        }

        info!("✅ Site settings updated successfully");
        Ok(())
    }

    async fn reset_site_settings(&self) -> Result<(), AppError> {
        debug!("Resetting site settings to defaults");

        // Delete all current settings records
        let list_params = ListParams::default();
        let records = self.list_records("_site_settings", list_params).await?;
        for record in records {
            self.delete_record("_site_settings", &record.id).await?;
        }

        // Reinitialize with defaults
        self.initialize_default_settings().await?;

        info!("✅ Site settings reset to defaults");
        Ok(())
    }

    async fn get_settings_section(&self, section: &str) -> Result<Value, AppError> {
        debug!("Getting settings section: {}", section);

        // Ensure settings collection exists
        self.create_site_settings_collection().await?;

        self.get_settings_section_from_db(section).await
    }

    async fn update_settings_section(&self, section: &str, data: &Value) -> Result<(), AppError> {
        debug!("Updating settings section: {}", section);

        // Ensure settings collection exists
        self.create_site_settings_collection().await?;

        self.update_settings_section_in_db(section, data).await
    }

    async fn test_email_configuration(&self) -> Result<bool, AppError> {
        debug!("Testing email configuration");

        let email_settings_value = self
            .get_settings_section_from_db(settings_sections::EMAIL)
            .await?;
        let email_settings: EmailSettings =
            serde_json::from_value(email_settings_value).map_err(|e| {
                AppError::internal(format!("Failed to deserialize email settings: {}", e))
            })?;

        // TODO: In a real implementation, this would send a test email
        // For now, we just validate the configuration
        self.validate_email_configuration(&email_settings).await
    }

    async fn get_settings_health(&self) -> Result<SettingsHealthStatus, AppError> {
        debug!("Getting settings health status");

        let settings = self.get_site_settings().await?;
        let mut warnings = Vec::new();
        let mut healthy = true;

        // Validate email configuration
        let email_valid = self.validate_email_configuration(&settings.email).await?;
        if !email_valid && settings.email.enabled {
            warnings.push("Email configuration is incomplete or invalid".to_string());
            healthy = false;
        }

        // Validate license
        let license_valid = self.validate_license(&settings.system_info).await?;
        if !license_valid {
            warnings.push("License is missing or expired".to_string());
            healthy = false;
        }

        // Check for other potential issues
        if settings.general.max_upload_size > 100 * 1024 * 1024 {
            warnings.push("Large file upload size may impact performance".to_string());
        }

        if settings.security.password_min_length < 8 {
            warnings.push("Password minimum length is below recommended 8 characters".to_string());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Ok(SettingsHealthStatus {
            healthy,
            email_config_valid: email_valid,
            license_valid,
            warnings,
            last_validated_at: now,
        })
    }
}
