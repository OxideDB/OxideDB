//! Site Settings Management
//!
//! This module provides types and traits for managing system-wide site settings
//! including branding, SMTP configuration, version information, and other
//! application-level preferences that are shared across all users.

use crate::AppError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Site settings data structure containing all system-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteSettings {
    /// Site branding and appearance settings
    pub branding: BrandingSettings,
    /// Email and SMTP configuration
    pub email: EmailSettings,
    /// System version and edition information
    pub system_info: SystemInfoSettings,
    /// General application settings
    pub general: GeneralSettings,
    /// Security and authentication settings
    pub security: SecuritySettings,
    /// When the settings were created
    pub created_at: i64,
    /// When the settings were last updated
    pub updated_at: i64,
}

/// Site branding and appearance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandingSettings {
    /// Site title displayed in browser and UI
    pub site_title: String,
    /// Site description or tagline
    pub site_description: Option<String>,
    /// Logo URL or base64 encoded image data
    pub logo_url: Option<String>,
    /// Favicon URL or base64 encoded icon data
    pub favicon_url: Option<String>,
    /// Primary brand color (hex format)
    pub primary_color: Option<String>,
    /// Secondary brand color (hex format)
    pub secondary_color: Option<String>,
    /// Custom CSS for additional styling
    pub custom_css: Option<String>,
    /// Footer text or HTML
    pub footer_text: Option<String>,
}

/// Email and SMTP configuration for system notifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailSettings {
    /// Enable email functionality
    pub enabled: bool,
    /// SMTP server hostname
    pub smtp_host: Option<String>,
    /// SMTP server port
    pub smtp_port: Option<u16>,
    /// SMTP username
    pub smtp_username: Option<String>,
    /// SMTP password (encrypted in storage)
    pub smtp_password: Option<String>,
    /// Use TLS encryption
    pub smtp_tls: bool,
    /// Use STARTTLS
    pub smtp_starttls: bool,
    /// From email address for system emails
    pub from_email: Option<String>,
    /// From name for system emails
    pub from_name: Option<String>,
    /// Reply-to email address
    pub reply_to_email: Option<String>,
    /// Email template settings
    pub templates: EmailTemplateSettings,
}

/// Email template configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailTemplateSettings {
    /// Email verification template
    pub verification_template: Option<String>,
    /// Password reset template
    pub password_reset_template: Option<String>,
    /// Welcome email template
    pub welcome_template: Option<String>,
    /// Custom email signature
    pub signature: Option<String>,
}

/// System version and edition information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoSettings {
    /// OxideDB version (read-only, auto-populated)
    pub oxidedb_version: String,
    /// OxideDB edition (Community, Professional, Enterprise)
    pub oxidedb_edition: OxideDbEdition,
    /// Installation ID (unique identifier for this instance)
    pub installation_id: String,
    /// Deployment environment (development, staging, production)
    pub environment: DeploymentEnvironment,
    /// Custom instance name or identifier
    pub instance_name: Option<String>,
    /// License key (for commercial editions)
    pub license_key: Option<String>,
    /// License expiration date
    pub license_expires_at: Option<i64>,
}

/// General application settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    /// Default timezone for the application
    pub default_timezone: String,
    /// Default language/locale
    pub default_locale: String,
    /// Maximum file upload size in bytes
    pub max_upload_size: u64,
    /// Enable public API access
    pub allow_public_api: bool,
    /// Maximum API requests per minute per user
    pub api_rate_limit: u32,
    /// Maintenance mode settings
    pub maintenance: MaintenanceSettings,
    /// Backup and retention settings
    pub backup: BackupSettings,
}

/// Security and authentication settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecuritySettings {
    /// Password minimum length
    pub password_min_length: u8,
    /// Require password complexity
    pub password_require_complexity: bool,
    /// Session timeout in minutes
    pub session_timeout_minutes: u32,
    /// Maximum login attempts before lockout
    pub max_login_attempts: u8,
    /// Account lockout duration in minutes
    pub lockout_duration_minutes: u32,
    /// Enable two-factor authentication
    pub enable_2fa: bool,
    /// Force 2FA for admin users
    pub force_2fa_admin: bool,
    /// Enable security audit logging
    pub enable_audit_logging: bool,
}

/// Maintenance mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceSettings {
    /// Enable maintenance mode
    pub enabled: bool,
    /// Maintenance message displayed to users
    pub message: Option<String>,
    /// Estimated completion time
    pub estimated_completion: Option<i64>,
    /// Allow admin access during maintenance
    pub allow_admin_access: bool,
}

/// Backup configuration settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSettings {
    /// Enable automatic backups
    pub enabled: bool,
    /// Backup frequency in hours
    pub frequency_hours: u32,
    /// Number of backups to retain
    pub retention_count: u32,
    /// Backup storage location
    pub storage_location: Option<String>,
    /// Enable backup compression
    pub enable_compression: bool,
    /// Include user data in backups
    pub include_user_data: bool,
}

/// OxideDB edition variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OxideDbEdition {
    /// Community edition (free, open source)
    Community,
    /// Professional edition (commercial, additional features)
    Professional,
    /// Enterprise edition (commercial, full feature set)
    Enterprise,
}

/// Deployment environment classification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeploymentEnvironment {
    /// Development environment
    Development,
    /// Staging/testing environment
    Staging,
    /// Production environment
    Production,
}

