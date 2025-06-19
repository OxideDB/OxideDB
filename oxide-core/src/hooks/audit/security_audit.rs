//! Security Audit Hook
//!
//! This hook monitors and logs security-related events including authentication
//! attempts, failed operations, and suspicious activities.

use crate::{BeforeEventContext, AfterEventContext, AppError};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tracing::{warn, error, info, debug};

/// Security event severity levels
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SecurityLevel {
    Info,
    Warning,
    Critical,
}

/// Security event types
#[derive(Debug, Clone)]
pub enum SecurityEvent {
    AuthenticationAttempt { email: String, success: bool },
    PasswordChangeAttempt { user_id: String, success: bool },
    UnauthorizedAccess { collection: String, action: String },
    SuspiciousActivity { description: String, severity: SecurityLevel },
    DataModification { collection: String, record_id: String, sensitive: bool },
    PrivilegeEscalation { user_id: String, attempted_action: String },
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum events per time window
    pub max_events: usize,
    /// Time window duration
    pub window_duration: Duration,
    /// Enable rate limiting
    pub enabled: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_events: 100,
            window_duration: Duration::from_secs(300), // 5 minutes
            enabled: true,
        }
    }
}

/// Configuration for security auditing
#[derive(Debug, Clone)]
pub struct SecurityAuditConfig {
    /// Collections considered sensitive
    pub sensitive_collections: Vec<String>,
    /// Actions that should trigger security alerts
    pub sensitive_actions: Vec<String>,
    /// Whether to enable anomaly detection
    pub enable_anomaly_detection: bool,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
    /// Whether to log IP addresses (requires context)
    pub log_ip_addresses: bool,
}

impl Default for SecurityAuditConfig {
    fn default() -> Self {
        Self {
            sensitive_collections: vec![
                "users".to_string(),
                "superusers".to_string(),
                "admin".to_string(),
                "auth".to_string(),
            ],
            sensitive_actions: vec![
                "delete".to_string(),
                "update".to_string(),
                "auth".to_string(),
            ],
            enable_anomaly_detection: true,
            rate_limit: RateLimitConfig::default(),
            log_ip_addresses: false,
        }
    }
}

/// Event counter for rate limiting
#[derive(Debug)]
struct EventCounter {
    count: usize,
    window_start: SystemTime,
}

/// Security audit hook for monitoring security events
pub struct SecurityAuditHook {
    config: SecurityAuditConfig,
    event_counters: Arc<Mutex<HashMap<String, EventCounter>>>,
}

