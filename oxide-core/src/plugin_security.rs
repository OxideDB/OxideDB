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

use crate::{auth::CrudOperation, AppError};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, warn};
use ts_rs::TS;

/// Capability types that can be granted to plugins
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
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
    /// Allow plugin to register HTTP routes
    RegisterHttpRoutes {
        /// Allowed path patterns (regex)
        path_patterns: Vec<String>,
        /// Allowed HTTP methods
        methods: Vec<String>,
    },
    /// Allow plugin to create records in collections
    CreateRecords {
        /// Collections the plugin can create records in
        collections: Vec<String>,
    },
    /// Allow plugin to read records from collections
    ReadRecords {
        /// Collections the plugin can read from
        collections: Vec<String>,
    },
    /// Allow plugin to update records in collections
    UpdateRecords {
        /// Collections the plugin can update
        collections: Vec<String>,
    },
    /// Allow plugin to delete records from collections
    DeleteRecords {
        /// Collections the plugin can delete from
        collections: Vec<String>,
    },
    /// Allow plugin to handle HTTP requests
    HandleHttpRequests,
}

impl PluginCapability {
    /// Return true when this granted capability covers the requested capability.
    ///
    /// Structured capabilities are intentionally treated as scoped grants: a
    /// grant for `["*"]` covers every requested collection, while a grant for
    /// `["posts"]` only covers requests scoped to `posts`.
    pub fn grants(&self, requested: &PluginCapability) -> bool {
        use PluginCapability::*;

        match (self, requested) {
            (LogInfo, LogInfo)
            | (LogError, LogError)
            | (ReadEventData, ReadEventData)
            | (ModifyEventData, ModifyEventData)
            | (BlockOperations, BlockOperations)
            | (ScheduleTasks, ScheduleTasks)
            | (HandleHttpRequests, HandleHttpRequests) => true,
            (ReadConfig { keys: granted }, ReadConfig { keys: requested }) => {
                string_patterns_cover(granted, requested, false)
            }
            (
                AccessCollection {
                    collection: granted_collection,
                    operations: granted_operations,
                },
                AccessCollection {
                    collection: requested_collection,
                    operations: requested_operations,
                },
            ) => {
                string_pattern_matches(granted_collection, requested_collection, false)
                    && requested_operations
                        .iter()
                        .all(|operation| granted_operations.contains(operation))
            }
            (
                HttpRequest {
                    allowed_urls: granted_urls,
                    rate_limit: granted_rate_limit,
                },
                HttpRequest {
                    allowed_urls: requested_urls,
                    rate_limit: requested_rate_limit,
                },
            ) => {
                granted_rate_limit >= requested_rate_limit
                    && string_patterns_cover(granted_urls, requested_urls, false)
            }
            (
                PersistentStorage {
                    max_size: granted_max_size,
                    key_prefixes: granted_prefixes,
                },
                PersistentStorage {
                    max_size: requested_max_size,
                    key_prefixes: requested_prefixes,
                },
            ) => {
                granted_max_size >= requested_max_size
                    && string_patterns_cover(granted_prefixes, requested_prefixes, false)
            }
            (
                EmitEvents {
                    event_types: granted_event_types,
                },
                EmitEvents {
                    event_types: requested_event_types,
                },
            ) => string_patterns_cover(granted_event_types, requested_event_types, false),
            (
                RegisterHttpRoutes {
                    path_patterns: granted_paths,
                    methods: granted_methods,
                },
                RegisterHttpRoutes {
                    path_patterns: requested_paths,
                    methods: requested_methods,
                },
            ) => {
                string_patterns_cover(granted_paths, requested_paths, false)
                    && string_patterns_cover(granted_methods, requested_methods, true)
            }
            (
                CreateRecords {
                    collections: granted,
                },
                CreateRecords {
                    collections: requested,
                },
            )
            | (
                ReadRecords {
                    collections: granted,
                },
                ReadRecords {
                    collections: requested,
                },
            )
            | (
                UpdateRecords {
                    collections: granted,
                },
                UpdateRecords {
                    collections: requested,
                },
            )
            | (
                DeleteRecords {
                    collections: granted,
                },
                DeleteRecords {
                    collections: requested,
                },
            ) => string_patterns_cover(granted, requested, false),
            (_, CreateRecords { collections }) => collections
                .iter()
                .all(|collection| self.allows_record_operation(&CrudOperation::Create, collection)),
            (_, ReadRecords { collections }) => collections.iter().all(|collection| {
                self.allows_record_operation(&CrudOperation::Read, collection)
                    || self.allows_record_operation(&CrudOperation::List, collection)
            }),
            (_, UpdateRecords { collections }) => collections
                .iter()
                .all(|collection| self.allows_record_operation(&CrudOperation::Update, collection)),
            (_, DeleteRecords { collections }) => collections
                .iter()
                .all(|collection| self.allows_record_operation(&CrudOperation::Delete, collection)),
            _ => false,
        }
    }