/// Request to update site settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSiteSettingsRequest {
    /// Branding settings to update
    pub branding: Option<BrandingSettings>,
    /// Email settings to update
    pub email: Option<EmailSettings>,
    /// System info to update (limited fields)
    pub system_info: Option<SystemInfoUpdateRequest>,
    /// General settings to update
    pub general: Option<GeneralSettings>,
    /// Security settings to update
    pub security: Option<SecuritySettings>,
}

/// Limited system info update request (read-only fields excluded)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfoUpdateRequest {
    /// OxideDB edition
    pub oxidedb_edition: Option<OxideDbEdition>,
    /// Deployment environment
    pub environment: Option<DeploymentEnvironment>,
    /// Custom instance name
    pub instance_name: Option<String>,
    /// License key
    pub license_key: Option<String>,
}

/// Response for site settings operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteSettingsResponse {
    /// Whether the operation was successful
    pub success: bool,
    /// Optional message
    pub message: Option<String>,
    /// The settings data (for get operations)
    pub settings: Option<SiteSettings>,
}

/// Trait for site settings service
#[async_trait]
pub trait SiteSettingsService {
    /// Get current site settings
    async fn get_site_settings(&self) -> Result<SiteSettings, AppError>;

    /// Update site settings (partial update)
    async fn update_site_settings(
        &self,
        request: UpdateSiteSettingsRequest,
    ) -> Result<(), AppError>;

    /// Reset site settings to defaults
    async fn reset_site_settings(&self) -> Result<(), AppError>;

    /// Get a specific settings section
    async fn get_settings_section(&self, section: &str) -> Result<Value, AppError>;

    /// Update a specific settings section
    async fn update_settings_section(&self, section: &str, data: &Value) -> Result<(), AppError>;

    /// Validate email configuration by sending a test email
    async fn test_email_configuration(&self) -> Result<bool, AppError>;

    /// Get system health including settings validation
    async fn get_settings_health(&self) -> Result<SettingsHealthStatus, AppError>;
}

/// Settings health status for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsHealthStatus {
    /// Overall settings health
    pub healthy: bool,
    /// Email configuration status
    pub email_config_valid: bool,
    /// License status (for commercial editions)
    pub license_valid: bool,
    /// Configuration warnings or issues
    pub warnings: Vec<String>,
    /// Last settings validation time
    pub last_validated_at: i64,
}

impl Default for SiteSettings {
    fn default() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            branding: BrandingSettings::default(),
            email: EmailSettings::default(),
            system_info: SystemInfoSettings::default(),
            general: GeneralSettings::default(),
            security: SecuritySettings::default(),
            created_at: now,
            updated_at: now,
        }
    }
}

impl Default for BrandingSettings {
    fn default() -> Self {
        Self {
            site_title: "OxideDB".to_string(),
            site_description: Some("A hook-first database with plugin architecture".to_string()),
            logo_url: None,
            favicon_url: None,
            primary_color: Some("#2563eb".to_string()), // Blue
            secondary_color: Some("#64748b".to_string()), // Slate
            custom_css: None,
            footer_text: Some("Powered by OxideDB".to_string()),
        }
    }
}

impl Default for EmailSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp_host: None,
            smtp_port: Some(587),
            smtp_username: None,
            smtp_password: None,
            smtp_tls: true,
            smtp_starttls: true,
            from_email: None,
            from_name: Some("OxideDB".to_string()),
            reply_to_email: None,
            templates: EmailTemplateSettings::default(),
        }
    }
}

impl Default for EmailTemplateSettings {
    fn default() -> Self {
        Self {
            verification_template: None,
            password_reset_template: None,
            welcome_template: None,
            signature: Some("Best regards,\nThe OxideDB Team".to_string()),
        }
    }
}

impl Default for SystemInfoSettings {
    fn default() -> Self {
        use uuid::Uuid;

        Self {
            oxidedb_version: env!("CARGO_PKG_VERSION").to_string(),
            oxidedb_edition: OxideDbEdition::Community,
            installation_id: Uuid::new_v4().to_string(),
            environment: DeploymentEnvironment::Development,
            instance_name: None,
            license_key: None,
            license_expires_at: None,
        }
    }
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            default_timezone: "UTC".to_string(),
            default_locale: "en-US".to_string(),
            max_upload_size: 10 * 1024 * 1024, // 10MB
            allow_public_api: false,
            api_rate_limit: 1000, // requests per minute
            maintenance: MaintenanceSettings::default(),
            backup: BackupSettings::default(),
        }
    }
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            password_min_length: 8,
            password_require_complexity: true,
            session_timeout_minutes: 480, // 8 hours
            max_login_attempts: 5,
            lockout_duration_minutes: 15,
            enable_2fa: false,
            force_2fa_admin: false,
            enable_audit_logging: true,
        }
    }
}

impl Default for MaintenanceSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            message: Some("System maintenance in progress. Please try again later.".to_string()),
            estimated_completion: None,
            allow_admin_access: true,
        }
    }
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            frequency_hours: 24, // Daily
            retention_count: 7,  // Keep 7 backups
            storage_location: None,
            enable_compression: true,
            include_user_data: true,
        }
    }
}

/// Common settings section keys used throughout the system
pub mod settings_sections {
    /// Branding and appearance settings
    pub const BRANDING: &str = "branding";
    /// Email and SMTP configuration
    pub const EMAIL: &str = "email";
    /// System information
    pub const SYSTEM_INFO: &str = "system_info";
    /// General application settings
    pub const GENERAL: &str = "general";
    /// Security settings
    pub const SECURITY: &str = "security";
}
