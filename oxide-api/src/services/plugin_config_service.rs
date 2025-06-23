//! Plugin Configuration Service
//!
//! This service provides database operations for plugin configurations,
//! with WASM files stored on the filesystem for better performance and scalability.

use oxide_core::{
    AppError,
    plugin_config::{
        PluginConfiguration, PluginStatus, plugin_config_to_record, record_to_plugin_config,
        filesystem as plugin_fs
    },
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits},
};
use oxide_db::{Db, db::ListParams};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn, error};

/// Service for managing plugin configurations in the database and filesystem
pub struct PluginConfigService {
    db: Arc<dyn Db>,
    plugins_dir: PathBuf,
}

impl PluginConfigService {
    /// Create a new plugin configuration service
    pub fn new(db: Arc<dyn Db>, plugins_dir: PathBuf) -> Self {
        Self { db, plugins_dir }
    }

    /// Install a new plugin with WASM data
    pub async fn install_plugin(
        &self,
        name: String,
        version: String,
        description: String,
        author: String,
        trust_level: PluginTrustLevel,
        capabilities: Vec<PluginCapability>,
        resource_limits: ResourceLimits,
        wasm_data: Vec<u8>,
        metadata: Option<serde_json::Value>,
    ) -> Result<String, AppError> {
        debug!("Installing plugin: {} v{}", name, version);

        // Check if plugin already exists
        if self.plugin_exists(&name).await? {
            return Err(AppError::conflict(format!("Plugin '{}' already exists", name)));
        }

        // Save WASM file to filesystem
        let (wasm_path, wasm_size, wasm_hash) = tokio::task::spawn_blocking({
            let plugins_dir = self.plugins_dir.clone();
            let name = name.clone();
            let version = version.clone();
            move || plugin_fs::save_wasm_file(&plugins_dir, &name, &version, &wasm_data)
        })
        .await
        .map_err(|e| AppError::internal(format!("Failed to spawn blocking task: {}", e)))?
        .map_err(|e| AppError::internal(format!("Failed to save WASM file: {}", e)))?;

        // Create plugin configuration
        let mut config = PluginConfiguration::new(
            name,
            version,
            description,
            author,
            trust_level,
            capabilities,
            resource_limits,
            Some(wasm_path),
            Some(wasm_size),
            Some(wasm_hash),
        );

        if let Some(metadata) = metadata {
            config.set_metadata(metadata);
        }

        // Save to database
        let record_id = self.save_plugin_config(&config).await?;

        info!("✅ Successfully installed plugin: {}", config.name);
        Ok(record_id)
    }

    /// Update an existing plugin with new WASM data
    pub async fn update_plugin_wasm(
        &self,
        plugin_name: &str,
        new_version: String,
        wasm_data: Vec<u8>,
    ) -> Result<(), AppError> {
        debug!("Updating plugin WASM: {} to v{}", plugin_name, new_version);

        // Get existing plugin configuration
        let mut config = self.get_plugin_config(plugin_name).await?;

        // Remove old WASM file if it exists
        if let Some(old_wasm_path) = &config.wasm_path {
            if let Err(e) = tokio::task::spawn_blocking({
                let plugins_dir = self.plugins_dir.clone();
                let old_wasm_path = old_wasm_path.clone();
                move || plugin_fs::delete_wasm_file(&plugins_dir, &old_wasm_path)
            }).await {
                warn!("Failed to delete old WASM file: {}", e);
            }
        }

        // Save new WASM file
        let (wasm_path, wasm_size, wasm_hash) = tokio::task::spawn_blocking({
            let plugins_dir = self.plugins_dir.clone();
            let name = plugin_name.to_string();
            let version = new_version.clone();
            move || plugin_fs::save_wasm_file(&plugins_dir, &name, &version, &wasm_data)
        })
        .await
        .map_err(|e| AppError::internal(format!("Failed to spawn blocking task: {}", e)))?
        .map_err(|e| AppError::internal(format!("Failed to save WASM file: {}", e)))?;

        // Update configuration
        config.version = new_version;
        config.set_wasm_info(Some(wasm_path), Some(wasm_size), Some(wasm_hash));

        // Save updated configuration
        self.update_plugin_config(&config).await?;

        info!("✅ Successfully updated plugin WASM: {}", plugin_name);
        Ok(())
    }

