//! Plugin Configuration Management
//!
//! This module provides structures and utilities for persisting plugin configurations
//! to the database, with WASM files stored separately on the filesystem.

use crate::{
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits},
    collection::{CollectionSchema, CollectionType, FieldDefinition},
    field_types::FieldType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use ts_rs::TS;

/// Complete plugin configuration that can be persisted to the database
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PluginConfiguration {
    /// Plugin name (unique identifier)
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author
    pub author: String,
    /// Current status of the plugin
    pub status: PluginStatus,
    /// Trust level assigned to the plugin
    pub trust_level: PluginTrustLevel,
    /// Capabilities granted to the plugin
    pub capabilities: Vec<PluginCapability>,
    /// Resource limits for the plugin
    pub resource_limits: ResourceLimits,
    /// Plugin metadata (configuration, settings, etc.)
    #[ts(type = "any")]
    pub metadata: serde_json::Value,
    /// Path to the WASM file on the filesystem (relative to plugins directory)
    pub wasm_path: Option<String>,
    /// Path to the plugin directory (relative to plugins base directory)
    pub plugin_directory: Option<String>,
    /// File size of the WASM binary in bytes
    pub wasm_size: Option<u64>,
    /// SHA256 hash of the WASM file for integrity verification
    pub wasm_hash: Option<String>,
    /// Whether the plugin is enabled/disabled
    pub enabled: bool,
    /// Timestamp when plugin was installed
    pub installed_at: i64,
    /// Timestamp when plugin was last updated
    pub updated_at: i64,
}

/// Plugin status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum PluginStatus {
    /// Plugin is installed and enabled
    Enabled,
    /// Plugin is installed but disabled
    Disabled,
    /// Plugin encountered an error
    Error,
    /// Plugin is currently being loaded
    Loading,
    /// Plugin is being uninstalled
    Uninstalling,
}

impl PluginConfiguration {
    /// Create a new plugin configuration
    pub fn new(
        name: String,
        version: String,
        description: String,
        author: String,
        trust_level: PluginTrustLevel,
        capabilities: Vec<PluginCapability>,
        resource_limits: ResourceLimits,
        wasm_path: Option<String>,
        wasm_size: Option<u64>,
        wasm_hash: Option<String>,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            name,
            version,
            description,
            author,
            status: PluginStatus::Enabled,
            trust_level,
            capabilities,
            resource_limits,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            wasm_path,
            plugin_directory: None,
            wasm_size,
            wasm_hash,
            enabled: true,
            installed_at: now,
            updated_at: now,
        }
    }

    /// Update the plugin configuration
    pub fn update(&mut self) {
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
    }

    /// Enable the plugin
    pub fn enable(&mut self) {
        self.enabled = true;
        self.status = PluginStatus::Enabled;
        self.update();
    }

    /// Disable the plugin
    pub fn disable(&mut self) {
        self.enabled = false;
        self.status = PluginStatus::Disabled;
        self.update();
    }

    /// Set plugin status
    pub fn set_status(&mut self, status: PluginStatus) {
        self.status = status;
        self.update();
    }

    /// Add a capability to the plugin
    pub fn add_capability(&mut self, capability: PluginCapability) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
            self.update();
        }
    }

    /// Remove a capability from the plugin
    pub fn remove_capability(&mut self, capability: &PluginCapability) {
        self.capabilities.retain(|c| c != capability);
        self.update();
    }

    /// Update trust level
    pub fn set_trust_level(&mut self, trust_level: PluginTrustLevel) {
        self.trust_level = trust_level;
        self.update();
    }

    /// Update resource limits
    pub fn set_resource_limits(&mut self, resource_limits: ResourceLimits) {
        self.resource_limits = resource_limits;
        self.update();
    }

    /// Set metadata
    pub fn set_metadata(&mut self, metadata: serde_json::Value) {
        self.metadata = metadata;
        self.update();
    }

    /// Update WASM file information
    pub fn set_wasm_info(&mut self, wasm_path: Option<String>, wasm_size: Option<u64>, wasm_hash: Option<String>) {
        self.wasm_path = wasm_path;
        self.wasm_size = wasm_size;
        self.wasm_hash = wasm_hash;
        self.update();
    }

    /// Set plugin directory path
    pub fn set_plugin_directory(&mut self, plugin_directory: Option<String>) {
        self.plugin_directory = plugin_directory;
        self.update();
    }

    /// Get the full filesystem path to the WASM file
    pub fn get_wasm_file_path(&self, plugins_dir: &PathBuf) -> Option<PathBuf> {
        self.wasm_path.as_ref().map(|path| plugins_dir.join(path))
    }

    /// Check if the plugin has a WASM file
    pub fn has_wasm_file(&self) -> bool {
        self.wasm_path.is_some()
    }
}

