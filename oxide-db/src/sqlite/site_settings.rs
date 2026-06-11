//! Site settings storage implementation for SQLite
//!
//! This module implements site settings storage operations for the SQLite database.
//! Site settings are stored in a dedicated system collection and manage system-wide configuration.

use super::SqliteDb;
use crate::db::{Db, ListParams};
use async_trait::async_trait;
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use oxide_core::{
    site_settings::{
        settings_sections, BrandingSettings, EmailSettings, GeneralSettings, SecuritySettings,
        SettingsHealthStatus, SiteSettings, SiteSettingsService, SystemInfoSettings,
        UpdateSiteSettingsRequest,
    },
    AppError,
};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;
use tokio::{net::TcpStream, time::timeout};
use tracing::{debug, info, warn};

const SMTP_CONNECT_TIMEOUT_SECONDS: u64 = 5;
const LICENSE_PUBLIC_KEY_ENV: &str = "OXIDEDB_LICENSE_PUBLIC_KEY";
const LICENSE_PUBLIC_KEY_FILE_ENV: &str = "OXIDEDB_LICENSE_PUBLIC_KEY_FILE";

#[derive(Debug, Deserialize)]
struct SignedLicenseDocument {
    algorithm: Option<String>,
    payload: String,
    signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn professional_system_info(expires_at: i64) -> SystemInfoSettings {
        SystemInfoSettings {
            oxidedb_version: "0.1.0".to_string(),
            oxidedb_edition: oxide_core::site_settings::OxideDbEdition::Professional,
            installation_id: "installation-1".to_string(),
            environment: oxide_core::site_settings::DeploymentEnvironment::Production,
            instance_name: None,
            license_key: None,
            license_expires_at: Some(expires_at),
        }
    }

    fn signed_license(signing_key: &SigningKey, payload: &[u8]) -> String {
        let signature = signing_key.sign(payload);
        serde_json::json!({
            "algorithm": "ed25519",
            "payload": STANDARD.encode(payload),
            "signature": STANDARD.encode(signature.to_bytes()),
        })
        .to_string()
    }

    #[test]
    fn signed_license_verifies_matching_claims() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let expires_at = current_unix_timestamp() + 3600;
        let payload = serde_json::json!({
            "edition": "professional",
            "installation_id": "installation-1",
            "expires_at": expires_at,
        })
        .to_string();
        let license = signed_license(&signing_key, payload.as_bytes());
        let system_info = professional_system_info(expires_at);

        let verified =
            verify_signed_license_with_key(&system_info, &license, &signing_key.verifying_key())
                .unwrap();

        assert!(verified);
    }

    #[test]
    fn signed_license_rejects_wrong_installation() {
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let expires_at = current_unix_timestamp() + 3600;
        let payload = serde_json::json!({
            "edition": "professional",
            "installation_id": "other-installation",
            "expires_at": expires_at,
        })
        .to_string();
        let license = signed_license(&signing_key, payload.as_bytes());
        let system_info = professional_system_info(expires_at);

        let verified =
            verify_signed_license_with_key(&system_info, &license, &signing_key.verifying_key())
                .unwrap();

        assert!(!verified);
    }
}