    /// Load WASM data for a plugin from filesystem
    pub async fn load_plugin_wasm(&self, plugin_name: &str) -> Result<Vec<u8>, AppError> {
        debug!("Loading plugin WASM: {}", plugin_name);

        let config = self.get_plugin_config(plugin_name).await?;
        
        let wasm_path = config.wasm_path.ok_or_else(|| {
            AppError::not_found("wasm_file", plugin_name)
        })?;

        let wasm_data = tokio::task::spawn_blocking({
            let plugins_dir = self.plugins_dir.clone();
            let wasm_path = wasm_path.clone();
            move || plugin_fs::load_wasm_file(&plugins_dir, &wasm_path)
        })
        .await
        .map_err(|e| AppError::internal(format!("Failed to spawn blocking task: {}", e)))?
        .map_err(|e| AppError::internal(format!("Failed to load WASM file: {}", e)))?;

        // Verify file integrity if hash is available
        if let Some(expected_hash) = &config.wasm_hash {
            let is_valid = tokio::task::spawn_blocking({
                let plugins_dir = self.plugins_dir.clone();
                let wasm_path = wasm_path.clone();
                let expected_hash = expected_hash.clone();
                move || plugin_fs::verify_wasm_file(&plugins_dir, &wasm_path, &expected_hash)
            })
            .await
            .map_err(|e| AppError::internal(format!("Failed to spawn blocking task: {}", e)))?
            .map_err(|e| AppError::internal(format!("Failed to verify WASM file: {}", e)))?;

            if !is_valid {
                error!("WASM file integrity check failed for plugin: {}", plugin_name);
                return Err(AppError::internal("WASM file integrity check failed"));
            }
        }

        debug!("✅ Successfully loaded plugin WASM: {}", plugin_name);
        Ok(wasm_data)
    }

    /// Uninstall a plugin (remove from database and filesystem)
    pub async fn uninstall_plugin(&self, plugin_name: &str) -> Result<(), AppError> {
        debug!("Uninstalling plugin: {}", plugin_name);

        // Get plugin configuration to find WASM file
        let config = self.get_plugin_config(plugin_name).await?;

        // Remove WASM file from filesystem
        if let Some(wasm_path) = &config.wasm_path {
            if let Err(e) = tokio::task::spawn_blocking({
                let plugins_dir = self.plugins_dir.clone();
                let wasm_path = wasm_path.clone();
                move || plugin_fs::delete_wasm_file(&plugins_dir, &wasm_path)
            }).await {
                warn!("Failed to delete WASM file during uninstall: {}", e);
            }
        }

        // Remove from database
        self.delete_plugin_config(plugin_name).await?;

        info!("✅ Successfully uninstalled plugin: {}", plugin_name);
        Ok(())
    }

    /// List orphaned WASM files (files without corresponding database entries)
    pub async fn list_orphaned_wasm_files(&self) -> Result<Vec<String>, AppError> {
        debug!("Listing orphaned WASM files");

        // Get all WASM files from filesystem
        let filesystem_files = tokio::task::spawn_blocking({
            let plugins_dir = self.plugins_dir.clone();
            move || plugin_fs::list_wasm_files(&plugins_dir)
        })
        .await
        .map_err(|e| AppError::internal(format!("Failed to spawn blocking task: {}", e)))?
        .map_err(|e| AppError::internal(format!("Failed to list WASM files: {}", e)))?;

        // Get all plugin configurations from database
        let configs = self.list_plugin_configs().await?;
        let db_wasm_files: Vec<String> = configs
            .iter()
            .filter_map(|config| config.wasm_path.clone())
            .collect();

        // Find orphaned files
        let orphaned_files: Vec<String> = filesystem_files
            .into_iter()
            .filter(|file| !db_wasm_files.contains(file))
            .collect();

        debug!("✅ Found {} orphaned WASM files", orphaned_files.len());
        Ok(orphaned_files)
    }