impl SecurityAuditHook {
    /// Create a new security audit hook with default configuration
    pub fn new() -> Self {
        Self {
            config: SecurityAuditConfig::default(),
            event_counters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a new security audit hook with custom configuration
    pub fn with_config(config: SecurityAuditConfig) -> Self {
        Self {
            config,
            event_counters: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Handle Before events for security monitoring
    pub fn handle_before_event(&self, event_type: &str, context: &BeforeEventContext) -> Result<(), AppError> {
        // Check for sensitive collection access
        if self.is_sensitive_collection(&context.collection) {
            self.log_security_event(SecurityEvent::DataModification {
                collection: context.collection.clone(),
                record_id: context.record_id.clone().unwrap_or_else(|| "new".to_string()),
                sensitive: true,
            })?;
        }

        // Check for suspicious patterns in data
        if self.config.enable_anomaly_detection {
            self.detect_anomalies(context)?;
        }

        // Rate limiting check
        if self.config.rate_limit.enabled {
            self.check_rate_limit(&context.collection, event_type)?;
        }

        Ok(())
    }

    /// Handle After events for security monitoring
    pub fn handle_after_event(&self, event_type: &str, context: &AfterEventContext) -> Result<(), AppError> {
        match context {
            AfterEventContext::UserAuthenticated { user_id, email, .. } => {
                self.log_security_event(SecurityEvent::AuthenticationAttempt {
                    email: email.clone(),
                    success: true,
                })?;
                info!("🔐 Successful authentication: {} ({})", email, user_id);
            }
            AfterEventContext::UserRegistered { user_id, email, .. } => {
                info!("👤 New user registered: {} ({})", email, user_id);
            }
            AfterEventContext::ErrorOccurred { error_type, message, .. } => {
                if error_type.contains("auth") || error_type.contains("Auth") {
                    self.log_security_event(SecurityEvent::SuspiciousActivity {
                        description: format!("Authentication error: {}", message),
                        severity: SecurityLevel::Warning,
                    })?;
                }
            }
            _ => {
                // Handle other security-relevant events
                debug!("Security audit: {} event processed", event_type);
            }
        }

        Ok(())
    }

    /// Log a security event with appropriate severity
    fn log_security_event(&self, event: SecurityEvent) -> Result<(), AppError> {
        match event {
            SecurityEvent::AuthenticationAttempt { email, success } => {
                if success {
                    info!("🔐 Authentication successful: {}", email);
                } else {
                    warn!("🚨 Authentication failed: {}", email);
                }
            }
            SecurityEvent::PasswordChangeAttempt { user_id, success } => {
                if success {
                    info!("🔑 Password changed: {}", user_id);
                } else {
                    warn!("🚨 Password change failed: {}", user_id);
                }
            }
            SecurityEvent::UnauthorizedAccess { collection, action } => {
                error!("🚨 Unauthorized access attempt: {} on {}", action, collection);
            }
            SecurityEvent::SuspiciousActivity { description, severity } => {
                match severity {
                    SecurityLevel::Info => info!("ℹ️ Security info: {}", description),
                    SecurityLevel::Warning => warn!("⚠️ Security warning: {}", description),
                    SecurityLevel::Critical => error!("🚨 Security alert: {}", description),
                }
            }
            SecurityEvent::DataModification { collection, record_id, sensitive } => {
                if sensitive {
                    warn!("🔒 Sensitive data modified: {} record {}", collection, record_id);
                } else {
                    debug!("📝 Data modified: {} record {}", collection, record_id);
                }
            }
            SecurityEvent::PrivilegeEscalation { user_id, attempted_action } => {
                error!("🚨 Privilege escalation attempt: {} tried {}", user_id, attempted_action);
            }
        }

        Ok(())
    }

    /// Check if a collection is considered sensitive
    fn is_sensitive_collection(&self, collection: &str) -> bool {
        self.config.sensitive_collections.contains(&collection.to_string())
    }

    /// Detect anomalies in data or access patterns
    fn detect_anomalies(&self, context: &BeforeEventContext) -> Result<(), AppError> {
        // Check for unusually large data payloads
        let data_size = serde_json::to_string(&context.data)
            .map(|s| s.len())
            .unwrap_or(0);

        if data_size > 10_000 { // 10KB threshold
            self.log_security_event(SecurityEvent::SuspiciousActivity {
                description: format!("Large data payload: {} bytes in collection {}", data_size, context.collection),
                severity: SecurityLevel::Warning,
            })?;
        }

        // Check for suspicious field patterns
        if let Some(obj) = context.data.as_object() {
            // Check for potential injection attempts
            for (key, value) in obj {
                if let Some(str_value) = value.as_str() {
                    if self.contains_suspicious_patterns(str_value) {
                        self.log_security_event(SecurityEvent::SuspiciousActivity {
                            description: format!("Suspicious content in field '{}' of collection {}", key, context.collection),
                            severity: SecurityLevel::Warning,
                        })?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Check for suspicious patterns in string data
    fn contains_suspicious_patterns(&self, data: &str) -> bool {
        let suspicious_patterns = [
            "DROP TABLE",
            "DELETE FROM",
            "<script",
            "javascript:",
            "SELECT * FROM",
            "UNION SELECT",
            "../../../",
            "passwd",
            "/etc/",
        ];

        let data_lower = data.to_lowercase();
        suspicious_patterns.iter().any(|pattern| data_lower.contains(&pattern.to_lowercase()))
    }

    /// Check rate limiting for events
    fn check_rate_limit(&self, collection: &str, event_type: &str) -> Result<(), AppError> {
        let key = format!("{}:{}", collection, event_type);
        let now = SystemTime::now();

        let mut counters = self.event_counters.lock().map_err(|_| {
            AppError::internal("Failed to acquire lock for rate limiting")
        })?;

        let counter = counters.entry(key.clone()).or_insert(EventCounter {
            count: 0,
            window_start: now,
        });

        // Check if we're in a new time window
        if now.duration_since(counter.window_start).unwrap_or(Duration::ZERO) > self.config.rate_limit.window_duration {
            counter.count = 0;
            counter.window_start = now;
        }

        counter.count += 1;

        if counter.count > self.config.rate_limit.max_events {
            self.log_security_event(SecurityEvent::SuspiciousActivity {
                description: format!("Rate limit exceeded for {}: {} events", key, counter.count),
                severity: SecurityLevel::Critical,
            })?;
            
            return Err(AppError::validation("rate_limit", "Rate limit exceeded"));
        }

        Ok(())
    }

    /// Get current security statistics
    pub fn get_security_stats(&self) -> Result<HashMap<String, usize>, AppError> {
        let counters = self.event_counters.lock().map_err(|_| {
            AppError::internal("Failed to acquire lock for security stats")
        })?;

        Ok(counters.iter().map(|(k, v)| (k.clone(), v.count)).collect())
    }

    /// Clear security statistics
    pub fn clear_stats(&self) -> Result<(), AppError> {
        let mut counters = self.event_counters.lock().map_err(|_| {
            AppError::internal("Failed to acquire lock for clearing stats")
        })?;

        counters.clear();
        Ok(())
    }

    /// Get the current configuration
    pub fn config(&self) -> &SecurityAuditConfig {
        &self.config
    }
}

impl Default for SecurityAuditHook {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_security_audit_config() {
        let config = SecurityAuditConfig::default();
        let hook = SecurityAuditHook::with_config(config);

        assert!(hook.config.sensitive_collections.contains(&"users".to_string()));
        assert!(hook.config.enable_anomaly_detection);
    }

    #[test]
    fn test_sensitive_collection_detection() {
        let hook = SecurityAuditHook::new();

        assert!(hook.is_sensitive_collection("users"));
        assert!(hook.is_sensitive_collection("superusers"));
        assert!(!hook.is_sensitive_collection("posts"));
    }

    #[test]
    fn test_suspicious_pattern_detection() {
        let hook = SecurityAuditHook::new();

        assert!(hook.contains_suspicious_patterns("DROP TABLE users"));
        assert!(hook.contains_suspicious_patterns("<script>alert('xss')</script>"));
        assert!(!hook.contains_suspicious_patterns("normal user input"));
    }

    #[test]
    fn test_anomaly_detection() {
        let hook = SecurityAuditHook::new();

        let context = BeforeEventContext::new_create(
            "users".to_string(),
            json!({
                "email": "test@example.com",
                "malicious": "DROP TABLE users"
            }),
        );

        // This should detect the suspicious SQL pattern
        assert!(hook.detect_anomalies(&context).is_ok());
    }

    #[test]
    fn test_rate_limiting() {
        let mut config = SecurityAuditConfig::default();
        config.rate_limit.max_events = 2;
        config.rate_limit.window_duration = Duration::from_secs(60);
        
        let hook = SecurityAuditHook::with_config(config);

        // First two requests should pass
        assert!(hook.check_rate_limit("users", "create").is_ok());
        assert!(hook.check_rate_limit("users", "create").is_ok());
        
        // Third request should be rate limited
        assert!(hook.check_rate_limit("users", "create").is_err());
    }
} 