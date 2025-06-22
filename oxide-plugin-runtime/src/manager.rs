//! Plugin Management and Event System Integration
//!
//! This module provides high-level plugin management functionality including
//! plugin loading, event system integration, and lifecycle management.

use crate::WasmtimePluginRuntime;
use oxide_core::{
    AppError, BeforeEventContext, BeforeEventType, EventBus,
    plugin_api::{EventPayload, plugin_exports, PluginRuntime},
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits, SecurityPolicies},
    auth::CrudOperation,
};
use oxide_db::Db;
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
    /// Create a new plugin manager with the specified security policies and database
    pub fn new(database: Arc<dyn Db>, security_policies: SecurityPolicies) -> Result<Self, AppError> {
        let runtime = WasmtimePluginRuntime::new_with_security_policies(database, security_policies)
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
    ) -> Result<(), AppError> {
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
    pub async fn register_with_event_system(&self, event_bus: &Arc<dyn EventBus>) -> Result<(), AppError> {
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
    pub fn get_loaded_plugin_count(&self) -> Result<usize, AppError> {
        let runtime_guard = self.runtime.lock().map_err(|_| {
            AppError::internal("Failed to acquire plugin runtime lock")
        })?;
        Ok(runtime_guard.list_plugins().len())
    }

    /// Get statistics for all loaded plugins
    pub fn get_plugin_statistics(&self) -> Result<Vec<PluginStatistics>, AppError> {
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

    /// Get all registered HTTP routes from all loaded plugins
    pub fn get_registered_routes(&self) -> Result<Vec<oxide_core::plugin_api::RouteRegistration>, AppError> {
        let runtime_guard = self.runtime.lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        Ok(runtime_guard.get_registered_routes())
    }

    /// Handle HTTP request to a plugin route
    pub async fn handle_http_request(
        &self,
        plugin_name: &str,
        handler_function: &str,
        request: &oxide_core::plugin_api::HttpRequestContext,
    ) -> Result<oxide_core::plugin_api::HttpResponse, AppError> {
        let mut runtime_guard = self.runtime.lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.handle_http_request(plugin_name, handler_function, request)
            .map_err(|e| AppError::internal(format!("Plugin HTTP request failed: {}", e)))
    }

    /// Find plugin handler for a given route
    pub fn find_route_handler(&self, method: &str, path: &str) -> Result<Option<(String, String)>, AppError> {
        let routes = self.get_registered_routes()?;
        
        for route in routes {
            if route.method.to_uppercase() == method.to_uppercase() {
                if self.path_matches_pattern(&route.path, path) {
                    return Ok(Some((route.plugin_name, route.handler_function)));
                }
            }
        }
        
        Ok(None)
    }

    /// Check if a path matches a route pattern (supports :param syntax)
    fn path_matches_pattern(&self, pattern: &str, path: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.trim_start_matches('/').split('/').collect();
        let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return false;
        }

        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if pattern_part.starts_with(':') {
                // Parameter match - always matches
                continue;
            } else if pattern_part != path_part {
                return false;
            }
        }

        true
    }

    /// Extract path parameters from a matched route
    pub fn extract_path_params(&self, pattern: &str, path: &str) -> std::collections::HashMap<String, String> {
        let mut params = std::collections::HashMap::new();
        let pattern_parts: Vec<&str> = pattern.trim_start_matches('/').split('/').collect();
        let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        for (pattern_part, path_part) in pattern_parts.iter().zip(path_parts.iter()) {
            if let Some(param_name) = pattern_part.strip_prefix(':') {
                params.insert(param_name.to_string(), path_part.to_string());
            }
        }

        params
    }

    /// Load a single plugin from the specified path
    async fn load_single_plugin(&self, plugin_name: &str, path: &PathBuf) -> Result<(), AppError> {
        info!("Loading plugin: {} from {:?}", plugin_name, path);
        
        let wasm_bytes = std::fs::read(path)
            .map_err(|e| AppError::internal(format!("Failed to read WASM file {:?}: {}", path, e)))?;
        
        let mut runtime_guard = self.runtime.lock().map_err(|_| {
            AppError::internal("Failed to acquire plugin runtime lock")
        })?;

        // Load plugin with development-friendly settings including HTTP capabilities
        runtime_guard.load_plugin_with_trust(
            plugin_name,
            &wasm_bytes,
            PluginTrustLevel::FullyTrusted,
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
                // Add HTTP capabilities for plugin routes
                PluginCapability::RegisterHttpRoutes {
                    path_patterns: vec!["*".to_string()],
                    methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()],
                },
                PluginCapability::HandleHttpRequests,
                PluginCapability::CreateRecords {
                    collections: vec!["*".to_string()],
                },
                PluginCapability::ReadRecords {
                    collections: vec!["*".to_string()],
                },
                PluginCapability::UpdateRecords {
                    collections: vec!["*".to_string()],
                },
                PluginCapability::DeleteRecords {
                    collections: vec!["*".to_string()],
                },
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
        
        // Log plugin routes if any were registered (call directly on runtime_guard to avoid deadlock)
        let all_routes = runtime_guard.get_registered_routes();
        let plugin_routes: Vec<_> = all_routes.into_iter()
            .filter(|route| route.plugin_name == plugin_name)
            .collect();
        
        if !plugin_routes.is_empty() {
            info!("📋 Plugin '{}' registered {} HTTP routes:", plugin_name, plugin_routes.len());
            for route in plugin_routes {
                info!("  🔌 {:>6} /plugin{} → {}::{}", 
                      route.method, 
                      route.path, 
                      route.plugin_name, 
                      route.handler_function);
            }
        }
        
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
    use oxide_core::auth::{AuthService, AuthServiceConfig};
    use oxide_core::InMemoryEventBus;
    use oxide_db::SqliteDb;

    #[test]
    fn test_plugin_manager_creation() {
        let policies = SecurityPolicies::default();
        // Create a mock database for testing
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let event_bus = Arc::new(InMemoryEventBus::new());
        let db = SqliteDb::new(":memory:", event_bus, auth_service).unwrap();
        let manager = PluginManager::new(Arc::new(db), policies);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_plugin_event_bridge_creation() {
        let policies = SecurityPolicies::default();
        // Create a mock database for testing
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let event_bus = Arc::new(InMemoryEventBus::new());
        let db = SqliteDb::new(":memory:", event_bus, auth_service).unwrap();
        let runtime = WasmtimePluginRuntime::new_with_security_policies(Arc::new(db), policies).unwrap();
        let runtime = Arc::new(Mutex::new(runtime));
        let bridge = PluginEventBridge::new(runtime);
        
        // Bridge should be created successfully
        assert!(std::ptr::addr_of!(bridge) as usize > 0);
    }
} 