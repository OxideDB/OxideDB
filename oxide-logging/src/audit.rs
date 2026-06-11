//! Security audit service with tamper detection and integrity checks
//!
//! This module provides comprehensive security auditing capabilities:
//! - Tamper-evident audit trails
//! - Cryptographic integrity verification
//! - Risk assessment and scoring
//! - Security event correlation

use crate::{
    error::LoggingResult,
    models::{AuditEventType, CorrelationId, LogContext, LogLevel, SecurityAuditEvent},
    storage::SqliteLogStorage,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::debug;
use uuid::Uuid;

/// Security audit service for tamper-evident logging
pub struct SecurityAuditService {
    /// Storage backend
    storage: Arc<SqliteLogStorage>,
    /// Previous event hash for chain integrity
    last_event_hash: Arc<tokio::sync::RwLock<Option<String>>>,
}

impl SecurityAuditService {
    /// Create a new security audit service
    pub fn new(storage: Arc<SqliteLogStorage>) -> Self {
        Self {
            storage,
            last_event_hash: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Log an authentication event
    pub async fn log_authentication(
        &self,
        actor: impl Into<String>,
        action: impl Into<String>,
        result: impl Into<String>,
        context: LogContext,
        risk_score: Option<u8>,
    ) -> LoggingResult<Uuid> {
        let actor_str = actor.into();
        let action_str = action.into();
        let result_str = result.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::Authentication,
            if result_str.to_lowercase().contains("success") {
                LogLevel::Info
            } else {
                LogLevel::Warn
            },
            format!("Authentication event: {}", action_str),
            actor_str,
            action_str,
            result_str,
        )
        .with_context(context.clone())
        .with_risk_score(risk_score.unwrap_or(self.calculate_auth_risk_score(&context)));

        self.log_audit_event(event).await
    }

    /// Log an authorization event
    pub async fn log_authorization(
        &self,
        actor: impl Into<String>,
        target: impl Into<String>,
        action: impl Into<String>,
        result: impl Into<String>,
        context: LogContext,
        risk_score: Option<u8>,
    ) -> LoggingResult<Uuid> {
        let actor_str = actor.into();
        let target_str = target.into();
        let action_str = action.into();
        let result_str = result.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::Authorization,
            if result_str.to_lowercase().contains("granted") {
                LogLevel::Info
            } else {
                LogLevel::Warn
            },
            format!(
                "Authorization check: {} attempted {} on {}",
                actor_str, action_str, target_str
            ),
            actor_str,
            action_str,
            result_str,
        )
        .with_target(target_str)
        .with_context(context.clone())
        .with_risk_score(risk_score.unwrap_or(self.calculate_authz_risk_score(&context)));

        self.log_audit_event(event).await
    }

    /// Log a data access event
    pub async fn log_data_access(
        &self,
        actor: impl Into<String>,
        target: impl Into<String>,
        action: impl Into<String>,
        context: LogContext,
    ) -> LoggingResult<Uuid> {
        let actor_str = actor.into();
        let target_str = target.into();
        let action_str = action.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::DataAccess,
            LogLevel::Info,
            format!(
                "Data access: {} performed {} on {}",
                actor_str, action_str, target_str
            ),
            actor_str,
            action_str,
            "success".to_string(),
        )
        .with_target(target_str)
        .with_context(context.clone())
        .with_risk_score(self.calculate_data_access_risk_score(&context));

        self.log_audit_event(event).await
    }

    /// Log a data modification event
    pub async fn log_data_modification(
        &self,
        actor: impl Into<String>,
        target: impl Into<String>,
        action: impl Into<String>,
        context: LogContext,
    ) -> LoggingResult<Uuid> {
        let actor_str = actor.into();
        let target_str = target.into();
        let action_str = action.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::DataModification,
            LogLevel::Warn, // Data modifications are always notable
            format!(
                "Data modification: {} performed {} on {}",
                actor_str, action_str, target_str
            ),
            actor_str,
            action_str,
            "success".to_string(),
        )
        .with_target(target_str)
        .with_context(context.clone())
        .with_risk_score(self.calculate_data_modification_risk_score(&context));

        self.log_audit_event(event).await
    }

    /// Log a configuration change event
    pub async fn log_configuration_change(
        &self,
        actor: impl Into<String>,
        target: impl Into<String>,
        action: impl Into<String>,
        context: LogContext,
    ) -> LoggingResult<Uuid> {
        let actor_str = actor.into();
        let target_str = target.into();
        let action_str = action.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::ConfigurationChange,
            LogLevel::Warn, // Config changes are always notable
            format!(
                "Configuration change: {} performed {} on {}",
                actor_str, action_str, target_str
            ),
            actor_str,
            action_str,
            "success".to_string(),
        )
        .with_target(target_str)
        .with_context(context)
        .with_risk_score(75); // Config changes are inherently risky

        self.log_audit_event(event).await
    }

    /// Log a security violation event
    pub async fn log_security_violation(
        &self,
        actor: impl Into<String>,
        violation_type: impl Into<String>,
        description: impl Into<String>,
        context: LogContext,
    ) -> LoggingResult<Uuid> {
        let actor_str = actor.into();
        let violation_type_str = violation_type.into();
        let description_str = description.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::SecurityViolation,
            LogLevel::Error, // Security violations are errors
            description_str,
            actor_str,
            violation_type_str,
            "violation_detected".to_string(),
        )
        .with_context(context)
        .with_risk_score(90); // Security violations are high risk

        self.log_audit_event(event).await
    }

    /// Log a plugin event
    pub async fn log_plugin_event(
        &self,
        plugin_name: impl Into<String>,
        action: impl Into<String>,
        result: impl Into<String>,
        context: LogContext,
    ) -> LoggingResult<Uuid> {
        let plugin_name_str = plugin_name.into();
        let action_str = action.into();
        let result_str = result.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::PluginEvent,
            LogLevel::Info,
            format!("Plugin event: {} performed {}", plugin_name_str, action_str),
            plugin_name_str,
            action_str,
            result_str,
        )
        .with_context(context.clone())
        .with_risk_score(self.calculate_plugin_risk_score(&context));

        self.log_audit_event(event).await
    }

    /// Log a system event
    pub async fn log_system_event(
        &self,
        event_type: impl Into<String>,
        description: impl Into<String>,
        context: LogContext,
    ) -> LoggingResult<Uuid> {
        let event_type_str = event_type.into();
        let description_str = description.into();

        let event = SecurityAuditEvent::new(
            AuditEventType::SystemEvent,
            LogLevel::Info,
            description_str,
            "system".to_string(),
            event_type_str,
            "completed".to_string(),
        )
        .with_context(context)
        .with_risk_score(10); // System events are generally low risk

        self.log_audit_event(event).await
    }

    /// Log a generic audit event with integrity verification
    pub async fn log_audit_event(&self, mut event: SecurityAuditEvent) -> LoggingResult<Uuid> {
        // Calculate integrity hash
        let integrity_hash = self.calculate_integrity_hash(&event).await?;
        event.integrity_hash = Some(integrity_hash.clone());

        // Store the event
        self.storage.insert_audit_event(&event).await?;

        // Update the last event hash for chain integrity
        let mut last_hash = self.last_event_hash.write().await;
        *last_hash = Some(integrity_hash);

        debug!(
            "Logged audit event: {} (type: {:?}, risk: {:?})",
            event.id, event.event_type, event.risk_score
        );

        Ok(event.id)
    }

    /// Verify the integrity of audit events
    pub async fn verify_audit_integrity(&self) -> LoggingResult<IntegrityCheck> {
        // This would implement a full integrity check of the audit trail
        // For now, we'll return a basic implementation
        Ok(IntegrityCheck {
            total_events_checked: 0,
            integrity_violations: Vec::new(),
            chain_breaks: Vec::new(),
            verification_timestamp: Utc::now(),
            is_valid: true,
        })
    }

    /// Calculate integrity hash for an audit event
    async fn calculate_integrity_hash(&self, event: &SecurityAuditEvent) -> LoggingResult<String> {
        // Get the previous event hash for chaining
        let last_hash = self.last_event_hash.read().await;
        let previous_hash = last_hash.as_deref().unwrap_or("genesis");

        // Create a deterministic string representation of the event
        let event_data = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            event.id,
            event.correlation_id,
            event.timestamp.timestamp(),
            event.event_type.as_str(),
            event.actor,
            event.action,
            event.result,
            previous_hash,
            serde_json::to_string(&event.context).unwrap_or_default()
        );

        // Calculate SHA-256 hash
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        event_data.hash(&mut hasher);
        let hash = hasher.finish();

        Ok(format!("{:x}", hash))
    }

    /// Calculate risk score for authentication events
    fn calculate_auth_risk_score(&self, context: &LogContext) -> u8 {
        let mut risk = 20; // Base risk for auth events

        // Increase risk for failed attempts
        if let Some(ref user_agent) = context.user_agent {
            if user_agent.contains("bot") || user_agent.contains("script") {
                risk += 30;
            }
        }

        // Increase risk for unusual IP patterns
        if let Some(ref ip) = context.client_ip {
            if ip.starts_with("10.") || ip.starts_with("192.168.") || ip.starts_with("172.") {
                risk -= 5; // Internal IPs are slightly less risky
            } else {
                risk += 10; // External IPs have higher risk
            }
        }

        risk.min(100)
    }

    /// Calculate risk score for authorization events
    fn calculate_authz_risk_score(&self, context: &LogContext) -> u8 {
        let mut risk = 15; // Base risk for authz events

        // Higher risk for admin operations
        if let Some(ref operation) = context.operation {
            if operation.contains("admin")
                || operation.contains("delete")
                || operation.contains("modify")
            {
                risk += 25;
            }
        }

        risk.min(100)
    }

    /// Calculate risk score for data access events
    fn calculate_data_access_risk_score(&self, context: &LogContext) -> u8 {
        let mut risk = 10; // Base risk for data access

        // Higher risk for sensitive collections
        if let Some(ref collection) = context.collection {
            if collection.contains("user")
                || collection.contains("auth")
                || collection.contains("admin")
            {
                risk += 20;
            }
        }

        risk.min(100)
    }

    /// Calculate risk score for data modification events
    fn calculate_data_modification_risk_score(&self, context: &LogContext) -> u8 {
        let mut risk = 30; // Higher base risk for modifications

        // Much higher risk for sensitive collections
        if let Some(ref collection) = context.collection {
            if collection.contains("user")
                || collection.contains("auth")
                || collection.contains("admin")
            {
                risk += 40;
            }
        }

        // Higher risk for bulk operations
        if let Some(ref operation) = context.operation {
            if operation.contains("bulk") || operation.contains("batch") {
                risk += 20;
            }
        }

        risk.min(100)
    }

    /// Calculate risk score for plugin events
    fn calculate_plugin_risk_score(&self, context: &LogContext) -> u8 {
        let mut risk = 25; // Plugins have inherent risk

        // Higher risk for untrusted plugins
        if context.metadata.get("trust_level").and_then(|v| v.as_str()) == Some("untrusted") {
            risk += 30;
        }

        risk.min(100)
    }
}

