//! Plugin handler types and data structures

use oxide_core::{
    auth::CollectionPermissions,
    field_types::{FieldType, ValidationRules},
    plugin_security::{PluginCapability, PluginTrustLevel},
    FieldDefinition,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// TOML-based plugin metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin metadata section
    pub plugin: PluginMetadata,
    /// Security configuration
    pub security: PluginSecurity,
    /// Admin UI pages contributed by this plugin
    pub admin: Option<PluginAdmin>,
    /// Dependencies on other plugins or system components
    pub dependencies: Option<PluginDependencies>,
    /// Plugin configuration schema
    pub config: Option<PluginConfigSchema>,
}

/// Core plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (unique identifier)
    pub name: String,
    /// Plugin version (semantic versioning)
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author(s)
    pub author: String,
    /// Plugin homepage or repository URL
    pub homepage: Option<String>,
    /// Plugin license identifier (e.g., MIT, Apache-2.0)
    pub license: Option<String>,
    /// Minimum required OxideDB version
    pub min_oxide_version: Option<String>,
    /// Plugin keywords/tags for discovery
    pub keywords: Option<Vec<String>>,
    /// Plugin categories for organization
    pub categories: Option<Vec<String>>,
    /// Changelog or release notes
    pub changelog: Option<String>,
    /// Build timestamp (ISO 8601 format)
    pub build_timestamp: String,
    /// Main WASM file name within the package
    pub wasm_file: String,
}

/// Security configuration for the plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSecurity {
    /// Required capabilities for plugin operation
    pub required_capabilities: Vec<String>,
    /// Recommended trust level for this plugin
    pub recommended_trust_level: String,
    /// Digital signature for verification (file path or embedded)
    pub signature: Option<String>,
    /// Certificate chain for verification
    pub certificate_chain: Option<Vec<String>>,
    /// Security contact information
    pub security_contact: Option<String>,
    /// Known security vulnerabilities or advisories
    pub security_advisories: Option<Vec<String>>,
    /// Audit information
    pub audit_info: Option<PluginAuditInfo>,
}

/// Admin UI contributions declared by a plugin package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAdmin {
    /// Pages that should be mounted inside the OxideDB admin console.
    #[serde(default)]
    pub pages: Vec<PluginAdminPage>,
    /// Record form fields that should be added to collection admin pages.
    #[serde(default)]
    pub record_fields: Vec<PluginAdminRecordField>,
}

/// A single admin page served from packaged plugin assets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAdminPage {
    /// Stable URL slug for the page within the plugin's admin namespace.
    pub slug: String,
    /// Human-readable page title shown in navigation and headers.
    pub title: String,
    /// Entry HTML file under the package's admin asset directory.
    pub entry: String,
    /// Optional short description shown by the host admin shell.
    pub description: Option<String>,
    /// Optional icon identifier understood by the admin shell.
    pub icon: Option<String>,
    /// Optional navigation group label for organizing plugin pages.
    pub nav_group: Option<String>,
}

/// A schema-backed field contributed to the admin record form by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAdminRecordField {
    /// Collection name this field applies to, or `*` for all user collections.
    pub collection: String,
    /// Field name to add to the collection form.
    pub name: String,
    /// The type of this field.
    pub field_type: FieldType,
    /// Whether this field is required.
    #[serde(default)]
    pub required: bool,
    /// Whether this field must be unique.
    #[serde(default)]
    pub unique: bool,
    /// Whether this field should be indexed for performance.
    #[serde(default)]
    pub index: bool,
    /// Default value for this field.
    #[serde(default)]
    pub default: Option<serde_json::Value>,
    /// Validation rules for this field.
    #[serde(default)]
    pub validation: Option<ValidationRules>,
}

impl PluginAdminRecordField {
    /// Convert this manifest declaration into a collection field definition.
    pub fn definition(&self) -> FieldDefinition {
        FieldDefinition {
            field_type: self.field_type.clone(),
            required: self.required,
            unique: self.unique,
            index: self.index,
            default: self.default.clone(),
            validation: self.validation.clone(),
        }
    }
}

/// Plugin dependencies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDependencies {
    /// Dependencies on other plugins
    pub plugins: Option<HashMap<String, String>>,
    /// System requirements
    pub system: Option<PluginSystemRequirements>,
}

/// System requirements for the plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSystemRequirements {
    /// Minimum memory requirement in MB
    pub min_memory_mb: Option<u64>,
    /// Required CPU features
    pub cpu_features: Option<Vec<String>>,
    /// Required system libraries
    pub system_libs: Option<Vec<String>>,
}

/// Plugin configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfigSchema {
    /// JSON schema for plugin configuration
    pub schema: serde_json::Value,
    /// Default configuration values
    pub defaults: Option<serde_json::Value>,
}

/// Audit information for the plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuditInfo {
    /// Audit date (ISO 8601 format)
    pub audit_date: String,
    /// Auditor name or organization
    pub auditor: String,
    /// Audit report URL or identifier
    pub report_url: Option<String>,
    /// Audit status (e.g., "passed", "failed", "pending")
    pub status: String,
}

