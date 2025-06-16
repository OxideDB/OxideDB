//! Plugin Security and Capability-Based Access Control
//!
//! This module implements a comprehensive security framework for OxideDB plugins,
//! providing capability-based access control to ensure untrusted plugins cannot
//! perform unauthorized operations or access sensitive data.
//!
//! ## Security Model
//!
//! The security model is based on the principle of least privilege:
//! - Plugins start with zero capabilities
//! - Capabilities must be explicitly granted
//! - Each capability grants access to specific host functions or data
//! - Capabilities can be revoked at runtime
//! - All plugin operations are audited

use crate::{AppError, auth::{UserRole, CrudOperation}};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn, error};

/// Capability types that can be granted to plugins
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginCapability {
    /// Allow plugin to log messages (info level)
    LogInfo,
    /// Allow plugin to log error messages
    LogError,
    /// Allow plugin to read event payload data
    ReadEventData,
    /// Allow plugin to modify event data
    ModifyEventData,
    /// Allow plugin to prevent operations from continuing
    BlockOperations,
    /// Allow plugin to read configuration values
    ReadConfig {
        /// Specific config keys the plugin can access
        keys: Vec<String>,
    },
    /// Allow plugin to access specific collections
    AccessCollection {
        /// Collection name
        collection: String,
        /// Operations allowed on this collection
        operations: Vec<CrudOperation>,
    },
    /// Allow plugin to make HTTP requests (with URL restrictions)
    HttpRequest {
        /// Allowed URL patterns (regex)
        allowed_urls: Vec<String>,
        /// Maximum requests per minute
        rate_limit: u32,
    },
    /// Allow plugin to store/retrieve persistent data
    PersistentStorage {
        /// Maximum storage size in bytes
        max_size: u64,
        /// Allowed key prefixes
        key_prefixes: Vec<String>,
    },
    /// Allow plugin to schedule tasks
    ScheduleTasks,
    /// Allow plugin to emit custom events
    EmitEvents {
        /// Allowed event types
        event_types: Vec<String>,
    },
}

/// Security context for a plugin execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSecurityContext {
    /// Plugin name
    pub plugin_name: String,
    /// Granted capabilities
    pub capabilities: HashSet<PluginCapability>,
    /// Trust level of the plugin
    pub trust_level: PluginTrustLevel,
    /// Resource usage limits
    pub resource_limits: ResourceLimits,
    /// Execution statistics
    pub stats: ExecutionStats,
    /// Security violations count
    pub violations: u32,
    /// Whether the plugin is currently suspended
    pub suspended: bool,
}

/// Trust level assigned to plugins
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginTrustLevel {
    /// Untrusted plugin (default for external plugins)
    Untrusted,
    /// Partially trusted (verified but limited)
    PartiallyTrusted,
    /// Fully trusted (internal/verified plugins)
    FullyTrusted,
    /// System plugin (core functionality)
    System,
}

/// Resource usage limits for plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum memory usage in bytes
    pub max_memory: u64,
    /// Maximum execution time per call in milliseconds
    pub max_execution_time: u64,
    /// Maximum number of host function calls per execution
    pub max_host_calls: u32,
    /// Maximum number of executions per minute
    pub rate_limit: u32,
}

/// Execution statistics for monitoring
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionStats {
    /// Total number of executions
    pub total_executions: u64,
    /// Total execution time in milliseconds
    pub total_execution_time: u64,
    /// Number of host function calls
    pub host_function_calls: u64,
    /// Memory usage high watermark
    pub peak_memory_usage: u64,
    /// Last execution timestamp
    pub last_execution: Option<u64>,
    /// Number of failed executions
    pub failed_executions: u64,
}

/// Security violation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityViolation {
    /// Attempted to call unauthorized host function
    UnauthorizedHostFunction {
        function_name: String,
        required_capability: PluginCapability,
    },
    /// Exceeded resource limits
    ResourceLimitExceeded {
        limit_type: String,
        limit_value: u64,
        actual_value: u64,
    },
    /// Attempted to access unauthorized data
    UnauthorizedDataAccess {
        data_type: String,
        required_capability: PluginCapability,
    },
    /// Malicious behavior detected
    MaliciousBehavior {
        description: String,
    },
}

