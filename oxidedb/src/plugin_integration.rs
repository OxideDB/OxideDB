//! Plugin Integration and Management
//!
//! This module provides high-level plugin management functionality including
//! plugin loading, event system integration, and lifecycle management.

use crate::{WasmtimePluginRuntime, Result};
use oxide_core::{
    AppError, BeforeEventContext, BeforeEventType, EventBus,
    plugin_api::{EventPayload, plugin_exports, PluginRuntime},
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits, SecurityPolicies},
    auth::CrudOperation,
};
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tracing::{info, warn, error};

/// Type alias for before create event handlers
type BeforeCreateHandler = Arc<dyn Fn(&mut BeforeEventContext) -> Pin<Box<dyn Future<Output = std::result::Result<(), AppError>> + Send + '_>> + Send + Sync>;

/// High-level plugin manager that orchestrates plugin operations
pub struct PluginManager {
    runtime: Arc<Mutex<WasmtimePluginRuntime>>,
    bridge: PluginEventBridge,
}

impl PluginManager {
    /// Create a new plugin manager with the specified security policies
    pub fn new(security_policies: SecurityPolicies) -> Result<Self> {
        let runtime = WasmtimePluginRuntime::new_with_security_policies(security_policies)
            .map_err(|e| AppError::internal(format!("Failed to create plugin runtime: {}", e)))?;
        
        let runtime = Arc::new(Mutex::new(runtime));
        let bridge = PluginEventBridge::new(Arc::clone(&runtime));

        Ok(Self { runtime, bridge })
    }

    /// Load all plugins from the specified folder
    pub async fn load_plugins_from_folder(
        &mut self,
        plugin_folder: &PathBuf,
        _event_bus: &Arc<dyn EventBus>,
    ) -> Result<()> {
        info!("Loading plugins from folder: {:?}", plugin_folder);
        
        let entries = std::fs::read_dir(plugin_folder)
            .map_err(|e| AppError::internal(format!("Failed to read plugin folder: {}", e)))?;

        let mut loaded_plugins = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| AppError::internal(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                let plugin_name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                
                self.load_single_plugin(plugin_name, &path).await?;
                loaded_plugins.push(plugin_name.to_string());
            }
        }
        
        info!("✅ Loaded {} plugins from folder", loaded_plugins.len());
        Ok(())
    }

    /// Register all loaded plugins with the event system
    pub async fn register_with_event_system(&self, event_bus: &Arc<dyn EventBus>) -> Result<()> {
        let loaded_plugins = {
            let runtime_guard = self.runtime.lock().map_err(|_| {
                AppError::internal("Failed to acquire plugin runtime lock")
            })?;
            runtime_guard.list_plugins()
        };
        
        for plugin_name in loaded_plugins {
            let plugin_handler = self.bridge.create_before_create_handler(plugin_name.clone());
            let metadata = oxide_core::event::handlers::HandlerMetadata::new(
                format!("plugin_{}", plugin_name),
                format!("Plugin: {}", plugin_name)
            ).with_description(format!("WASM plugin handler for {}", plugin_name));
            
            event_bus.subscribe_before(
                BeforeEventType::RecordCreate.name(),
                plugin_handler,
                metadata,
            ).await?;
            info!("✅ Plugin '{}' registered with event system", plugin_name);
        }

        // Register demo hook if no plugins are loaded
        if self.get_loaded_plugin_count()? == 0 {
            let demo_metadata = oxide_core::event::handlers::HandlerMetadata::new(
                "demo_hook".to_string(),
                "Demo Hook".to_string()
            ).with_description("Demo event listener for showcasing event system".to_string());
            
            event_bus.subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(|context: &mut BeforeEventContext| {
                    Box::pin(async move {
                        info!(
                            "🎣 Demo Hook triggered: About to create record in collection '{}' with data: {}",
                            context.collection, context.data
                        );
                        Ok(())
                    })
                }),
                demo_metadata,
            ).await?;
            info!("✅ Demo event listener registered (no plugins loaded)");
        }

        Ok(())
    }

    /// Get the number of loaded plugins
    pub fn get_loaded_plugin_count(&self) -> Result<usize> {
        let runtime_guard = self.runtime.lock().map_err(|_| {
            AppError::internal("Failed to acquire plugin runtime lock")
        })?;
        Ok(runtime_guard.list_plugins().len())
    }

    /// Get statistics for all loaded plugins
    pub fn get_plugin_statistics(&self) -> Result<Vec<PluginStatistics>> {
        let runtime_guard = self.runtime.lock().map_err(|_| {
            AppError::internal("Failed to acquire plugin runtime lock")
        })?;
        
        let mut stats = Vec::new();
        for plugin_name in runtime_guard.list_plugins() {
            stats.push(PluginStatistics {
                name: plugin_name,
                status: PluginStatus::Active, // TODO: Implement real status tracking
                executions: 0, // TODO: Implement execution counting
                errors: 0, // TODO: Implement error tracking
            });
        }
        
        Ok(stats)
    }

    /// Load a single plugin from the specified path
    async fn load_single_plugin(&self, plugin_name: &str, path: &PathBuf) -> Result<()> {
        info!("Loading plugin: {} from {:?}", plugin_name, path);
        
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| AppError::internal(format!("Failed to read WASM file {:?}: {}", path, e)))?;
        
        let mut runtime_guard = self.runtime.lock().map_err(|_| {
            AppError::internal("Failed to acquire plugin runtime lock")
        })?;

        // Load plugin with development-friendly settings
        runtime_guard.load_plugin_with_trust(
            plugin_name,
            &wasm_bytes,
            PluginTrustLevel::PartiallyTrusted,
            vec![
                PluginCapability::ReadEventData,
                PluginCapability::ModifyEventData,
                PluginCapability::BlockOperations,
                PluginCapability::AccessCollection {
                    collection: "*".to_string(),
                    operations: vec![CrudOperation::Create, CrudOperation::Read, CrudOperation::Update, CrudOperation::Delete],
                },
                PluginCapability::LogInfo,
                PluginCapability::LogError,
            ],
            ResourceLimits::default(),
        ).map_err(|e| AppError::internal(format!("Failed to load plugin {}: {}", plugin_name, e)))?;
        
        info!("✅ Plugin '{}' loaded successfully", plugin_name);
        
        // Test plugin initialization
        let test_payload = EventPayload {
            event_type: "plugin_init".to_string(),
            collection: "test".to_string(),
            data: "{}".to_string(),
            metadata: serde_json::json!({}),
        };
        
        let _init_response = runtime_guard.call_plugin_function(
            plugin_name,
            plugin_exports::PLUGIN_INIT,
            &test_payload,
        ).map_err(|e| AppError::internal(format!("Failed to initialize plugin {}: {}", plugin_name, e)))?;
        
        info!("✅ Plugin '{}' initialized successfully", plugin_name);
        Ok(())
    }
}