/// Result of extracting and validating a plugin package
#[derive(Debug, Clone)]
pub struct PluginPackage {
    /// Plugin manifest from TOML
    pub manifest: PluginManifest,
    /// WASM binary data
    pub wasm_data: Vec<u8>,
    /// Digital signature data (if present)
    pub signature_data: Option<Vec<u8>>,
    /// Canonical bytes covered by the detached package signature
    pub signature_payload: Vec<u8>,
    /// Package hash for integrity verification
    pub package_hash: String,
    /// Size of the original package
    pub package_size: u64,
    /// Path where the plugin package was extracted
    pub extraction_path: std::path::PathBuf,
}

/// Runtime-safe information for an admin page contributed by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAdminPageInfo {
    /// Plugin that owns the page.
    pub plugin_name: String,
    /// Plugin version that supplied the page.
    pub plugin_version: String,
    /// Stable page slug from the plugin manifest.
    pub slug: String,
    /// Human-readable page title.
    pub title: String,
    /// Optional short page description.
    pub description: Option<String>,
    /// Optional icon identifier for the admin shell.
    pub icon: Option<String>,
    /// Navigation group label for this page.
    pub nav_group: String,
    /// URL path in the OxideDB admin where the page is mounted.
    pub admin_path: String,
    /// URL to the packaged page entry HTML.
    pub source_url: String,
    /// Whether the owning plugin is currently enabled.
    pub enabled: bool,
}

/// Runtime-safe information for a record form field contributed by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAdminRecordFieldInfo {
    /// Plugin that owns the field contribution.
    pub plugin_name: String,
    /// Plugin version that supplied the field contribution.
    pub plugin_version: String,
    /// Collection target from the plugin manifest.
    pub collection: String,
    /// Field name to add to the collection form.
    pub field_name: String,
    /// Schema-compatible field definition.
    pub field: FieldDefinition,
    /// Whether the owning plugin is currently enabled.
    pub enabled: bool,
}

/// Information about a plugin route including permissions
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PluginRouteInfo {
    pub plugin_name: String,
    pub method: String,
    pub path: String,
    pub handler_function: String,
    pub permissions: Option<CollectionPermissions>,
    pub has_custom_permissions: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub status: PluginStatus,
    pub version: String,
    pub description: String,
    pub author: String,
    pub capabilities: Vec<PluginCapability>,
    pub trust_level: PluginTrustLevel,
    pub routes: Vec<PluginRouteInfo>,
    pub admin_pages: Vec<PluginAdminPageInfo>,
    pub executions: u64,
    pub errors: u64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
    pub resource_usage: ResourceUsageInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginDetails {
    pub name: String,
    pub status: PluginStatus,
    pub version: String,
    pub description: String,
    pub author: String,
    pub capabilities: Vec<PluginCapability>,
    pub trust_level: PluginTrustLevel,
    pub routes: Vec<PluginRouteInfo>,
    pub admin_pages: Vec<PluginAdminPageInfo>,
    pub executions: u64,
    pub errors: u64,
    pub last_execution: Option<chrono::DateTime<chrono::Utc>>,
    pub resource_usage: ResourceUsageInfo,
    pub audit_log: Vec<oxide_core::plugin_security::SecurityAuditEntry>,
    pub permissions: Option<CollectionPermissions>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PluginStatus {
    Enabled,
    Disabled,
    Error,
    Loading,
    Uninstalling,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ResourceUsageInfo {
    pub memory_bytes: u64,
    pub cpu_time_ms: u64,
    pub api_calls: u64,
    pub storage_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateTrustLevelRequest {
    pub trust_level: PluginTrustLevel,
}

/// Result of extracting and validating a plugin package
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginAnalysisResult {
    pub is_valid: bool,
    pub plugin_info: Option<PluginInfo>,
    pub declared_capabilities: Vec<String>,
    pub recommended_trust_level: Option<String>,
    pub security_info: PluginSecurityInfo,
    pub size_bytes: u64,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginSecurityInfo {
    pub binary_hash: String,
    pub hash_algorithm: String,
    pub signature_valid: bool,
    pub security_advisories: Vec<String>,
    pub audit_info: Option<oxide_plugin_sdk::PluginAuditInfo>,
}

/// Result returned after a plugin package is installed.
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginInstallResult {
    /// Installed plugin information.
    pub plugin_info: PluginInfo,
    /// Trust level applied from the package manifest after policy validation.
    pub applied_trust_level: PluginTrustLevel,
    /// Capabilities applied from the package manifest.
    pub applied_capabilities: Vec<PluginCapability>,
    /// Raw capability declarations read from the package manifest.
    pub declared_capabilities: Vec<String>,
    /// Security information calculated during package validation.
    pub security_info: PluginSecurityInfo,
    /// Package size in bytes.
    pub size_bytes: u64,
    /// Notices the user should review after automatic installation.
    pub notices: Vec<PluginInstallNotice>,
}

/// A notice produced during automatic plugin installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInstallNotice {
    /// Severity of the notice.
    pub severity: PluginInstallNoticeSeverity,
    /// Human-readable notice text.
    pub message: String,
}

/// Severity for an automatic plugin installation notice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginInstallNoticeSeverity {
    /// Informational notice.
    Info,
    /// Warning that should be reviewed by the installer.
    Warning,
}