/// Create the _plugins system collection schema
pub fn create_plugins_collection_schema() -> CollectionSchema {
    let mut schema = CollectionSchema::new("_plugins".to_string(), CollectionType::Base);
    let mut fields = HashMap::new();

    // Plugin name (unique identifier)
    fields.insert("name".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: true,
        default: None,
        validation: None,
        index: true,
    });

    // Plugin version
    fields.insert("version".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: false,
        default: Some(serde_json::json!("1.0.0")),
        validation: None,
        index: false,
    });

    // Plugin description
    fields.insert("description".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: false,
        unique: false,
        default: None,
        validation: None,
        index: false,
    });

    // Plugin author
    fields.insert("author".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: false,
        unique: false,
        default: Some(serde_json::json!("Unknown")),
        validation: None,
        index: false,
    });

    // Plugin status
    fields.insert("status".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: false,
        default: Some(serde_json::json!("Enabled")),
        validation: None,
        index: false,
    });

    // Trust level
    fields.insert("trust_level".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: true,
        unique: false,
        default: Some(serde_json::json!("Untrusted")),
        validation: None,
        index: false,
    });

    // Capabilities (stored as JSON array)
    fields.insert("capabilities".to_string(), FieldDefinition {
        field_type: FieldType::Json,
        required: false,
        unique: false,
        default: Some(serde_json::json!([])),
        validation: None,
        index: false,
    });

    // Resource limits (stored as JSON object)
    fields.insert("resource_limits".to_string(), FieldDefinition {
        field_type: FieldType::Json,
        required: false,
        unique: false,
        default: None,
        validation: None,
        index: false,
    });

    // Plugin metadata
    fields.insert("metadata".to_string(), FieldDefinition {
        field_type: FieldType::Json,
        required: false,
        unique: false,
        default: Some(serde_json::json!({})),
        validation: None,
        index: false,
    });

    // WASM file path (relative to plugins directory)
    fields.insert("wasm_path".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: false,
        unique: false,
        default: None,
        validation: None,
        index: false,
    });

    // Plugin directory path (relative to plugins base directory)
    fields.insert("plugin_directory".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: false,
        unique: false,
        default: None,
        validation: None,
        index: false,
    });

    // WASM file size in bytes
    fields.insert("wasm_size".to_string(), FieldDefinition {
        field_type: FieldType::Number,
        required: false,
        unique: false,
        default: None,
        validation: None,
        index: false,
    });

    // WASM file hash for integrity verification
    fields.insert("wasm_hash".to_string(), FieldDefinition {
        field_type: FieldType::Text,
        required: false,
        unique: false,
        default: None,
        validation: None,
        index: false,
    });

    // Whether plugin is enabled
    fields.insert("enabled".to_string(), FieldDefinition {
        field_type: FieldType::Boolean,
        required: true,
        unique: false,
        default: Some(serde_json::json!(true)),
        validation: None,
        index: false,
    });

    // Installation timestamp
    fields.insert("installed_at".to_string(), FieldDefinition {
        field_type: FieldType::Number,
        required: true,
        unique: false,
        default: None,
        validation: None,
        index: false,
    });

    schema.fields = fields;
    schema
}

/// Convert a PluginConfiguration to a JSON record for database storage
pub fn plugin_config_to_record(config: &PluginConfiguration) -> Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(config)
}