#[derive(Debug, Deserialize)]
struct LicenseClaims {
    edition: String,
    expires_at: i64,
    installation_id: Option<String>,
    not_before: Option<i64>,
}

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

        let smtp_host = email_settings
            .smtp_host
            .as_deref()
            .map(str::trim)
            .filter(|host| !host.is_empty());
        let smtp_username = email_settings
            .smtp_username
            .as_deref()
            .map(str::trim)
            .filter(|username| !username.is_empty());
        let from_email = email_settings
            .from_email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty() && email.contains('@'));

        // Check required fields for SMTP
        let Some(smtp_host) = smtp_host else {
            return Ok(false);
        };

        if smtp_username.is_none() {
            return Ok(false);
        }

        if from_email.is_none() {
            return Ok(false);
        }

        let smtp_port = email_settings.smtp_port.unwrap_or(587);
        if smtp_port == 0 {
            return Ok(false);
        }

        self.smtp_endpoint_reachable(smtp_host, smtp_port).await
    }

    async fn smtp_endpoint_reachable(&self, host: &str, port: u16) -> Result<bool, AppError> {
        match timeout(
            Duration::from_secs(SMTP_CONNECT_TIMEOUT_SECONDS),
            TcpStream::connect((host, port)),
        )
        .await
        {
            Ok(Ok(_stream)) => Ok(true),
            Ok(Err(error)) => {
                warn!(
                    "SMTP configuration check failed to connect to {}:{}: {}",
                    host, port, error
                );
                Ok(false)
            }
            Err(_) => {
                warn!(
                    "SMTP configuration check timed out after {} seconds for {}:{}",
                    SMTP_CONNECT_TIMEOUT_SECONDS, host, port
                );
                Ok(false)
            }
        }
    }

    /// Validate license information
    async fn validate_license(&self, system_info: &SystemInfoSettings) -> Result<bool, AppError> {
        match &system_info.oxidedb_edition {
            oxide_core::site_settings::OxideDbEdition::Community => Ok(true), // No license required
            oxide_core::site_settings::OxideDbEdition::Professional
            | oxide_core::site_settings::OxideDbEdition::Enterprise => {
                let Some(license_key) = system_info.license_key.as_deref() else {
                    return Ok(false);
                };

                verify_signed_license(system_info, license_key)
            }
        }
    }
}

fn verify_signed_license(
    system_info: &SystemInfoSettings,
    license_key: &str,
) -> Result<bool, AppError> {
    let Some(verifying_key) = load_license_public_key()? else {
        warn!("Commercial edition license is configured but no trusted license public key is set");
        return Ok(false);
    };

    verify_signed_license_with_key(system_info, license_key, &verifying_key)
}

fn verify_signed_license_with_key(
    system_info: &SystemInfoSettings,
    license_key: &str,
    verifying_key: &VerifyingKey,
) -> Result<bool, AppError> {
    let document = parse_signed_license_document(license_key)?;
    if document
        .algorithm
        .as_deref()
        .map(|algorithm| !algorithm.eq_ignore_ascii_case("ed25519"))
        .unwrap_or(false)
    {
        warn!("Unsupported license signature algorithm");
        return Ok(false);
    }

    let payload = decode_key_material(&document.payload).map_err(|e| {
        AppError::validation(
            "license_key".to_string(),
            format!("Invalid license payload encoding: {}", e),
        )
    })?;
    let signature_bytes = decode_key_material(&document.signature).map_err(|e| {
        AppError::validation(
            "license_key".to_string(),
            format!("Invalid license signature encoding: {}", e),
        )
    })?;
    let signature: [u8; 64] = signature_bytes.try_into().map_err(|bytes: Vec<u8>| {
        AppError::validation(
            "license_key".to_string(),
            format!(
                "Invalid Ed25519 license signature length: expected 64 bytes, got {}",
                bytes.len()
            ),
        )
    })?;
    let signature = Signature::from_bytes(&signature);

    if verifying_key.verify(&payload, &signature).is_err() {
        warn!("License signature did not verify against the configured public key");
        return Ok(false);
    }

    let claims: LicenseClaims = serde_json::from_slice(&payload).map_err(|e| {
        AppError::validation(
            "license_key".to_string(),
            format!("Invalid license payload JSON: {}", e),
        )
    })?;
    let now = current_unix_timestamp();

    if let Some(not_before) = claims.not_before {
        if now < not_before {
            warn!("License is not valid before {}", not_before);
            return Ok(false);
        }
    }

    if claims.expires_at < now {
        warn!("License expired at {}", claims.expires_at);
        return Ok(false);
    }

    if let Some(configured_expiry) = system_info.license_expires_at {
        if claims.expires_at != configured_expiry {
            warn!(
                "Configured license expiry {} does not match signed license expiry {}",
                configured_expiry, claims.expires_at
            );
            return Ok(false);
        }
    }

    if !license_edition_matches(&system_info.oxidedb_edition, &claims.edition) {
        warn!(
            "License edition '{}' does not match configured edition",
            claims.edition
        );
        return Ok(false);
    }

    if let Some(installation_id) = claims.installation_id.as_deref() {
        if installation_id != system_info.installation_id {
            warn!("License installation_id does not match this installation");
            return Ok(false);
        }
    }

    Ok(true)
}