    /// Clean up orphaned WASM files
    pub async fn cleanup_orphaned_files(&self) -> Result<u32, AppError> {
        debug!("Cleaning up orphaned WASM files");

        let orphaned_files = self.list_orphaned_wasm_files().await?;
        let mut cleaned_count = 0;

        for file in orphaned_files {
            if let Err(e) = tokio::task::spawn_blocking({
                let plugins_dir = self.plugins_dir.clone();
                let file = file.clone();
                move || plugin_fs::delete_wasm_file(&plugins_dir, &file)
            }).await {
                warn!("Failed to delete orphaned WASM file {}: {}", file, e);
            } else {
                cleaned_count += 1;
                debug!("Deleted orphaned WASM file: {}", file);
            }
        }

        info!("✅ Cleaned up {} orphaned WASM files", cleaned_count);
        Ok(cleaned_count)
    }

    /// Save a plugin configuration to the database
    pub async fn save_plugin_config(&self, config: &PluginConfiguration) -> Result<String, AppError> {
        debug!("Saving plugin configuration: {}", config.name);

        // Convert plugin config to record data
        let record_data = plugin_config_to_record(config)
            .map_err(|e| AppError::internal(format!("Failed to serialize plugin config: {}", e)))?;

        // Check if plugin already exists
        if let Ok(existing_record) = self.get_plugin_config(&config.name).await {
            // Update existing plugin
            let record = self.db.update_record("_plugins", &existing_record.name, record_data).await?;
            info!("✅ Updated plugin configuration: {}", config.name);
            Ok(record.id)
        } else {
            // Create new plugin
            let record = self.db.create_record("_plugins", record_data).await?;
            info!("✅ Created plugin configuration: {}", config.name);
            Ok(record.id)
        }
    }

    /// Get a plugin configuration by name
    pub async fn get_plugin_config(&self, plugin_name: &str) -> Result<PluginConfiguration, AppError> {
        debug!("Getting plugin configuration: {}", plugin_name);

        // List all plugins and find by name (since we need to search by name, not ID)
        let params = ListParams::default();
        let records = self.db.list_records("_plugins", params).await?;

        for record in records {
            if let Ok(config) = record_to_plugin_config(&record.data) {
                if config.name == plugin_name {
                    debug!("✅ Found plugin configuration: {}", plugin_name);
                    return Ok(config);
                }
            }
        }

        Err(AppError::not_found("plugin", plugin_name))
    }

    /// List all plugin configurations
    pub async fn list_plugin_configs(&self) -> Result<Vec<PluginConfiguration>, AppError> {
        debug!("Listing all plugin configurations");

        let params = ListParams::default();
        let records = self.db.list_records("_plugins", params).await?;

        let mut configs = Vec::new();
        for record in records {
            match record_to_plugin_config(&record.data) {
                Ok(config) => configs.push(config),
                Err(e) => {
                    warn!("Failed to deserialize plugin config from record {}: {}", record.id, e);
                }
            }
        }

        debug!("✅ Listed {} plugin configurations", configs.len());
        Ok(configs)
    }

    /// Update a plugin configuration
    pub async fn update_plugin_config(&self, config: &PluginConfiguration) -> Result<(), AppError> {
        debug!("Updating plugin configuration: {}", config.name);

        // Get existing record to verify it exists
        let _existing_config = self.get_plugin_config(&config.name).await?;
        
        // Convert to record data
        let record_data = plugin_config_to_record(config)
            .map_err(|e| AppError::internal(format!("Failed to serialize plugin config: {}", e)))?;

        // Find the record ID by listing and matching name
        let params = ListParams::default();
        let records = self.db.list_records("_plugins", params).await?;

        for record in records {
            if let Ok(existing) = record_to_plugin_config(&record.data) {
                if existing.name == config.name {
                    self.db.update_record("_plugins", &record.id, record_data).await?;
                    info!("✅ Updated plugin configuration: {}", config.name);
                    return Ok(());
                }
            }
        }

        Err(AppError::not_found("plugin", &config.name))
    }

    /// Delete a plugin configuration
    pub async fn delete_plugin_config(&self, plugin_name: &str) -> Result<(), AppError> {
        debug!("Deleting plugin configuration: {}", plugin_name);

        // Find the record ID by listing and matching name
        let params = ListParams::default();
        let records = self.db.list_records("_plugins", params).await?;

        for record in records {
            if let Ok(config) = record_to_plugin_config(&record.data) {
                if config.name == plugin_name {
                    self.db.delete_record("_plugins", &record.id).await?;
                    info!("✅ Deleted plugin configuration: {}", plugin_name);
                    return Ok(());
                }
            }
        }

        Err(AppError::not_found("plugin", plugin_name))
    }