/// Plugin security manager
pub struct PluginSecurityManager {
    /// Security contexts for loaded plugins
    contexts: HashMap<String, PluginSecurityContext>,
    /// Global security policies
    policies: SecurityPolicies,
    /// Audit log
    audit_log: Vec<SecurityAuditEntry>,
}

/// Global security policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityPolicies {
    /// Default trust level for new plugins
    pub default_trust_level: PluginTrustLevel,
    /// Default resource limits
    pub default_resource_limits: ResourceLimits,
    /// Maximum number of violations before suspension
    pub max_violations_before_suspension: u32,
    /// Whether to allow untrusted plugins
    pub allow_untrusted_plugins: bool,
    /// Require code signing for plugins
    pub require_code_signing: bool,
}

/// Security audit entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditEntry {
    /// Timestamp
    pub timestamp: u64,
    /// Plugin name
    pub plugin_name: String,
    /// Event type
    pub event_type: SecurityEventType,
    /// Additional details
    pub details: serde_json::Value,
}

/// Security event types for auditing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEventType {
    /// Plugin loaded
    PluginLoaded,
    /// Plugin unloaded
    PluginUnloaded,
    /// Capability granted
    CapabilityGranted,
    /// Capability revoked
    CapabilityRevoked,
    /// Security violation
    SecurityViolation,
    /// Plugin suspended
    PluginSuspended,
    /// Plugin resumed
    PluginResumed,
    /// Resource limit exceeded
    ResourceLimitExceeded,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory: 16 * 1024 * 1024, // 16MB
            max_execution_time: 5000,      // 5 seconds
            max_host_calls: 1000,          // 1000 calls per execution
            rate_limit: 60,                // 60 executions per minute
        }
    }
}

impl Default for SecurityPolicies {
    fn default() -> Self {
        Self {
            default_trust_level: PluginTrustLevel::Untrusted,
            default_resource_limits: ResourceLimits::default(),
            max_violations_before_suspension: 5,
            allow_untrusted_plugins: false, // Secure by default
            require_code_signing: true,
        }
    }
}

