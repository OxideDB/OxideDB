//! Plugin Runtime Factory for Wasmtime
//!
//! This module provides a factory implementation for creating
//! WasmtimePluginRuntime instances based on configuration.

use crate::runtime::WasmtimePluginRuntime;
use oxide_core::{
    plugin_api::{PluginResult, PluginRuntimeConfig, PluginRuntimeFactory},
    plugin_security::SecurityPolicies,
};
use oxide_db::Db;
use std::sync::Arc;

/// Factory for creating Wasmtime-based plugin runtime instances
pub struct WasmtimePluginRuntimeFactory {
    database: Arc<dyn Db>,
}

impl WasmtimePluginRuntimeFactory {
    /// Create a new factory instance with database
    pub fn new(database: Arc<dyn Db>) -> Self {
        Self { database }
    }
}

impl PluginRuntimeFactory for WasmtimePluginRuntimeFactory {
    type Runtime = WasmtimePluginRuntime;

    fn create_runtime(&self) -> PluginResult<Self::Runtime> {
        WasmtimePluginRuntime::new(self.database.clone())
    }

    fn runtime_type(&self) -> &'static str {
        "wasmtime"
    }

    fn supports_config(&self, config: &PluginRuntimeConfig) -> bool {
        matches!(config.runtime_type.as_str(), "wasmtime" | "wasmtime-wasi")
    }
}

/// Factory for creating Wasmtime runtime instances with custom security policies
pub struct WasmtimePluginRuntimeFactoryWithPolicies {
    database: Arc<dyn Db>,
    policies: SecurityPolicies,
}

impl WasmtimePluginRuntimeFactoryWithPolicies {
    /// Create a new factory with security policies
    pub fn new(database: Arc<dyn Db>, policies: SecurityPolicies) -> Self {
        Self { database, policies }
    }

    /// Create a factory from configuration
    pub fn from_config(database: Arc<dyn Db>, config: &PluginRuntimeConfig) -> PluginResult<Self> {
        let policies = if let Some(security_config) = &config.security_policies {
            serde_json::from_value(security_config.clone())
                .map_err(|e| oxide_core::plugin_api::PluginError::InitializationFailed(
                    format!("Failed to parse security policies: {}", e)
                ))?
        } else {
            SecurityPolicies::default()
        };

        Ok(Self::new(database, policies))
    }
}

impl PluginRuntimeFactory for WasmtimePluginRuntimeFactoryWithPolicies {
    type Runtime = WasmtimePluginRuntime;

    fn create_runtime(&self) -> PluginResult<Self::Runtime> {
        WasmtimePluginRuntime::new_with_security_policies(self.database.clone(), self.policies.clone())
    }

    fn runtime_type(&self) -> &'static str {
        "wasmtime-secure"
    }

    fn supports_config(&self, config: &PluginRuntimeConfig) -> bool {
        matches!(config.runtime_type.as_str(), "wasmtime" | "wasmtime-wasi" | "wasmtime-secure")
    }
} 