    /// Return true when this capability allows a CRUD operation on a collection.
    pub fn allows_record_operation(&self, operation: &CrudOperation, collection: &str) -> bool {
        match self {
            PluginCapability::AccessCollection {
                collection: granted_collection,
                operations,
            } => {
                string_pattern_matches(granted_collection, collection, false)
                    && operations.contains(operation)
            }
            PluginCapability::CreateRecords { collections } => {
                *operation == CrudOperation::Create
                    && string_patterns_cover(collections, &[collection.to_string()], false)
            }
            PluginCapability::ReadRecords { collections } => {
                matches!(operation, CrudOperation::Read | CrudOperation::List)
                    && string_patterns_cover(collections, &[collection.to_string()], false)
            }
            PluginCapability::UpdateRecords { collections } => {
                *operation == CrudOperation::Update
                    && string_patterns_cover(collections, &[collection.to_string()], false)
            }
            PluginCapability::DeleteRecords { collections } => {
                *operation == CrudOperation::Delete
                    && string_patterns_cover(collections, &[collection.to_string()], false)
            }
            _ => false,
        }
    }

    /// Return true when this capability allows registering the HTTP route.
    pub fn allows_http_route(&self, method: &str, path: &str) -> bool {
        match self {
            PluginCapability::RegisterHttpRoutes {
                path_patterns,
                methods,
            } => {
                string_patterns_cover(path_patterns, &[path.to_string()], false)
                    && string_patterns_cover(methods, &[method.to_string()], true)
            }
            _ => false,
        }
    }
}

fn string_patterns_cover(
    granted_patterns: &[String],
    requested_values: &[String],
    case_insensitive: bool,
) -> bool {
    granted_patterns.iter().any(|pattern| pattern == "*")
        || requested_values.iter().all(|requested| {
            granted_patterns
                .iter()
                .any(|pattern| string_pattern_matches(pattern, requested, case_insensitive))
        })
}

fn string_pattern_matches(pattern: &str, value: &str, case_insensitive: bool) -> bool {
    let (pattern, value) = if case_insensitive {
        (pattern.to_uppercase(), value.to_uppercase())
    } else {
        (pattern.to_string(), value.to_string())
    };

    if pattern == "*" || pattern == value {
        return true;
    }

    let mut remaining = value.as_str();
    let mut first = true;

    for part in pattern.split('*') {
        if part.is_empty() {
            first = false;
            continue;
        }

        if first && !pattern.starts_with('*') {
            if !remaining.starts_with(part) {
                return false;
            }
            remaining = &remaining[part.len()..];
        } else if let Some(index) = remaining.find(part) {
            remaining = &remaining[index + part.len()..];
        } else {
            return false;
        }

        first = false;
    }

    pattern.ends_with('*') || remaining.is_empty()
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
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
    MaliciousBehavior { description: String },
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
            max_execution_time: 5000,     // 5 seconds
            max_host_calls: 1000,         // 1000 calls per execution
            rate_limit: 60,               // 60 executions per minute
        }
    }
}

impl ResourceLimits {
    /// Create conservative resource limits for strict security
    pub fn conservative() -> Self {
        Self {
            max_memory: 4 * 1024 * 1024, // 4MB
            max_execution_time: 1000,    // 1 second
            max_host_calls: 50,          // 50 calls per execution
            rate_limit: 10,              // 10 executions per minute
        }
    }