/// Bridge to connect WASM plugins with the EventBus system
pub struct PluginEventBridge {
    plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>,
}

impl PluginEventBridge {
    /// Create a new plugin event bridge
    pub fn new(plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>) -> Self {
        Self { plugin_runtime }
    }

    /// Create an event handler that calls the plugin for BeforeRecordCreate events
    pub fn create_before_create_handler(&self, plugin_name: String) -> BeforeCreateHandler {
        let runtime = Arc::clone(&self.plugin_runtime);
        
        Arc::new(move |context: &mut BeforeEventContext| {
            let runtime = Arc::clone(&runtime);
            let plugin_name = plugin_name.clone();
            Box::pin(async move {
                info!("🔌 Calling plugin '{}' for BeforeRecordCreate event", plugin_name);
                
                // Convert BeforeEventContext to EventPayload
                let payload = EventPayload {
                    event_type: "BeforeRecordCreate".to_string(),
                    collection: context.collection.clone(),
                    data: context.data.to_string(),
                    metadata: context.metadata.clone(),
                };

                // Call the plugin
                let mut runtime_guard = runtime.lock().map_err(|_| {
                    AppError::internal("Failed to acquire plugin runtime lock")
                })?;

                match runtime_guard.call_plugin_function(&plugin_name, plugin_exports::ON_BEFORE_CREATE, &payload) {
                    Ok(response) => {
                        info!("🔌 Plugin '{}' response: allow={}, error={:?}", 
                              plugin_name, response.allow, response.error_message);
                        
                        // If plugin modified the data, update the context
                        if let Some(ref modified_data) = response.modified_data {
                            match serde_json::from_str(modified_data) {
                                Ok(new_data) => {
                                    context.data = new_data;
                                    info!("🔌 Plugin modified data: {}", modified_data);
                                }
                                Err(e) => {
                                    warn!("🔌 Plugin returned invalid modified data: {}", e);
                                }
                            }
                        }
                        
                        // If plugin doesn't allow the operation, return an error
                        if !response.allow {
                            let error_msg = response.error_message.unwrap_or_else(|| 
                                "Plugin rejected the operation".to_string()
                            );
                            return Err(AppError::plugin(&plugin_name, &error_msg));
                        }
                        
                        Ok(())
                    }
                    Err(e) => {
                        error!("🔌 Plugin '{}' execution failed: {}", plugin_name, e);
                        Err(AppError::plugin(plugin_name, format!("Plugin execution failed: {}", e)))
                    }
                }
            })
        })
    }
}

/// Plugin execution statistics
#[derive(Debug, Clone)]
pub struct PluginStatistics {
    pub name: String,
    pub status: PluginStatus,
    pub executions: u64,
    pub errors: u64,
}

/// Plugin status enumeration
#[derive(Debug, Clone)]
pub enum PluginStatus {
    Active,
    Suspended,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::plugin_security::SecurityPolicies;

    #[test]
    fn test_plugin_manager_creation() {
        let policies = SecurityPolicies::default();
        let manager = PluginManager::new(policies);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_plugin_event_bridge_creation() {
        let policies = SecurityPolicies::default();
        let runtime = WasmtimePluginRuntime::new_with_security_policies(policies).unwrap();
        let runtime = Arc::new(Mutex::new(runtime));
        let bridge = PluginEventBridge::new(runtime);
        
        // Bridge should be created successfully
        assert!(std::ptr::addr_of!(bridge) as usize > 0);
    }
} 