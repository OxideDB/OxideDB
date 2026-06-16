//! Plugin Management and Event System Integration
//!
//! This module provides high-level plugin management functionality including
//! plugin loading, event system integration, and lifecycle management.

use crate::{
    host_state::{lock_host_state, HostState, HostStateRef},
    WasmtimePluginRuntime,
};
use oxide_core::{
    plugin_api::{plugin_exports, EventPayload, PluginError, PluginRuntime},
    plugin_config::PluginConfiguration,
    plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits, SecurityPolicies},
    AfterEventContext, AfterEventType, AppError, BeforeEventContext, BeforeEventType, EventBus,
};
use oxide_db::Db;
use std::{
    future::Future,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
};
use tracing::{error, info, warn};

/// Type alias for before event handlers
type BeforeHandler = Arc<
    dyn Fn(
            &mut BeforeEventContext,
        ) -> Pin<Box<dyn Future<Output = std::result::Result<(), AppError>> + Send + '_>>
        + Send
        + Sync,
>;

/// Type alias for after event handlers
type AfterHandler = Arc<
    dyn Fn(&AfterEventContext) -> Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        + Send
        + Sync,
>;

/// High-level plugin manager that orchestrates plugin operations.
///
/// # Concurrency model (architectural note)
///
/// All plugin execution currently serializes on a single
/// `Arc<Mutex<WasmtimePluginRuntime>>`. [`WasmtimePluginRuntime`] now keeps a
/// separate Wasmtime `Store` for each loaded plugin instance, so plugin memory
/// and fuel are no longer owned by one shared store. The manager-level mutex is
/// intentionally still in place until runtime state that remains shared
/// between plugins (route registration, execution context, service bridges,
/// and hook recursion detection) can be split or protected with narrower
/// per-plugin locks.
///
/// To avoid deadlock when a plugin's HTTP handler (which holds this lock)
/// triggers a DB write that fires a hook needing the lock, hook dispatch uses
/// `try_lock`: if the runtime is busy, before-hooks currently *fail closed*
/// (reject the write) unless the caller is the same plugin recursing, and
/// after-hooks *fail soft* (skip). This preserves validation-hook safety but
/// means a slow plugin can block or reject unrelated plugin work. The safe path
/// forward is to move the remaining shared host state into explicit shared
/// services plus per-plugin execution state, then replace the global runtime
/// lock with per-plugin execution locks.
pub struct PluginManager {
    pub runtime: Arc<Mutex<WasmtimePluginRuntime>>,
    bridge: PluginEventBridge,
    security_policies: SecurityPolicies,
}

impl PluginManager {
    /// Create a new plugin manager with the specified security policies and database
    pub fn new(
        database: Arc<dyn Db>,
        security_policies: SecurityPolicies,
    ) -> Result<Self, AppError> {
        let runtime =
            WasmtimePluginRuntime::new_with_security_policies(database, security_policies)
                .map_err(|e| {
                    AppError::internal(format!("Failed to create plugin runtime: {}", e))
                })?;

        let runtime = Arc::new(Mutex::new(runtime));
        let bridge = PluginEventBridge::new(Arc::clone(&runtime));

        let security_policies = {
            let runtime_guard = runtime
                .lock()
                .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;
            runtime_guard.security_policies().clone()
        };

        Ok(Self {
            runtime,
            bridge,
            security_policies,
        })
    }

    /// Return true when this manager requires verified code signatures.
    pub fn requires_code_signing(&self) -> bool {
        self.security_policies.require_code_signing
    }