/// Convert a JSON record from database to PluginConfiguration
pub fn record_to_plugin_config(record: &serde_json::Value) -> Result<PluginConfiguration, serde_json::Error> {
    use tracing::{warn, debug};
    
    debug!("Converting database record to plugin config: {}", record);
    
    // Handle comprehensive field mapping and type conversion
    let mut record = record.clone();
    if let Some(obj) = record.as_object_mut() {
        // Fix empty string fields that should be JSON arrays/objects
        if let Some(capabilities) = obj.get("capabilities") {
            if capabilities.is_string() && capabilities.as_str() == Some("") {
                obj.insert("capabilities".to_string(), serde_json::json!([]));
            }
        } else {
            // Add missing capabilities field with default empty array
            obj.insert("capabilities".to_string(), serde_json::json!([]));
        }
        
        if let Some(resource_limits) = obj.get("resource_limits") {
            if resource_limits.is_string() && resource_limits.as_str() == Some("") {
                obj.insert("resource_limits".to_string(), serde_json::to_value(crate::plugin_security::ResourceLimits::default()).unwrap());
            }
        } else {
            // Add missing resource_limits field with defaults
            obj.insert("resource_limits".to_string(), serde_json::to_value(crate::plugin_security::ResourceLimits::default()).unwrap());
        }
        
        if let Some(metadata) = obj.get("metadata") {
            if metadata.is_string() && metadata.as_str() == Some("") {
                obj.insert("metadata".to_string(), serde_json::json!({}));
            }
        } else {
            // Add missing metadata field with empty object
            obj.insert("metadata".to_string(), serde_json::json!({}));
        }
        
        // Fix floating point numbers that should be integers
        if let Some(installed_at) = obj.get("installed_at") {
            if let Some(f) = installed_at.as_f64() {
                obj.insert("installed_at".to_string(), serde_json::json!(f as i64));
            }
        }
        
        // Handle updated_at field - might be stored as created_at or missing
        if let Some(updated_at) = obj.get("updated_at") {
            if let Some(f) = updated_at.as_f64() {
                obj.insert("updated_at".to_string(), serde_json::json!(f as i64));
            } else if let Some(i) = updated_at.as_i64() {
                obj.insert("updated_at".to_string(), serde_json::json!(i));
            }
        } else if let Some(created_at) = obj.get("created_at") {
            // Use created_at as fallback for updated_at
            if let Some(f) = created_at.as_f64() {
                obj.insert("updated_at".to_string(), serde_json::json!(f as i64));
            } else if let Some(i) = created_at.as_i64() {
                obj.insert("updated_at".to_string(), serde_json::json!(i));
            }
        } else {
            // Add current timestamp as fallback
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            obj.insert("updated_at".to_string(), serde_json::json!(now));
        }
        
        if let Some(wasm_size) = obj.get("wasm_size") {
            if let Some(f) = wasm_size.as_f64() {
                obj.insert("wasm_size".to_string(), serde_json::json!(f as u64));
            }
        }
        
        // Fix boolean fields that might be stored as integers
        if let Some(enabled) = obj.get("enabled") {
            if let Some(i) = enabled.as_i64() {
                obj.insert("enabled".to_string(), serde_json::json!(i != 0));
            }
        }
        
        // Handle missing optional fields
        if !obj.contains_key("description") {
            obj.insert("description".to_string(), serde_json::json!(""));
        }
        
        if !obj.contains_key("author") {
            obj.insert("author".to_string(), serde_json::json!("Unknown"));
        }
        
        if !obj.contains_key("version") {
            obj.insert("version".to_string(), serde_json::json!("1.0.0"));
        }
        
        if !obj.contains_key("status") {
            obj.insert("status".to_string(), serde_json::json!("Enabled"));
        }
        
        if !obj.contains_key("trust_level") {
            obj.insert("trust_level".to_string(), serde_json::json!("Untrusted"));
        }
        
        if !obj.contains_key("enabled") {
            obj.insert("enabled".to_string(), serde_json::json!(true));
        }
        
        if !obj.contains_key("installed_at") {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            obj.insert("installed_at".to_string(), serde_json::json!(now));
        }
    }
    
    debug!("Converted record for deserialization: {}", record);
    
    serde_json::from_value(record).map_err(|e| {
        warn!("Failed to deserialize plugin config: {}", e);
        e
    })
}