impl PluginSecurityManager {
    /// Create a new security manager
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            policies: SecurityPolicies::default(),
            audit_log: Vec::new(),
        }
    }

    /// Create a security manager with custom policies
    pub fn with_policies(policies: SecurityPolicies) -> Self {
        Self {
            contexts: HashMap::new(),
            policies,
            audit_log: Vec::new(),
        }
    }

    /// Register a new plugin with security context
    pub fn register_plugin(
        &mut self,
        plugin_name: String,
        trust_level: Option<PluginTrustLevel>,
    ) -> Result<(), AppError> {
        let trust_level = trust_level.unwrap_or_else(|| self.policies.default_trust_level.clone());
        
        // Check if untrusted plugins are allowed
        if trust_level == PluginTrustLevel::Untrusted && !self.policies.allow_untrusted_plugins {
            return Err(AppError::Security {
                message: "Untrusted plugins are not allowed by security policy".to_string(),
            });
        }

        let context = PluginSecurityContext {
            plugin_name: plugin_name.clone(),
            capabilities: HashSet::new(),
            trust_level: trust_level.clone(),
            resource_limits: self.get_resource_limits_for_trust_level(&trust_level),
            stats: ExecutionStats::default(),
            violations: 0,
            suspended: false,
        };

        self.contexts.insert(plugin_name.clone(), context);
        self.audit_event(
            plugin_name,
            SecurityEventType::PluginLoaded,
            serde_json::json!({ "trust_level": trust_level }),
        );

        Ok(())
    }

    /// Grant a capability to a plugin
    pub fn grant_capability(
        &mut self,
        plugin_name: &str,
        capability: PluginCapability,
    ) -> Result<(), AppError> {
        // First check if capability is allowed for this trust level
        let trust_level = {
            let context = self.contexts.get(plugin_name)
                .ok_or_else(|| AppError::Plugin {
                    plugin_name: plugin_name.to_string(),
                    message: "Plugin not registered".to_string(),
                })?;
            context.trust_level.clone()
        };

        if !self.is_capability_allowed(&trust_level, &capability) {
            return Err(AppError::Security {
                message: format!(
                    "Capability {:?} not allowed for trust level {:?}",
                    capability, trust_level
                ),
            });
        }

        // Now get mutable reference and add capability
        let context = self.contexts.get_mut(plugin_name)
            .ok_or_else(|| AppError::Plugin {
                plugin_name: plugin_name.to_string(),
                message: "Plugin not registered".to_string(),
            })?;

        context.capabilities.insert(capability.clone());
        self.audit_event(
            plugin_name.to_string(),
            SecurityEventType::CapabilityGranted,
            serde_json::json!({ "capability": capability }),
        );

        debug!("Granted capability {:?} to plugin {}", capability, plugin_name);
        Ok(())
    }

    /// Revoke a capability from a plugin
    pub fn revoke_capability(
        &mut self,
        plugin_name: &str,
        capability: &PluginCapability,
    ) -> Result<(), AppError> {
        let context = self.contexts.get_mut(plugin_name)
            .ok_or_else(|| AppError::Plugin {
                plugin_name: plugin_name.to_string(),
                message: "Plugin not registered".to_string(),
            })?;

        context.capabilities.remove(capability);
        self.audit_event(
            plugin_name.to_string(),
            SecurityEventType::CapabilityRevoked,
            serde_json::json!({ "capability": capability }),
        );

        debug!("Revoked capability {:?} from plugin {}", capability, plugin_name);
        Ok(())
    }

    /// Check if a plugin has a specific capability
    pub fn has_capability(
        &self,
        plugin_name: &str,
        capability: &PluginCapability,
    ) -> Result<bool, AppError> {
        let context = self.contexts.get(plugin_name)
            .ok_or_else(|| AppError::Plugin {
                plugin_name: plugin_name.to_string(),
                message: "Plugin not registered".to_string(),
            })?;

        if context.suspended {
            return Ok(false);
        }

        Ok(context.capabilities.contains(capability))
    }

    /// Validate a host function call
    pub fn validate_host_function_call(
        &mut self,
        plugin_name: &str,
        function_name: &str,
    ) -> Result<(), AppError> {
        let required_capability = self.get_required_capability_for_function(function_name)?;
        
        if !self.has_capability(plugin_name, &required_capability)? {
            self.record_violation(
                plugin_name,
                SecurityViolation::UnauthorizedHostFunction {
                    function_name: function_name.to_string(),
                    required_capability: required_capability.clone(),
                },
            )?;
            
            return Err(AppError::Security {
                message: format!(
                    "Plugin {} does not have capability {:?} required for function {}",
                    plugin_name, required_capability, function_name
                ),
            });
        }

        // Update statistics
        if let Some(context) = self.contexts.get_mut(plugin_name) {
            context.stats.host_function_calls += 1;
        }

        Ok(())
    }

    /// Record a security violation
    pub fn record_violation(
        &mut self,
        plugin_name: &str,
        violation: SecurityViolation,
    ) -> Result<(), AppError> {
        warn!("Security violation by plugin {}: {:?}", plugin_name, violation);

        let should_suspend = {
            let context = self.contexts.get_mut(plugin_name)
                .ok_or_else(|| AppError::Plugin {
                    plugin_name: plugin_name.to_string(),
                    message: "Plugin not registered".to_string(),
                })?;

            context.violations += 1;
            context.violations >= self.policies.max_violations_before_suspension
        };
        
        self.audit_event(
            plugin_name.to_string(),
            SecurityEventType::SecurityViolation,
            serde_json::json!({ "violation": violation }),
        );

        // Check if plugin should be suspended
        if should_suspend {
            self.suspend_plugin(plugin_name)?;
        }

        Ok(())
    }

    /// Suspend a plugin due to security violations
    pub fn suspend_plugin(&mut self, plugin_name: &str) -> Result<(), AppError> {
        let violations_count = {
            let context = self.contexts.get_mut(plugin_name)
                .ok_or_else(|| AppError::Plugin {
                    plugin_name: plugin_name.to_string(),
                    message: "Plugin not registered".to_string(),
                })?;

            context.suspended = true;
            context.violations
        };
        
        self.audit_event(
            plugin_name.to_string(),
            SecurityEventType::PluginSuspended,
            serde_json::json!({ "violations": violations_count }),
        );

        error!("Plugin {} suspended due to {} security violations", plugin_name, violations_count);
        Ok(())
    }

    /// Resume a suspended plugin
    pub fn resume_plugin(&mut self, plugin_name: &str) -> Result<(), AppError> {
        let context = self.contexts.get_mut(plugin_name)
            .ok_or_else(|| AppError::Plugin {
                plugin_name: plugin_name.to_string(),
                message: "Plugin not registered".to_string(),
            })?;

        context.suspended = false;
        context.violations = 0; // Reset violation count
        
        self.audit_event(
            plugin_name.to_string(),
            SecurityEventType::PluginResumed,
            serde_json::json!({}),
        );

        debug!("Plugin {} resumed", plugin_name);
        Ok(())
    }

    /// Get security context for a plugin
    pub fn get_context(&self, plugin_name: &str) -> Option<&PluginSecurityContext> {
        self.contexts.get(plugin_name)
    }

    /// Get audit log entries
    pub fn get_audit_log(&self) -> &[SecurityAuditEntry] {
        &self.audit_log
    }

    /// Clear audit log (for maintenance)
    pub fn clear_audit_log(&mut self) {
        self.audit_log.clear();
    }

    /// Unregister a plugin
    pub fn unregister_plugin(&mut self, plugin_name: &str) {
        if self.contexts.remove(plugin_name).is_some() {
            self.audit_event(
                plugin_name.to_string(),
                SecurityEventType::PluginUnloaded,
                serde_json::json!({}),
            );
        }
    }

    /// Get resource limits based on trust level
    fn get_resource_limits_for_trust_level(&self, trust_level: &PluginTrustLevel) -> ResourceLimits {
        match trust_level {
            PluginTrustLevel::Untrusted => ResourceLimits {
                max_memory: 8 * 1024 * 1024,  // 8MB
                max_execution_time: 1000,     // 1 second
                max_host_calls: 100,          // 100 calls
                rate_limit: 10,               // 10 executions per minute
            },
            PluginTrustLevel::PartiallyTrusted => ResourceLimits {
                max_memory: 32 * 1024 * 1024, // 32MB
                max_execution_time: 5000,     // 5 seconds
                max_host_calls: 500,          // 500 calls
                rate_limit: 30,               // 30 executions per minute
            },
            PluginTrustLevel::FullyTrusted => ResourceLimits {
                max_memory: 128 * 1024 * 1024, // 128MB
                max_execution_time: 30000,     // 30 seconds
                max_host_calls: 5000,          // 5000 calls
                rate_limit: 120,               // 120 executions per minute
            },
            PluginTrustLevel::System => ResourceLimits {
                max_memory: u64::MAX,          // Unlimited
                max_execution_time: u64::MAX,  // Unlimited
                max_host_calls: u32::MAX,      // Unlimited
                rate_limit: u32::MAX,          // Unlimited
            },
        }
    }

    /// Check if a capability is allowed for a trust level
    fn is_capability_allowed(
        &self,
        trust_level: &PluginTrustLevel,
        capability: &PluginCapability,
    ) -> bool {
        match trust_level {
            PluginTrustLevel::Untrusted => {
                matches!(
                    capability,
                    PluginCapability::LogInfo
                        | PluginCapability::LogError
                        | PluginCapability::ReadEventData
                )
            }
            PluginTrustLevel::PartiallyTrusted => {
                !matches!(
                    capability,
                    PluginCapability::HttpRequest { .. }
                        | PluginCapability::ScheduleTasks
                )
            }
            PluginTrustLevel::FullyTrusted | PluginTrustLevel::System => true,
        }
    }

    /// Get required capability for a host function
    fn get_required_capability_for_function(
        &self,
        function_name: &str,
    ) -> Result<PluginCapability, AppError> {
        match function_name {
            "log_info" => Ok(PluginCapability::LogInfo),
            "log_error" => Ok(PluginCapability::LogError),
            "get_event_payload" => Ok(PluginCapability::ReadEventData),
            "set_error" => Ok(PluginCapability::BlockOperations),
            "get_config" => Ok(PluginCapability::ReadConfig {
                keys: vec!["*".to_string()], // Will be refined based on actual key
            }),
            _ => Err(AppError::Security {
                message: format!("Unknown host function: {}", function_name),
            }),
        }
    }

    /// Record an audit event
    fn audit_event(
        &mut self,
        plugin_name: String,
        event_type: SecurityEventType,
        details: serde_json::Value,
    ) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let entry = SecurityAuditEntry {
            timestamp,
            plugin_name,
            event_type,
            details,
        };

        self.audit_log.push(entry);

        // Keep audit log size manageable
        if self.audit_log.len() > 10000 {
            self.audit_log.drain(0..1000); // Remove oldest 1000 entries
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registration() {
        let mut manager = PluginSecurityManager::new();
        
        // Test registering a plugin
        assert!(manager.register_plugin(
            "test_plugin".to_string(),
            Some(PluginTrustLevel::PartiallyTrusted)
        ).is_ok());
        
        // Test plugin context exists
        assert!(manager.get_context("test_plugin").is_some());
    }

    #[test]
    fn test_capability_management() {
        let mut manager = PluginSecurityManager::new();
        manager.register_plugin(
            "test_plugin".to_string(),
            Some(PluginTrustLevel::PartiallyTrusted)
        ).unwrap();
        
        // Grant capability
        assert!(manager.grant_capability(
            "test_plugin",
            PluginCapability::LogInfo
        ).is_ok());
        
        // Check capability
        assert!(manager.has_capability(
            "test_plugin",
            &PluginCapability::LogInfo
        ).unwrap());
        
        // Revoke capability
        assert!(manager.revoke_capability(
            "test_plugin",
            &PluginCapability::LogInfo
        ).is_ok());
        
        // Check capability is gone
        assert!(!manager.has_capability(
            "test_plugin",
            &PluginCapability::LogInfo
        ).unwrap());
    }

    #[test]
    fn test_trust_level_restrictions() {
        let mut manager = PluginSecurityManager::new();
        manager.register_plugin(
            "untrusted_plugin".to_string(),
            Some(PluginTrustLevel::Untrusted)
        ).unwrap();
        
        // Should be able to grant basic capabilities
        assert!(manager.grant_capability(
            "untrusted_plugin",
            PluginCapability::LogInfo
        ).is_ok());
        
        // Should not be able to grant advanced capabilities
        assert!(manager.grant_capability(
            "untrusted_plugin",
            PluginCapability::ScheduleTasks
        ).is_err());
    }

    #[test]
    fn test_security_violations() {
        let mut manager = PluginSecurityManager::new();
        manager.register_plugin(
            "bad_plugin".to_string(),
            Some(PluginTrustLevel::Untrusted)
        ).unwrap();
        
        // Record multiple violations
        for _ in 0..5 {
            manager.record_violation(
                "bad_plugin",
                SecurityViolation::UnauthorizedHostFunction {
                    function_name: "dangerous_function".to_string(),
                    required_capability: PluginCapability::ScheduleTasks,
                }
            ).unwrap();
        }
        
        // Plugin should be suspended
        let context = manager.get_context("bad_plugin").unwrap();
        assert!(context.suspended);
    }
}