    /// Load all plugins from the specified folder
    pub async fn load_plugins_from_folder(
        &mut self,
        plugin_folder: &PathBuf,
        _event_bus: &Arc<dyn EventBus>,
    ) -> Result<(), AppError> {
        info!("Loading plugins from folder: {:?}", plugin_folder);

        if self.requires_code_signing() {
            warn!(
                "Skipping legacy plugin folder loading because code signing is required and folder plugins have no persisted signature verification"
            );
            return Ok(());
        }

        let entries = std::fs::read_dir(plugin_folder)
            .map_err(|e| AppError::internal(format!("Failed to read plugin folder: {}", e)))?;

        let mut loaded_plugins = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|e| {
                AppError::internal(format!("Failed to read directory entry: {}", e))
            })?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                let plugin_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                self.load_single_plugin(plugin_name, &path, None).await?;
                loaded_plugins.push(plugin_name.to_string());
            }
        }

        info!("✅ Loaded {} plugins from folder", loaded_plugins.len());
        Ok(())
    }

    /// Register all loaded plugins with the event system
    pub async fn register_with_event_system(
        &self,
        event_bus: &Arc<dyn EventBus>,
    ) -> Result<(), AppError> {
        let loaded_plugins = {
            let runtime_guard = self
                .runtime
                .lock()
                .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;
            runtime_guard.list_plugins()
        };

        for plugin_name in loaded_plugins {
            self.register_plugin_with_event_system(event_bus, &plugin_name)
                .await?;
        }

        // Register demo hook if no plugins are loaded
        if self.get_loaded_plugin_count()? == 0 {
            let demo_metadata = oxide_core::event::handlers::HandlerMetadata::new(
                "demo_hook".to_string(),
                "Demo Hook".to_string(),
            )
            .with_description("Demo event listener for showcasing event system".to_string());

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

    /// Register one loaded plugin with all supported record event hooks.
    ///
    /// Handler IDs are stable and are unsubscribed before re-subscription so API
    /// installs/enables can safely call this without duplicating event handlers.
    pub async fn register_plugin_with_event_system(
        &self,
        event_bus: &Arc<dyn EventBus>,
        plugin_name: &str,
    ) -> Result<(), AppError> {
        if !self.is_plugin_loaded(plugin_name)? {
            warn!(
                "Plugin '{}' is not loaded; skipping event handler registration",
                plugin_name
            );
            return Ok(());
        }

        for (event_type, function_name) in Self::before_event_bindings() {
            let event_name = event_type.name();
            let handler_id = Self::plugin_event_handler_id(plugin_name, event_name);
            let _ = event_bus
                .unsubscribe_before(event_name, &handler_id)
                .await?;

            let plugin_handler = self
                .bridge
                .create_before_handler(plugin_name.to_string(), function_name);
            let metadata = oxide_core::event::handlers::HandlerMetadata::new(
                handler_id,
                format!("Plugin: {} ({})", plugin_name, event_name),
            )
            .with_description(format!(
                "WASM plugin '{}' handler for {}",
                plugin_name, event_name
            ))
            .with_tag("plugin".to_string(), plugin_name.to_string());

            event_bus
                .subscribe_before(event_name, plugin_handler, metadata)
                .await?;
        }

        for (event_type, function_name) in Self::after_event_bindings() {
            let event_name = event_type.name();
            let handler_id = Self::plugin_event_handler_id(plugin_name, event_name);
            let _ = event_bus.unsubscribe_after(event_name, &handler_id).await?;

            let plugin_handler = self
                .bridge
                .create_after_handler(plugin_name.to_string(), function_name);
            let metadata = oxide_core::event::handlers::HandlerMetadata::new(
                handler_id,
                format!("Plugin: {} ({})", plugin_name, event_name),
            )
            .with_description(format!(
                "WASM plugin '{}' handler for {}",
                plugin_name, event_name
            ))
            .with_tag("plugin".to_string(), plugin_name.to_string());

            event_bus
                .subscribe_after(event_name, plugin_handler, metadata)
                .await?;
        }

        info!("✅ Plugin '{}' registered with event system", plugin_name);
        Ok(())
    }

    /// Load a plugin into the runtime and register its event handlers.
    ///
    /// If handler registration fails after the WASM module has loaded, runtime
    /// state and any partially registered handlers are removed before returning
    /// the original registration error.
    pub async fn load_and_register_plugin(
        &self,
        event_bus: &Arc<dyn EventBus>,
        config: &PluginConfiguration,
        wasm_bytes: &[u8],
    ) -> Result<(), AppError> {
        self.unregister_plugin_from_event_system(event_bus, &config.name)
            .await?;
        self.load_plugin_bytes_with_config(&config.name, wasm_bytes, config)
            .await?;

        if let Err(error) = self
            .register_plugin_with_event_system(event_bus, &config.name)
            .await
        {
            warn!(
                "Plugin '{}' loaded but event registration failed; rolling back runtime state: {}",
                config.name, error
            );
            if let Err(cleanup_error) = self
                .unregister_and_unload_plugin(event_bus, &config.name)
                .await
            {
                warn!(
                    "Failed to fully roll back plugin '{}' after registration failure: {}",
                    config.name, cleanup_error
                );
            }
            return Err(error);
        }

        Ok(())
    }

    /// Remove all record event handlers for one plugin.
    pub async fn unregister_plugin_from_event_system(
        &self,
        event_bus: &Arc<dyn EventBus>,
        plugin_name: &str,
    ) -> Result<(), AppError> {
        for (event_type, _) in Self::before_event_bindings() {
            let event_name = event_type.name();
            let handler_id = Self::plugin_event_handler_id(plugin_name, event_name);
            let _ = event_bus
                .unsubscribe_before(event_name, &handler_id)
                .await?;
        }

        for (event_type, _) in Self::after_event_bindings() {
            let event_name = event_type.name();
            let handler_id = Self::plugin_event_handler_id(plugin_name, event_name);
            let _ = event_bus.unsubscribe_after(event_name, &handler_id).await?;
        }

        info!("✅ Plugin '{}' unregistered from event system", plugin_name);
        Ok(())
    }

    /// Remove a plugin from the event system and runtime.
    ///
    /// Both cleanup steps are attempted so a partial failure does not leave the
    /// other side active. The first error is returned after cleanup finishes.
    pub async fn unregister_and_unload_plugin(
        &self,
        event_bus: &Arc<dyn EventBus>,
        plugin_name: &str,
    ) -> Result<(), AppError> {
        let unregister_result = self
            .unregister_plugin_from_event_system(event_bus, plugin_name)
            .await;
        let unload_result = self.unload_plugin_from_runtime(plugin_name);

        match (unregister_result, unload_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(unload_error)) => {
                warn!(
                    "Plugin '{}' event unregistration and runtime unload both failed; unload error: {}",
                    plugin_name, unload_error
                );
                Err(error)
            }
        }
    }

    /// Remove a plugin from the runtime without touching EventBus handlers.
    pub fn unload_plugin_from_runtime(&self, plugin_name: &str) -> Result<(), AppError> {
        let mut runtime_guard = self
            .runtime
            .lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;

        runtime_guard
            .unload_plugin(plugin_name)
            .map_err(|e| AppError::internal(format!("Failed to unload plugin: {}", e)))
    }

    fn before_event_bindings() -> [(BeforeEventType, &'static str); 3] {
        [
            (
                BeforeEventType::RecordCreate,
                plugin_exports::ON_BEFORE_CREATE,
            ),
            (
                BeforeEventType::RecordUpdate,
                plugin_exports::ON_BEFORE_UPDATE,
            ),
            (
                BeforeEventType::RecordDelete,
                plugin_exports::ON_BEFORE_DELETE,
            ),
        ]
    }

    fn after_event_bindings() -> [(AfterEventType, &'static str); 3] {
        [
            (
                AfterEventType::RecordCreated,
                plugin_exports::ON_AFTER_CREATE,
            ),
            (
                AfterEventType::RecordUpdated,
                plugin_exports::ON_AFTER_UPDATE,
            ),
            (
                AfterEventType::RecordDeleted,
                plugin_exports::ON_AFTER_DELETE,
            ),
        ]
    }

    fn plugin_event_handler_id(plugin_name: &str, event_name: &str) -> String {
        format!("plugin_{}_{}", plugin_name, event_name)
    }

    /// Return true when a plugin is currently loaded in the runtime.
    pub fn is_plugin_loaded(&self, plugin_name: &str) -> Result<bool, AppError> {
        let runtime_guard = self
            .runtime
            .lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;

        Ok(runtime_guard
            .list_plugins()
            .iter()
            .any(|loaded_plugin| loaded_plugin == plugin_name))
    }

    /// Get the number of loaded plugins
    pub fn get_loaded_plugin_count(&self) -> Result<usize, AppError> {
        let runtime_guard = self
            .runtime
            .lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;
        Ok(runtime_guard.list_plugins().len())
    }

    /// Get statistics for all loaded plugins
    pub fn get_plugin_statistics(&self) -> Result<Vec<PluginStatistics>, AppError> {
        let runtime_guard = self
            .runtime
            .lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;

        let mut stats = Vec::new();
        for plugin_name in runtime_guard.list_plugins() {
            let execution_stats = runtime_guard
                .get_plugin_stats(&plugin_name)
                .unwrap_or_default();
            let status = if runtime_guard.is_plugin_suspended(&plugin_name) {
                PluginStatus::Suspended
            } else if execution_stats.failed_executions > 0
                && execution_stats.failed_executions == execution_stats.total_executions
            {
                PluginStatus::Error
            } else {
                PluginStatus::Active
            };

            stats.push(PluginStatistics {
                name: plugin_name,
                status,
                executions: execution_stats.total_executions,
                errors: execution_stats.failed_executions,
                last_execution: execution_stats.last_execution,
                total_execution_time_ms: execution_stats.total_execution_time,
                host_function_calls: execution_stats.host_function_calls,
                peak_memory_usage: execution_stats.peak_memory_usage,
            });
        }

        Ok(stats)
    }

    /// Get all registered HTTP routes from all loaded plugins
    pub fn get_registered_routes(
        &self,
    ) -> Result<Vec<oxide_core::plugin_api::RouteRegistration>, AppError> {
        let runtime_guard = self
            .runtime
            .lock()
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
        let mut runtime_guard = self
            .runtime
            .lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock".to_string()))?;

        runtime_guard
            .handle_http_request(plugin_name, handler_function, request)
            .map_err(|e| match e {
                PluginError::SecurityViolation(message) => AppError::security(format!(
                    "Plugin HTTP request denied for '{}': {}",
                    plugin_name, message
                )),
                other => AppError::internal(format!("Plugin HTTP request failed: {}", other)),
            })
    }

    /// Find plugin handler for a given route
    pub fn find_route_handler(
        &self,
        method: &str,
        path: &str,
    ) -> Result<Option<(String, String)>, AppError> {
        let routes = self.get_registered_routes()?;

        for route in routes {
            if route.method.to_uppercase() == method.to_uppercase()
                && self.path_matches_pattern(&route.path, path)
            {
                return Ok(Some((route.plugin_name, route.handler_function)));
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
    pub fn extract_path_params(
        &self,
        pattern: &str,
        path: &str,
    ) -> std::collections::HashMap<String, String> {
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

    /// Load a single plugin from the specified path with optional configuration
    async fn load_single_plugin(
        &self,
        plugin_name: &str,
        path: &PathBuf,
        config: Option<&oxide_core::plugin_config::PluginConfiguration>,
    ) -> Result<(), AppError> {
        info!("Loading plugin: {} from {:?}", plugin_name, path);

        let wasm_bytes = std::fs::read(path).map_err(|e| {
            AppError::internal(format!("Failed to read WASM file {:?}: {}", path, e))
        })?;

        self.load_plugin_bytes(plugin_name, &wasm_bytes, config)
            .await
    }

    async fn load_plugin_bytes(
        &self,
        plugin_name: &str,
        wasm_bytes: &[u8],
        config: Option<&oxide_core::plugin_config::PluginConfiguration>,
    ) -> Result<(), AppError> {
        self.ensure_code_signing_policy(plugin_name, config)?;

        // Determine the expected hash (if any) from the provided configuration
        let expected_hash: Option<String> = config.and_then(|c| c.wasm_hash.clone());

        // Verify WASM integrity – abort loading on mismatch
        self.verify_plugin_hash(plugin_name, wasm_bytes, expected_hash.as_deref())
            .await?;

        let mut runtime_guard = self
            .runtime
            .lock()
            .map_err(|_| AppError::internal("Failed to acquire plugin runtime lock"))?;

        // Use plugin configuration if provided, otherwise fall back to safe defaults
        let (capabilities, trust_level, resource_limits) = if let Some(plugin_config) = config {
            if plugin_config.capabilities.is_empty() {
                warn!(
                    "❌ Plugin '{}' has no capabilities granted – skipping load",
                    plugin_name
                );
                return Err(AppError::plugin(
                    plugin_name.to_string(),
                    "No capabilities granted".to_string(),
                ));
            }

            info!(
                "🔍 Using provided configuration for plugin '{}': {} capabilities",
                plugin_name,
                plugin_config.capabilities.len()
            );
            (
                plugin_config.capabilities.clone(),
                plugin_config.trust_level.clone(),
                plugin_config.resource_limits.clone(),
            )
        } else {
            info!(
                "🔄 No configuration provided, using default capabilities for plugin '{}'",
                plugin_name
            );
            (
                Self::get_default_capabilities(),
                PluginTrustLevel::FullyTrusted,
                ResourceLimits::default(),
            )
        };

        runtime_guard
            .load_plugin_with_trust(
                plugin_name,
                wasm_bytes,
                trust_level,
                capabilities,
                resource_limits,
            )
            .map_err(|e| AppError::internal(format!("Failed to load plugin: {}", e)))?;

        info!("✅ Plugin '{}' loaded successfully", plugin_name);
        Ok(())
    }

    /// Load a plugin with explicit configuration
    pub async fn load_plugin_with_config(
        &mut self,
        plugin_name: &str,
        path: &PathBuf,
        config: &oxide_core::plugin_config::PluginConfiguration,
    ) -> Result<(), AppError> {
        self.load_single_plugin(plugin_name, path, Some(config))
            .await
    }

    /// Load a plugin from already-read bytes using the persisted configuration.
    ///
    /// This is useful for callers that need to validate and read the WASM file
    /// through their own storage service before handing it to the runtime.
    pub async fn load_plugin_bytes_with_config(
        &self,
        plugin_name: &str,
        wasm_bytes: &[u8],
        config: &oxide_core::plugin_config::PluginConfiguration,
    ) -> Result<(), AppError> {
        self.load_plugin_bytes(plugin_name, wasm_bytes, Some(config))
            .await
    }

    /// Get default capabilities for development/fallback scenarios
    fn get_default_capabilities() -> Vec<PluginCapability> {
        vec![
            PluginCapability::LogInfo,
            PluginCapability::LogError,
            PluginCapability::ReadEventData,
            PluginCapability::ModifyEventData,
            PluginCapability::RegisterHttpRoutes {
                path_patterns: vec!["*".to_string()],
                methods: vec!["*".to_string()],
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
        ]
    }

    /// Verify plugin hash for integrity checking
    async fn verify_plugin_hash(
        &self,
        plugin_name: &str,
        wasm_bytes: &[u8],
        expected_hash: Option<&str>,
    ) -> Result<(), AppError> {
        use sha2::{Digest, Sha256};

        // Calculate SHA-256 hash of the WASM file
        let mut hasher = Sha256::new();
        hasher.update(wasm_bytes);
        let calculated_hash = hex::encode(hasher.finalize());

        match expected_hash {
            Some(expected) => {
                if calculated_hash != expected {
                    warn!(
                        "❌ Hash mismatch for plugin '{}': expected {}, got {}",
                        plugin_name, expected, calculated_hash
                    );
                    return Err(AppError::plugin(
                        plugin_name.to_string(),
                        "WASM integrity check failed".to_string(),
                    ));
                } else {
                    info!("🔒 Plugin '{}' hash verified successfully", plugin_name);
                }
            }
            None => {
                info!(
                    "ℹ️ No stored hash for plugin '{}' – skipping integrity verification",
                    plugin_name
                );
            }
        }

        Ok(())
    }

    fn ensure_code_signing_policy(
        &self,
        plugin_name: &str,
        config: Option<&oxide_core::plugin_config::PluginConfiguration>,
    ) -> Result<(), AppError> {
        if !self.requires_code_signing() || plugin_signature_verified(config) {
            return Ok(());
        }

        Err(AppError::security(format!(
            "Plugin '{}' cannot be loaded because code signing is required and no verified signature is recorded",
            plugin_name
        )))
    }
}

fn plugin_signature_verified(
    config: Option<&oxide_core::plugin_config::PluginConfiguration>,
) -> bool {
    config
        .and_then(|config| config.metadata.get("signature_verified"))
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

/// Bridge to connect WASM plugins with the EventBus system
pub struct PluginEventBridge {
    plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>,
    host_state: HostStateRef,
}

impl PluginEventBridge {
    /// Create a new plugin event bridge
    pub fn new(plugin_runtime: Arc<Mutex<WasmtimePluginRuntime>>) -> Self {
        let host_state = match plugin_runtime.lock() {
            Ok(runtime) => runtime.get_host_state(),
            Err(_) => {
                warn!(
                    "Failed to acquire plugin runtime lock while creating event bridge; using isolated host state"
                );
                Arc::new(Mutex::new(HostState::default()))
            }
        };

        Self {
            plugin_runtime,
            host_state,
        }
    }

    /// Create an event handler that calls the plugin for Before record events.
    pub fn create_before_handler(
        &self,
        plugin_name: String,
        function_name: &'static str,
    ) -> BeforeHandler {
        let runtime = Arc::clone(&self.plugin_runtime);
        let host_state = Arc::clone(&self.host_state);

        Arc::new(move |context: &mut BeforeEventContext| {
            let runtime = Arc::clone(&runtime);
            let host_state = Arc::clone(&host_state);
            let plugin_name = plugin_name.clone();
            let function_name = function_name.to_string();
            Box::pin(async move {
                info!(
                    "🔌 Calling plugin '{}' for before event function '{}'",
                    plugin_name, function_name
                );

                // Check if we can acquire the runtime lock without blocking. A plugin HTTP
                // handler can trigger database events while it already owns the runtime lock;
                // skip only that recursive same-plugin case and keep other busy states fail-closed.
                let mut runtime_guard = match runtime.try_lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        if Self::is_same_plugin_http_execution(&host_state, &plugin_name) {
                            info!(
                                "🔌 Plugin '{}' is handling HTTP request, skipping before event '{}' to prevent recursion",
                                plugin_name, function_name
                            );
                            return Ok(());
                        }

                        return Err(AppError::plugin(
                            plugin_name.clone(),
                            "Plugin runtime is busy; before hook failed closed".to_string(),
                        ));
                    }
                };

                if !runtime_guard
                    .list_plugins()
                    .iter()
                    .any(|loaded_plugin| loaded_plugin == &plugin_name)
                {
                    info!(
                        "🔌 Plugin '{}' is not loaded, skipping before event handler",
                        plugin_name
                    );
                    return Ok(());
                }

                if runtime_guard.is_plugin_suspended(&plugin_name) {
                    info!(
                        "🔌 Plugin '{}' is suspended, skipping before event handler",
                        plugin_name
                    );
                    return Ok(());
                }

                if !runtime_guard.has_function(&plugin_name, &function_name) {
                    info!(
                        "🔌 Plugin '{}' does not export '{}', skipping before event handler",
                        plugin_name, function_name
                    );
                    return Ok(());
                }

                // Check if this plugin is currently handling an HTTP request
                // If so, skip the event handler to prevent recursive calls
                if Self::is_same_plugin_http_execution(&host_state, &plugin_name) {
                    info!(
                        "🔌 Plugin '{}' is handling HTTP request, skipping before event '{}' to prevent recursion",
                        plugin_name, function_name
                    );
                    return Ok(());
                }

                // Convert BeforeEventContext to EventPayload
                let payload = Self::before_context_to_payload(&function_name, context);

                // Call the plugin
                match runtime_guard.call_plugin_function(&plugin_name, &function_name, &payload) {
                    Ok(response) => {
                        info!(
                            "🔌 Plugin '{}' response: allow={}, error={:?}",
                            plugin_name, response.allow, response.error_message
                        );

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
                            let error_msg = response
                                .error_message
                                .unwrap_or_else(|| "Plugin rejected the operation".to_string());
                            return Err(AppError::plugin(&plugin_name, &error_msg));
                        }

                        Ok(())
                    }
                    Err(e) => {
                        if matches!(e, PluginError::PluginNotFound(_)) {
                            info!(
                                "🔌 Plugin '{}' disappeared before event execution, skipping",
                                plugin_name
                            );
                            return Ok(());
                        }

                        error!("🔌 Plugin '{}' execution failed: {}", plugin_name, e);
                        Err(AppError::plugin(
                            plugin_name,
                            format!("Plugin execution failed: {}", e),
                        ))
                    }
                }
            })
        })
    }

    fn is_same_plugin_http_execution(host_state: &HostStateRef, plugin_name: &str) -> bool {
        let Some(state) = lock_host_state(host_state, "checking current plugin HTTP execution")
        else {
            return false;
        };

        state
            .current_plugin
            .as_deref()
            .is_some_and(|current_plugin| current_plugin == plugin_name)
            && state.current_http_request.is_some()
    }

    /// Create an event handler that calls the plugin for After record events.
    pub fn create_after_handler(
        &self,
        plugin_name: String,
        function_name: &'static str,
    ) -> AfterHandler {
        let runtime = Arc::clone(&self.plugin_runtime);

        Arc::new(move |context: &AfterEventContext| {
            let runtime = Arc::clone(&runtime);
            let plugin_name = plugin_name.clone();
            let function_name = function_name.to_string();
            Box::pin(async move {
                info!(
                    "🔌 Calling plugin '{}' for after event function '{}'",
                    plugin_name, function_name
                );

                let mut runtime_guard = match runtime.try_lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        info!(
                            "🔌 Plugin '{}' is already executing, skipping after event to avoid deadlock",
                            plugin_name
                        );
                        return Ok(());
                    }
                };

                if !runtime_guard
                    .list_plugins()
                    .iter()
                    .any(|loaded_plugin| loaded_plugin == &plugin_name)
                {
                    info!(
                        "🔌 Plugin '{}' is not loaded, skipping after event handler",
                        plugin_name
                    );
                    return Ok(());
                }

                if runtime_guard.is_plugin_suspended(&plugin_name) {
                    info!(
                        "🔌 Plugin '{}' is suspended, skipping after event handler",
                        plugin_name
                    );
                    return Ok(());
                }

                if !runtime_guard.has_function(&plugin_name, &function_name) {
                    info!(
                        "🔌 Plugin '{}' does not export '{}', skipping after event handler",
                        plugin_name, function_name
                    );
                    return Ok(());
                }

                let payload = Self::after_context_to_payload(&function_name, context)?;

                match runtime_guard.call_plugin_function(&plugin_name, &function_name, &payload) {
                    Ok(response) => {
                        info!(
                            "🔌 Plugin '{}' after response: allow={}, error={:?}",
                            plugin_name, response.allow, response.error_message
                        );
                        Ok(())
                    }
                    Err(e) => {
                        if matches!(e, PluginError::PluginNotFound(_)) {
                            info!(
                                "🔌 Plugin '{}' disappeared before after event execution, skipping",
                                plugin_name
                            );
                            return Ok(());
                        }

                        error!("🔌 Plugin '{}' after execution failed: {}", plugin_name, e);
                        Err(AppError::plugin(
                            plugin_name,
                            format!("Plugin after execution failed: {}", e),
                        ))
                    }
                }
            })
        })
    }

    fn before_context_to_payload(
        function_name: &str,
        context: &BeforeEventContext,
    ) -> EventPayload {
        let event_type = match function_name {
            plugin_exports::ON_BEFORE_CREATE => "BeforeRecordCreate",
            plugin_exports::ON_BEFORE_UPDATE => "BeforeRecordUpdate",
            plugin_exports::ON_BEFORE_DELETE => "BeforeRecordDelete",
            other => other,
        };

        EventPayload {
            event_type: event_type.to_string(),
            collection: context.collection.clone(),
            data: context.data.to_string(),
            metadata: serde_json::json!({
                "event_id": context.event_id.clone(),
                "timestamp": context.timestamp,
                "record_id": context.record_id.clone(),
                "old_data": context.old_data.clone(),
                "metadata": context.metadata.clone(),
            }),
        }
    }

    fn after_context_to_payload(
        function_name: &str,
        context: &AfterEventContext,
    ) -> Result<EventPayload, AppError> {
        let event_type = match function_name {
            plugin_exports::ON_AFTER_CREATE => "AfterRecordCreate",
            plugin_exports::ON_AFTER_UPDATE => "AfterRecordUpdate",
            plugin_exports::ON_AFTER_DELETE => "AfterRecordDelete",
            other => other,
        };

        let (collection, data, metadata) = match context {
            AfterEventContext::RecordCreated {
                event_id,
                timestamp,
                collection,
                record_id,
                data,
                ..
            } => (
                collection.clone(),
                data.clone(),
                serde_json::json!({
                    "event_id": event_id,
                    "timestamp": timestamp,
                    "record_id": record_id,
                }),
            ),
            AfterEventContext::RecordUpdated {
                event_id,
                timestamp,
                collection,
                record_id,
                old_data,
                new_data,
                ..
            } => (
                collection.clone(),
                new_data.clone(),
                serde_json::json!({
                    "event_id": event_id,
                    "timestamp": timestamp,
                    "record_id": record_id,
                    "old_data": old_data,
                }),
            ),
            AfterEventContext::RecordDeleted {
                event_id,
                timestamp,
                collection,
                record_id,
                data,
                ..
            } => (
                collection.clone(),
                data.clone(),
                serde_json::json!({
                    "event_id": event_id,
                    "timestamp": timestamp,
                    "record_id": record_id,
                }),
            ),
            _ => {
                return Err(AppError::internal(format!(
                    "Unsupported after event context for plugin function '{}'",
                    function_name
                )))
            }
        };

        Ok(EventPayload {
            event_type: event_type.to_string(),
            collection,
            data: data.to_string(),
            metadata,
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
    pub last_execution: Option<u64>,
    pub total_execution_time_ms: u64,
    pub host_function_calls: u64,
    pub peak_memory_usage: u64,
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
    use oxide_core::auth::{AuthService, AuthServiceConfig};
    use oxide_core::event::{
        AfterEventHandler, BeforeEventHandler, EventBusHealth, EventFilter, EventMetrics,
        HandlerExecutionResult, HandlerMetadata,
    };
    use oxide_core::plugin_config::PluginConfiguration;
    use oxide_core::plugin_security::SecurityPolicies;
    use oxide_core::InMemoryEventBus;
    use oxide_db::SqliteDb;
    use std::collections::HashMap;

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
        let runtime =
            WasmtimePluginRuntime::new_with_security_policies(Arc::new(db), policies).unwrap();
        let runtime = Arc::new(Mutex::new(runtime));
        let bridge = PluginEventBridge::new(runtime);

        // Bridge should be created successfully
        assert!(std::ptr::addr_of!(bridge) as usize > 0);
    }

    fn test_runtime_and_bridge() -> (Arc<Mutex<WasmtimePluginRuntime>>, PluginEventBridge) {
        let policies = SecurityPolicies::default();
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let event_bus = Arc::new(InMemoryEventBus::new());
        let db = SqliteDb::new(":memory:", event_bus, auth_service).unwrap();
        let runtime =
            WasmtimePluginRuntime::new_with_security_policies(Arc::new(db), policies).unwrap();
        let runtime = Arc::new(Mutex::new(runtime));
        let bridge = PluginEventBridge::new(Arc::clone(&runtime));
        (runtime, bridge)
    }

    fn sample_http_request_context() -> oxide_core::plugin_api::HttpRequestContext {
        oxide_core::plugin_api::HttpRequestContext {
            method: "POST".to_string(),
            path: "/api/hello/settings".to_string(),
            query_params: HashMap::new(),
            headers: HashMap::new(),
            body: None,
            path_params: HashMap::new(),
            user: None,
        }
    }

    fn sample_before_update_context() -> BeforeEventContext {
        BeforeEventContext::new_update(
            "_plugins".to_string(),
            "plugin-record".to_string(),
            serde_json::json!({"name": "hello-plugin"}),
            serde_json::json!({"name": "hello-plugin"}),
        )
    }

    #[test]
    fn busy_before_handler_skips_same_plugin_http_recursion() {
        let (runtime, bridge) = test_runtime_and_bridge();
        {
            let mut state = bridge.host_state.lock().unwrap();
            state.current_plugin = Some("hello-plugin".to_string());
            state.current_http_request = Some(sample_http_request_context());
        }

        let handler = bridge.create_before_handler("hello-plugin".to_string(), "on_before_update");
        let _runtime_guard = runtime.lock().unwrap();
        let mut context = sample_before_update_context();
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = tokio_runtime.block_on(handler(&mut context));

        assert!(result.is_ok());
    }

    #[test]
    fn busy_before_handler_fails_closed_for_unrelated_execution() {
        let (runtime, bridge) = test_runtime_and_bridge();
        {
            let mut state = bridge.host_state.lock().unwrap();
            state.current_plugin = Some("other-plugin".to_string());
            state.current_http_request = Some(sample_http_request_context());
        }

        let handler = bridge.create_before_handler("hello-plugin".to_string(), "on_before_update");
        let _runtime_guard = runtime.lock().unwrap();
        let mut context = sample_before_update_context();
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let result = tokio_runtime.block_on(handler(&mut context));

        assert!(matches!(result, Err(AppError::Plugin { .. })));
    }

    fn test_manager_with_policies(policies: SecurityPolicies) -> PluginManager {
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let event_bus = Arc::new(InMemoryEventBus::new());
        let db = SqliteDb::new(":memory:", event_bus, auth_service).unwrap();

        PluginManager::new(Arc::new(db), policies).unwrap()
    }

    fn plugin_config_with_signature(signature_verified: bool) -> PluginConfiguration {
        let mut config = PluginConfiguration::new(
            "test_plugin".to_string(),
            "1.0.0".to_string(),
            "Test plugin".to_string(),
            "Test Author".to_string(),
            PluginTrustLevel::PartiallyTrusted,
            vec![PluginCapability::LogInfo],
            ResourceLimits::default(),
            Some("test_plugin.wasm".to_string()),
            Some(128),
            Some("hash".to_string()),
        );
        config.set_metadata(serde_json::json!({
            "signature_verified": signature_verified
        }));
        config
    }

    const MINIMAL_PLUGIN_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x06, 0x01, 0x60, 0x00, 0x01, 0x7f,
        0x03, 0x02, 0x01, 0x00, 0x07, 0x0f, 0x01, 0x0b, 0x70, 0x6c, 0x75, 0x67, 0x69, 0x6e, 0x5f,
        0x69, 0x6e, 0x69, 0x74, 0x00, 0x00, 0x0a, 0x06, 0x01, 0x04, 0x00, 0x41, 0x00, 0x0b,
    ];

    struct FailingRegistrationEventBus;

    #[async_trait::async_trait]
    impl EventBus for FailingRegistrationEventBus {
        async fn dispatch_before(
            &self,
            _event_type: BeforeEventType,
            _context: &mut BeforeEventContext,
        ) -> Result<Vec<HandlerExecutionResult>, AppError> {
            Ok(Vec::new())
        }

        async fn dispatch_after(
            &self,
            _event_type: AfterEventType,
            _context: &AfterEventContext,
        ) -> Result<Vec<HandlerExecutionResult>, AppError> {
            Ok(Vec::new())
        }

        async fn subscribe_before(
            &self,
            _event_name: &str,
            _handler: BeforeEventHandler,
            _metadata: HandlerMetadata,
        ) -> Result<String, AppError> {
            Err(AppError::internal("registration failed"))
        }

        async fn subscribe_after(
            &self,
            _event_name: &str,
            _handler: AfterEventHandler,
            _metadata: HandlerMetadata,
        ) -> Result<String, AppError> {
            Err(AppError::internal("registration failed"))
        }

        async fn unsubscribe_before(
            &self,
            _event_name: &str,
            _handler_id: &str,
        ) -> Result<bool, AppError> {
            Ok(false)
        }

        async fn unsubscribe_after(
            &self,
            _event_name: &str,
            _handler_id: &str,
        ) -> Result<bool, AppError> {
            Ok(false)
        }

        async fn set_handler_enabled(
            &self,
            _handler_id: &str,
            _enabled: bool,
        ) -> Result<bool, AppError> {
            Ok(false)
        }

        fn before_listener_count(&self, _event_name: &str) -> usize {
            0
        }

        fn after_listener_count(&self, _event_name: &str) -> usize {
            0
        }

        fn list_handlers(&self) -> HashMap<String, Vec<HandlerMetadata>> {
            HashMap::new()
        }

        fn metrics(&self) -> EventMetrics {
            EventMetrics::default()
        }

        async fn add_filter(&self, _filter: Box<dyn EventFilter>) -> Result<String, AppError> {
            Ok("test-filter".to_string())
        }

        async fn remove_filter(&self, _filter_id: &str) -> Result<bool, AppError> {
            Ok(false)
        }

        fn health_status(&self) -> EventBusHealth {
            EventBusHealth::healthy()
        }

        async fn shutdown(&self) -> Result<(), AppError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn load_and_register_rolls_back_runtime_when_event_registration_fails() {
        let policies = SecurityPolicies {
            require_code_signing: false,
            allow_untrusted_plugins: true,
            ..Default::default()
        };
        let manager = test_manager_with_policies(policies);
        let event_bus: Arc<dyn EventBus> = Arc::new(FailingRegistrationEventBus);
        let config = PluginConfiguration::new(
            "rollback_plugin".to_string(),
            "1.0.0".to_string(),
            "Rollback test plugin".to_string(),
            "OxideDB".to_string(),
            PluginTrustLevel::PartiallyTrusted,
            vec![PluginCapability::LogInfo],
            ResourceLimits::default(),
            None,
            Some(MINIMAL_PLUGIN_WASM.len() as u64),
            None,
        );

        let result = manager
            .load_and_register_plugin(&event_bus, &config, MINIMAL_PLUGIN_WASM)
            .await;

        assert!(matches!(result, Err(AppError::Internal { .. })));
        match manager.get_loaded_plugin_count() {
            Ok(count) => assert_eq!(count, 0),
            Err(error) => panic!("failed to inspect loaded plugin count: {}", error),
        }
    }

    #[test]
    fn code_signing_policy_requires_recorded_verified_signature() {
        let manager = test_manager_with_policies(SecurityPolicies::default());
        let verified_config = plugin_config_with_signature(true);
        let unverified_config = plugin_config_with_signature(false);

        assert!(manager.requires_code_signing());
        assert!(manager
            .ensure_code_signing_policy("signed_plugin", Some(&verified_config))
            .is_ok());
        assert!(matches!(
            manager.ensure_code_signing_policy("unsigned_plugin", None),
            Err(AppError::Security { .. })
        ));
        assert!(matches!(
            manager.ensure_code_signing_policy("unverified_plugin", Some(&unverified_config)),
            Err(AppError::Security { .. })
        ));
    }

    #[test]
    fn relaxed_policy_allows_unsigned_plugins() {
        let policies = SecurityPolicies {
            require_code_signing: false,
            allow_untrusted_plugins: true,
            ..Default::default()
        };
        let manager = test_manager_with_policies(policies);

        assert!(!manager.requires_code_signing());
        assert!(manager
            .ensure_code_signing_policy("unsigned_plugin", None)
            .is_ok());
    }
}