    /// Enable a plugin
    pub async fn enable_plugin(&self, plugin_name: &str) -> Result<(), AppError> {
        debug!("Enabling plugin: {}", plugin_name);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.enable();
        self.update_plugin_config(&config).await?;

        info!("✅ Enabled plugin: {}", plugin_name);
        Ok(())
    }

    /// Disable a plugin
    pub async fn disable_plugin(&self, plugin_name: &str) -> Result<(), AppError> {
        debug!("Disabling plugin: {}", plugin_name);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.disable();
        self.update_plugin_config(&config).await?;

        info!("✅ Disabled plugin: {}", plugin_name);
        Ok(())
    }

    /// Update plugin status
    pub async fn update_plugin_status(&self, plugin_name: &str, status: PluginStatus) -> Result<(), AppError> {
        debug!("Updating plugin status: {} -> {:?}", plugin_name, status);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.set_status(status);
        self.update_plugin_config(&config).await?;

        info!("✅ Updated plugin status: {}", plugin_name);
        Ok(())
    }

    /// Add capability to a plugin
    pub async fn add_plugin_capability(&self, plugin_name: &str, capability: PluginCapability) -> Result<(), AppError> {
        debug!("Adding capability to plugin: {} -> {:?}", plugin_name, capability);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.add_capability(capability);
        self.update_plugin_config(&config).await?;

        info!("✅ Added capability to plugin: {}", plugin_name);
        Ok(())
    }

    /// Remove capability from a plugin
    pub async fn remove_plugin_capability(&self, plugin_name: &str, capability: &PluginCapability) -> Result<(), AppError> {
        debug!("Removing capability from plugin: {} -> {:?}", plugin_name, capability);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.remove_capability(capability);
        self.update_plugin_config(&config).await?;

        info!("✅ Removed capability from plugin: {}", plugin_name);
        Ok(())
    }

    /// Update plugin trust level
    pub async fn update_plugin_trust_level(&self, plugin_name: &str, trust_level: PluginTrustLevel) -> Result<(), AppError> {
        debug!("Updating plugin trust level: {} -> {:?}", plugin_name, trust_level);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.set_trust_level(trust_level);
        self.update_plugin_config(&config).await?;

        info!("✅ Updated plugin trust level: {}", plugin_name);
        Ok(())
    }

    /// Update plugin resource limits
    pub async fn update_plugin_resource_limits(&self, plugin_name: &str, resource_limits: ResourceLimits) -> Result<(), AppError> {
        debug!("Updating plugin resource limits: {}", plugin_name);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.set_resource_limits(resource_limits);
        self.update_plugin_config(&config).await?;

        info!("✅ Updated plugin resource limits: {}", plugin_name);
        Ok(())
    }

    /// Get enabled plugins
    pub async fn get_enabled_plugins(&self) -> Result<Vec<PluginConfiguration>, AppError> {
        debug!("Getting enabled plugins");

        let all_configs = self.list_plugin_configs().await?;
        let enabled_configs: Vec<PluginConfiguration> = all_configs
            .into_iter()
            .filter(|config| config.enabled)
            .collect();

        debug!("✅ Found {} enabled plugins", enabled_configs.len());
        Ok(enabled_configs)
    }

    /// Check if a plugin exists
    pub async fn plugin_exists(&self, plugin_name: &str) -> Result<bool, AppError> {
        match self.get_plugin_config(plugin_name).await {
            Ok(_) => Ok(true),
            Err(AppError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Load all enabled plugins with their WASM data
    pub async fn load_enabled_plugins(&self) -> Result<Vec<PluginConfiguration>, AppError> {
        debug!("Loading all enabled plugins");

        let enabled_configs = self.get_enabled_plugins().await?;

        debug!("✅ Loaded {} enabled plugins", enabled_configs.len());
        Ok(enabled_configs)
    }

    /// Update plugin metadata with extracted information from the plugin runtime
    pub async fn update_plugin_metadata(&self, plugin_name: &str, metadata: serde_json::Value) -> Result<(), AppError> {
        debug!("Updating plugin metadata: {}", plugin_name);

        let mut config = self.get_plugin_config(plugin_name).await?;
        config.set_metadata(metadata);
        self.update_plugin_config(&config).await?;

        info!("✅ Updated plugin metadata: {}", plugin_name);
        Ok(())
    }
} 