    /// Create relaxed resource limits for development
    pub fn relaxed() -> Self {
        Self {
            max_memory: 64 * 1024 * 1024, // 64MB
            max_execution_time: 30000,    // 30 seconds
            max_host_calls: 5000,         // 5000 calls per execution
            rate_limit: 300,              // 300 executions per minute
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

impl Default for PluginSecurityManager {
    fn default() -> Self {
        Self::new()
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

    /// Get the active security policies.
    pub fn policies(&self) -> &SecurityPolicies {
        &self.policies
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
            let context = self
                .contexts
                .get(plugin_name)
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
        let context = self
            .contexts
            .get_mut(plugin_name)
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

        debug!(
            "Granted capability {:?} to plugin {}",
            capability, plugin_name
        );
        Ok(())
    }

    /// Revoke a capability from a plugin
    pub fn revoke_capability(
        &mut self,
        plugin_name: &str,
        capability: &PluginCapability,
    ) -> Result<(), AppError> {
        let context = self
            .contexts
            .get_mut(plugin_name)
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

        debug!(
            "Revoked capability {:?} from plugin {}",
            capability, plugin_name
        );
        Ok(())
    }

    /// Check if a plugin has a specific capability
    pub fn has_capability(
        &self,
        plugin_name: &str,
        capability: &PluginCapability,
    ) -> Result<bool, AppError> {
        let context = self
            .contexts
            .get(plugin_name)
            .ok_or_else(|| AppError::Plugin {
                plugin_name: plugin_name.to_string(),
                message: "Plugin not registered".to_string(),
            })?;

        if context.suspended {
            return Ok(false);
        }

        Ok(context
            .capabilities
            .iter()
            .any(|granted| granted.grants(capability)))
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

    /// Record the result of a plugin execution.
    pub fn record_execution(
        &mut self,
        plugin_name: &str,
        execution_time_ms: u64,
        failed: bool,
        host_function_calls: u64,
        peak_memory_usage: u64,
    ) -> Result<(), AppError> {
        let context = self
            .contexts
            .get_mut(plugin_name)
            .ok_or_else(|| AppError::Plugin {
                plugin_name: plugin_name.to_string(),
                message: "Plugin not registered".to_string(),
            })?;

        context.stats.total_executions = context.stats.total_executions.saturating_add(1);
        context.stats.total_execution_time = context
            .stats
            .total_execution_time
            .saturating_add(execution_time_ms);
        context.stats.host_function_calls = context
            .stats
            .host_function_calls
            .saturating_add(host_function_calls);
        context.stats.peak_memory_usage = context.stats.peak_memory_usage.max(peak_memory_usage);
        context.stats.last_execution = Some(Self::current_timestamp());

        if failed {
            context.stats.failed_executions = context.stats.failed_executions.saturating_add(1);
        }

        Ok(())
    }

    /// Get execution statistics for a plugin.
    pub fn get_execution_stats(&self, plugin_name: &str) -> Option<ExecutionStats> {
        self.contexts
            .get(plugin_name)
            .map(|context| context.stats.clone())
    }

    /// Record a security violation
    pub fn record_violation(
        &mut self,
        plugin_name: &str,
        violation: SecurityViolation,
    ) -> Result<(), AppError> {
        warn!(
            "Security violation by plugin {}: {:?}",
            plugin_name, violation
        );

        let should_suspend = {
            let context = self
                .contexts
                .get_mut(plugin_name)
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
            let context = self
                .contexts
                .get_mut(plugin_name)
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

        error!(
            "Plugin {} suspended due to {} security violations",
            plugin_name, violations_count
        );
        Ok(())
    }

    /// Resume a suspended plugin
    pub fn resume_plugin(&mut self, plugin_name: &str) -> Result<(), AppError> {
        let context = self
            .contexts
            .get_mut(plugin_name)
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
    fn get_resource_limits_for_trust_level(
        &self,
        trust_level: &PluginTrustLevel,
    ) -> ResourceLimits {
        match trust_level {
            PluginTrustLevel::Untrusted => ResourceLimits {
                max_memory: 8 * 1024 * 1024, // 8MB
                max_execution_time: 1000,    // 1 second
                max_host_calls: 100,         // 100 calls
                rate_limit: 10,              // 10 executions per minute
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
                max_memory: u64::MAX,         // Unlimited
                max_execution_time: u64::MAX, // Unlimited
                max_host_calls: u32::MAX,     // Unlimited
                rate_limit: u32::MAX,         // Unlimited
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
            PluginTrustLevel::PartiallyTrusted => !matches!(
                capability,
                PluginCapability::HttpRequest { .. }
                    | PluginCapability::ScheduleTasks
                    | PluginCapability::RegisterHttpRoutes { .. }
                    | PluginCapability::DeleteRecords { .. }
            ),
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
            "register_http_route" => Ok(PluginCapability::RegisterHttpRoutes {
                path_patterns: vec!["*".to_string()],
                methods: vec!["*".to_string()],
            }),
            "create_record" => Ok(PluginCapability::CreateRecords {
                collections: vec!["*".to_string()],
            }),
            "read_records" => Ok(PluginCapability::ReadRecords {
                collections: vec!["*".to_string()],
            }),
            "update_records" => Ok(PluginCapability::UpdateRecords {
                collections: vec!["*".to_string()],
            }),
            "delete_records" => Ok(PluginCapability::DeleteRecords {
                collections: vec!["*".to_string()],
            }),
            "get_http_request" => Ok(PluginCapability::HandleHttpRequests),
            "set_http_response" => Ok(PluginCapability::HandleHttpRequests),
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
        let entry = SecurityAuditEntry {
            timestamp: Self::current_timestamp(),
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

    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_registration() {
        let mut manager = PluginSecurityManager::new();

        // Test registering a plugin
        assert!(manager
            .register_plugin(
                "test_plugin".to_string(),
                Some(PluginTrustLevel::PartiallyTrusted)
            )
            .is_ok());

        // Test plugin context exists
        assert!(manager.get_context("test_plugin").is_some());
    }

    #[test]
    fn test_capability_management() {
        let mut manager = PluginSecurityManager::new();
        manager
            .register_plugin(
                "test_plugin".to_string(),
                Some(PluginTrustLevel::PartiallyTrusted),
            )
            .unwrap();

        // Grant capability
        assert!(manager
            .grant_capability("test_plugin", PluginCapability::LogInfo)
            .is_ok());

        // Check capability
        assert!(manager
            .has_capability("test_plugin", &PluginCapability::LogInfo)
            .unwrap());

        // Revoke capability
        assert!(manager
            .revoke_capability("test_plugin", &PluginCapability::LogInfo)
            .is_ok());

        // Check capability is gone
        assert!(!manager
            .has_capability("test_plugin", &PluginCapability::LogInfo)
            .unwrap());
    }

    #[test]
    fn test_scoped_capability_grants() {
        let granted = PluginCapability::RegisterHttpRoutes {
            path_patterns: vec!["/api/hello/*".to_string()],
            methods: vec!["GET".to_string()],
        };
        let requested = PluginCapability::RegisterHttpRoutes {
            path_patterns: vec!["/api/hello/items".to_string()],
            methods: vec!["get".to_string()],
        };
        let denied = PluginCapability::RegisterHttpRoutes {
            path_patterns: vec!["/api/admin/items".to_string()],
            methods: vec!["GET".to_string()],
        };

        assert!(granted.grants(&requested));
        assert!(!granted.grants(&denied));
    }

    #[test]
    fn test_trust_level_restrictions() {
        let policies = SecurityPolicies {
            allow_untrusted_plugins: true, // Allow untrusted plugins for this test
            ..Default::default()
        };
        let mut manager = PluginSecurityManager::with_policies(policies);
        manager
            .register_plugin(
                "untrusted_plugin".to_string(),
                Some(PluginTrustLevel::Untrusted),
            )
            .unwrap();

        // Should be able to grant basic capabilities
        assert!(manager
            .grant_capability("untrusted_plugin", PluginCapability::LogInfo)
            .is_ok());

        // Should not be able to grant advanced capabilities
        assert!(manager
            .grant_capability("untrusted_plugin", PluginCapability::ScheduleTasks)
            .is_err());
    }

    #[test]
    fn test_security_violations() {
        let policies = SecurityPolicies {
            allow_untrusted_plugins: true, // Allow untrusted plugins for this test
            ..Default::default()
        };
        let mut manager = PluginSecurityManager::with_policies(policies);
        manager
            .register_plugin("bad_plugin".to_string(), Some(PluginTrustLevel::Untrusted))
            .unwrap();

        // Record multiple violations
        for _ in 0..5 {
            manager
                .record_violation(
                    "bad_plugin",
                    SecurityViolation::UnauthorizedHostFunction {
                        function_name: "dangerous_function".to_string(),
                        required_capability: PluginCapability::ScheduleTasks,
                    },
                )
                .unwrap();
        }

        // Plugin should be suspended
        let context = manager.get_context("bad_plugin").unwrap();
        assert!(context.suspended);
    }
}