/// Utility functions for plugin file management
pub mod filesystem {
    use super::*;
    use std::fs;
    use std::io::{self, Write};
    use std::path::Path;
    use sha2::{Sha256, Digest};

    /// Save WASM data to filesystem and return the path, size, and hash
    pub fn save_wasm_file(
        plugins_dir: &PathBuf,
        plugin_name: &str,
        version: &str,
        wasm_data: &[u8],
    ) -> io::Result<(String, u64, String)> {
        // Create plugins directory if it doesn't exist
        fs::create_dir_all(plugins_dir)?;

        // Generate filename: {plugin_name}-{version}.wasm
        let filename = format!("{}-{}.wasm", plugin_name, version);
        let file_path = plugins_dir.join(&filename);

        // Write the WASM data to file
        let mut file = fs::File::create(&file_path)?;
        file.write_all(wasm_data)?;

        // Calculate file size
        let size = wasm_data.len() as u64;

        // Calculate SHA256 hash
        let mut hasher = Sha256::new();
        hasher.update(wasm_data);
        let hash = format!("{:x}", hasher.finalize());

        Ok((filename, size, hash))
    }

    /// Load WASM data from filesystem
    pub fn load_wasm_file(plugins_dir: &PathBuf, wasm_path: &str) -> io::Result<Vec<u8>> {
        let file_path = plugins_dir.join(wasm_path);
        fs::read(file_path)
    }

    /// Delete WASM file from filesystem
    pub fn delete_wasm_file(plugins_dir: &PathBuf, wasm_path: &str) -> io::Result<()> {
        let file_path = plugins_dir.join(wasm_path);
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
    }

    /// Verify WASM file integrity using hash
    pub fn verify_wasm_file(plugins_dir: &PathBuf, wasm_path: &str, expected_hash: &str) -> io::Result<bool> {
        let wasm_data = load_wasm_file(plugins_dir, wasm_path)?;
        
        let mut hasher = Sha256::new();
        hasher.update(&wasm_data);
        let actual_hash = format!("{:x}", hasher.finalize());

        Ok(actual_hash == expected_hash)
    }