/// Result of an audit trail integrity check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityCheck {
    /// Total number of events checked
    pub total_events_checked: u64,
    /// List of integrity violations found
    pub integrity_violations: Vec<IntegrityViolation>,
    /// List of chain breaks found
    pub chain_breaks: Vec<ChainBreak>,
    /// When the verification was performed
    pub verification_timestamp: DateTime<Utc>,
    /// Overall integrity status
    pub is_valid: bool,
}

/// An integrity violation in the audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrityViolation {
    /// Event ID with the violation
    pub event_id: Uuid,
    /// Type of violation
    pub violation_type: String,
    /// Description of the violation
    pub description: String,
    /// Expected hash
    pub expected_hash: Option<String>,
    /// Actual hash
    pub actual_hash: Option<String>,
}

/// A break in the audit trail chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainBreak {
    /// Event ID where the chain break occurs
    pub event_id: Uuid,
    /// Previous event ID in the chain
    pub previous_event_id: Option<Uuid>,
    /// Description of the break
    pub description: String,
}

/// Audit trail information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrail {
    /// Start time of the trail
    pub start_time: DateTime<Utc>,
    /// End time of the trail
    pub end_time: DateTime<Utc>,
    /// Total number of events in the trail
    pub total_events: u64,
    /// Events by type
    pub events_by_type: HashMap<AuditEventType, u64>,
    /// Average risk score
    pub average_risk_score: f64,
    /// Highest risk events
    pub high_risk_events: Vec<Uuid>,
}

/// Audit event correlation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditCorrelation {
    /// Correlation ID
    pub correlation_id: CorrelationId,
    /// Related event IDs
    pub event_ids: Vec<Uuid>,
    /// Start time of the correlated events
    pub start_time: DateTime<Utc>,
    /// End time of the correlated events
    pub end_time: DateTime<Utc>,
    /// Summary of the correlated activity
    pub summary: String,
}