fn parse_signed_license_document(license_key: &str) -> Result<SignedLicenseDocument, AppError> {
    let trimmed = license_key.trim();

    if let Ok(document) = serde_json::from_str::<SignedLicenseDocument>(trimmed) {
        return Ok(document);
    }

    let decoded = decode_key_material(trimmed).map_err(|e| {
        AppError::validation(
            "license_key".to_string(),
            format!("Invalid license encoding: {}", e),
        )
    })?;

    serde_json::from_slice::<SignedLicenseDocument>(&decoded).map_err(|e| {
        AppError::validation(
            "license_key".to_string(),
            format!("Invalid license JSON document: {}", e),
        )
    })
}

fn load_license_public_key() -> Result<Option<VerifyingKey>, AppError> {
    let material = if let Ok(value) = std::env::var(LICENSE_PUBLIC_KEY_ENV) {
        Some(value)
    } else if let Ok(path) = std::env::var(LICENSE_PUBLIC_KEY_FILE_ENV) {
        Some(std::fs::read_to_string(&path).map_err(|e| {
            AppError::internal(format!(
                "Failed to read license public key file '{}': {}",
                path, e
            ))
        })?)
    } else {
        None
    };

    let Some(material) = material else {
        return Ok(None);
    };
    let Some(material) = extract_key_material(&material) else {
        return Ok(None);
    };

    let key_bytes = decode_key_material(&material).map_err(|e| {
        AppError::validation(
            "license_public_key".to_string(),
            format!("Invalid license public key encoding: {}", e),
        )
    })?;
    let key_bytes: [u8; 32] = key_bytes.try_into().map_err(|bytes: Vec<u8>| {
        AppError::validation(
            "license_public_key".to_string(),
            format!(
                "Invalid Ed25519 license public key length: expected 32 bytes, got {}",
                bytes.len()
            ),
        )
    })?;

    VerifyingKey::from_bytes(&key_bytes)
        .map(Some)
        .map_err(|e| AppError::validation("license_public_key".to_string(), e.to_string()))
}

fn license_edition_matches(
    expected: &oxide_core::site_settings::OxideDbEdition,
    actual: &str,
) -> bool {
    let expected = match expected {
        oxide_core::site_settings::OxideDbEdition::Community => "community",
        oxide_core::site_settings::OxideDbEdition::Professional => "professional",
        oxide_core::site_settings::OxideDbEdition::Enterprise => "enterprise",
    };

    expected.eq_ignore_ascii_case(actual.trim())
}

fn extract_key_material(value: &str) -> Option<String> {
    for line in value.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((name, material)) = trimmed.split_once('=') {
            let name = name.trim().to_ascii_lowercase();
            if matches!(
                name.as_str(),
                "public_key" | "license_key" | "key" | "ed25519"
            ) {
                return Some(material.trim().to_string());
            }
        }

        return Some(trimmed.to_string());
    }

    None
}

fn decode_key_material(material: &str) -> Result<Vec<u8>, String> {
    let mut material = material.trim().trim_matches('"').trim_matches('\'');

    for prefix in ["ed25519:", "base64:", "hex:"] {
        if let Some(stripped) = material.strip_prefix(prefix) {
            material = stripped.trim();
            break;
        }
    }

    if material.len().is_multiple_of(2) && material.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return decode_hex_material(material);
    }

    STANDARD
        .decode(material)
        .or_else(|_| URL_SAFE.decode(material))
        .or_else(|_| URL_SAFE_NO_PAD.decode(material))
        .map_err(|e| e.to_string())
}

fn decode_hex_material(material: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(material.len() / 2);
    let mut chars = material.as_bytes().chunks_exact(2);

    for pair in &mut chars {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        bytes.push((high << 4) | low);
    }

    if !chars.remainder().is_empty() {
        return Err("hex input must contain an even number of digits".to_string());
    }

    Ok(bytes)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("hex input contains a non-hex digit".to_string()),
    }
}

fn current_unix_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
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

        // This validates SMTP reachability without sending credentials or test mail.
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