    /// List all WASM files in the plugins directory
    pub fn list_wasm_files(plugins_dir: &PathBuf) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        
        if plugins_dir.exists() {
            for entry in fs::read_dir(plugins_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_file() {
                    if let Some(extension) = path.extension() {
                        if extension == "wasm" {
                            if let Some(filename) = path.file_name() {
                                if let Some(filename_str) = filename.to_str() {
                                    files.push(filename_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Ok(files)
    }

    /// Create a plugin directory for a specific plugin version
    pub fn create_plugin_directory(
        plugins_dir: &PathBuf,
        plugin_name: &str,
        version: &str,
    ) -> io::Result<PathBuf> {
        // Create base plugins directory if it doesn't exist
        fs::create_dir_all(plugins_dir)?;

        // Create plugin-specific directory: {plugin_name}-{version}
        let plugin_dir_name = format!("{}-{}", plugin_name, version);
        let plugin_dir = plugins_dir.join(&plugin_dir_name);
        
        // Create the plugin directory
        fs::create_dir_all(&plugin_dir)?;
        
        Ok(plugin_dir)
    }

    /// Remove a plugin directory and all its contents
    pub fn remove_plugin_directory(
        plugins_dir: &PathBuf,
        plugin_name: &str,
        version: &str,
    ) -> io::Result<()> {
        let plugin_dir_name = format!("{}-{}", plugin_name, version);
        let plugin_dir = plugins_dir.join(&plugin_dir_name);
        
        if plugin_dir.exists() {
            fs::remove_dir_all(plugin_dir)?;
        }
        
        Ok(())
    }

    /// Get the path to a plugin directory
    pub fn get_plugin_directory(
        plugins_dir: &PathBuf,
        plugin_name: &str,
        version: &str,
    ) -> PathBuf {
        let plugin_dir_name = format!("{}-{}", plugin_name, version);
        plugins_dir.join(&plugin_dir_name)
    }

    /// Load WASM data from a plugin directory
    pub fn load_wasm_from_plugin_dir(
        plugin_dir: &Path,
        wasm_filename: &str,
    ) -> io::Result<Vec<u8>> {
        let wasm_path = plugin_dir.join(wasm_filename);
        fs::read(wasm_path)
    }

    /// Load manifest (plugin.toml) from a plugin directory
    pub fn load_manifest_from_plugin_dir(plugin_dir: &Path) -> io::Result<String> {
        let manifest_path = plugin_dir.join("plugin.toml");
        fs::read_to_string(manifest_path)
    }

    /// Check if a plugin directory exists
    pub fn plugin_directory_exists(
        plugins_dir: &PathBuf,
        plugin_name: &str,
        version: &str,
    ) -> bool {
        let plugin_dir = get_plugin_directory(plugins_dir, plugin_name, version);
        plugin_dir.exists() && plugin_dir.is_dir()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_plugin_configuration_creation() {
        let config = PluginConfiguration::new(
            "test_plugin".to_string(),
            "1.0.0".to_string(),
            "Test plugin".to_string(),
            "Test Author".to_string(),
            PluginTrustLevel::Untrusted,
            vec![PluginCapability::LogInfo],
            ResourceLimits::default(),
            Some("test_plugin-1.0.0.wasm".to_string()),
            Some(1024),
            Some("abc123".to_string()),
        );

        assert_eq!(config.name, "test_plugin");
        assert_eq!(config.version, "1.0.0");
        assert_eq!(config.enabled, true);
        assert!(config.has_wasm_file());
    }

    #[test]
    fn test_plugin_configuration_capabilities() {
        let mut config = PluginConfiguration::new(
            "test_plugin".to_string(),
            "1.0.0".to_string(),
            "Test plugin".to_string(),
            "Test Author".to_string(),
            PluginTrustLevel::Untrusted,
            vec![],
            ResourceLimits::default(),
            None,
            None,
            None,
        );

        // Add capability
        config.add_capability(PluginCapability::LogInfo);
        assert!(config.capabilities.contains(&PluginCapability::LogInfo));

        // Remove capability
        config.remove_capability(&PluginCapability::LogInfo);
        assert!(!config.capabilities.contains(&PluginCapability::LogInfo));
    }

    #[test]
    fn test_plugins_collection_schema() {
        let schema = create_plugins_collection_schema();
        assert_eq!(schema.name, "_plugins");
        assert_eq!(schema.collection_type, CollectionType::Base);
        assert!(schema.fields.contains_key("name"));
        assert!(schema.fields.contains_key("trust_level"));
        assert!(schema.fields.contains_key("capabilities"));
        assert!(schema.fields.contains_key("wasm_path"));
        assert!(schema.fields.contains_key("wasm_hash"));
    }

    #[test]
    fn test_filesystem_operations() {
        let temp_dir = TempDir::new().unwrap();
        let plugins_dir = temp_dir.path().to_path_buf();
        let wasm_data = b"test wasm data";

        // Save WASM file
        let (filename, size, hash) = filesystem::save_wasm_file(
            &plugins_dir,
            "test_plugin",
            "1.0.0",
            wasm_data,
        ).unwrap();

        assert_eq!(filename, "test_plugin-1.0.0.wasm");
        assert_eq!(size, wasm_data.len() as u64);
        assert!(!hash.is_empty());

        // Load WASM file
        let loaded_data = filesystem::load_wasm_file(&plugins_dir, &filename).unwrap();
        assert_eq!(loaded_data, wasm_data);

        // Verify WASM file
        let is_valid = filesystem::verify_wasm_file(&plugins_dir, &filename, &hash).unwrap();
        assert!(is_valid);

        // List WASM files
        let files = filesystem::list_wasm_files(&plugins_dir).unwrap();
        assert!(files.contains(&filename));

        // Delete WASM file
        filesystem::delete_wasm_file(&plugins_dir, &filename).unwrap();
        let files_after_delete = filesystem::list_wasm_files(&plugins_dir).unwrap();
        assert!(!files_after_delete.contains(&filename));
    }
} 