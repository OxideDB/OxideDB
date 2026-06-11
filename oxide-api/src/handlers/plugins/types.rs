//! Plugin handler types and data structures

use oxide_core::{
    auth::CollectionPermissions,
    plugin_security::{PluginCapability, PluginTrustLevel},
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
