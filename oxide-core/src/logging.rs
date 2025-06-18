//! Logging abstractions for OxideDB
//!
//! This module defines the logging traits and interfaces that can be implemented
//! by concrete logging services. It provides a clean abstraction layer without
//! directly depending on any specific logging implementation.

use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Result type for logging operations
pub type LoggingResult<T> = Result<T, AppError>;

/// Trait for correlation IDs that can be passed between components
pub trait CorrelationIdTrait: Send + Sync + Clone + std::fmt::Display + std::fmt::Debug {
    /// Create a new correlation ID
    fn new() -> Self;
    /// Parse from string
    fn from_str(s: &str) -> Result<Self, AppError>;
    /// Convert to string
    fn to_string(&self) -> String;
}

/// Log severity levels (matches the concrete implementation)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    /// Critical system errors
    Error = 0,
    /// Warning conditions  
    Warn = 1,
    /// Informational messages
    Info = 2,
    /// Debug information
    Debug = 3,
    /// Detailed trace information
    Trace = 4,
}

impl LogLevel {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN", 
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "ERROR" => Some(LogLevel::Error),
            "WARN" | "WARNING" => Some(LogLevel::Warn),
            "INFO" => Some(LogLevel::Info),
            "DEBUG" => Some(LogLevel::Debug),
            "TRACE" => Some(LogLevel::Trace),
            _ => None,
        }
    }
}

/// Types of security audit events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Authentication events
    Authentication,
    /// Authorization events
    Authorization,
    /// Data access events
    DataAccess,
    /// Data modification events
    DataModification,
    /// Configuration changes
    ConfigurationChange,
    /// Security policy violations
    SecurityViolation,
    /// Plugin events
    PluginEvent,
    /// System events
    SystemEvent,
}

impl AuditEventType {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditEventType::Authentication => "authentication",
            AuditEventType::Authorization => "authorization",
            AuditEventType::DataAccess => "data_access",
            AuditEventType::DataModification => "data_modification",
            AuditEventType::ConfigurationChange => "configuration_change",
            AuditEventType::SecurityViolation => "security_violation",
            AuditEventType::PluginEvent => "plugin_event",
            AuditEventType::SystemEvent => "system_event",
        }
    }
}

/// Context information for log entries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogContext {
    /// User ID if available
    pub user_id: Option<String>,
    /// Session ID if available
    pub session_id: Option<String>,
    /// Collection being operated on
    pub collection: Option<String>,
    /// Record ID being operated on
    pub record_id: Option<String>,
    /// Operation being performed
    pub operation: Option<String>,
    /// Client IP address
    pub client_ip: Option<String>,
    /// User agent string
    pub user_agent: Option<String>,
    /// Additional custom metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for LogContext {
    fn default() -> Self {
        Self {
            user_id: None,
            session_id: None,
            collection: None,
            record_id: None,
            operation: None,
            client_ip: None,
            user_agent: None,
            metadata: HashMap::new(),
        }
    }
}

impl LogContext {
    /// Create a new empty log context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set collection
    pub fn with_collection(mut self, collection: impl Into<String>) -> Self {
        self.collection = Some(collection.into());
        self
    }

    /// Set record ID
    pub fn with_record_id(mut self, record_id: impl Into<String>) -> Self {
        self.record_id = Some(record_id.into());
        self
    }

    /// Set operation
    pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// Set client IP
    pub fn with_client_ip(mut self, client_ip: impl Into<String>) -> Self {
        self.client_ip = Some(client_ip.into());
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Add custom metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Trait for application logging service
pub trait ApplicationLogger: Send + Sync {
    /// The correlation ID type used by this logger
    type CorrelationId: CorrelationIdTrait;

    /// Log an informational message
    fn info(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Log a warning message
    fn warn(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Log an error message
    fn error(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Log a debug message
    fn debug(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Log a trace message
    fn trace(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Log with context
    fn log_with_context(
        &self,
        level: LogLevel,
        message: String,
        module: String,
        context: LogContext,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Log with correlation ID
    fn log_with_correlation(
        &self,
        level: LogLevel,
        message: String,
        module: String,
        correlation_id: Self::CorrelationId,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Log with both context and correlation ID
    fn log_with_context_and_correlation(
        &self,
        level: LogLevel,
        message: String,
        module: String,
        context: LogContext,
        correlation_id: Self::CorrelationId,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;

    /// Force flush all pending logs
    fn flush(&self) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>>;
}

/// Trait for security audit logging
pub trait SecurityAuditor: Send + Sync {
    /// The correlation ID type used by this auditor
    type CorrelationId: CorrelationIdTrait;

    /// Log an authentication event
    fn log_authentication(
        &self,
        actor: String,
        action: String,
        result: String,
        context: LogContext,
        risk_score: Option<u8>,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;

    /// Log an authorization event
    fn log_authorization(
        &self,
        actor: String,
        target: String,
        action: String,
        result: String,
        context: LogContext,
        risk_score: Option<u8>,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;

    /// Log a data access event
    fn log_data_access(
        &self,
        actor: String,
        target: String,
        action: String,
        context: LogContext,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;

    /// Log a data modification event
    fn log_data_modification(
        &self,
        actor: String,
        target: String,
        action: String,
        context: LogContext,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;

    /// Log a configuration change event
    fn log_configuration_change(
        &self,
        actor: String,
        target: String,
        action: String,
        context: LogContext,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;

    /// Log a security violation event
    fn log_security_violation(
        &self,
        actor: String,
        violation_type: String,
        description: String,
        context: LogContext,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;

    /// Log a plugin event
    fn log_plugin_event(
        &self,
        plugin_name: String,
        action: String,
        result: String,
        context: LogContext,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;

    /// Log a system event
    fn log_system_event(
        &self,
        event_type: String,
        description: String,
        context: LogContext,
    ) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>>;
}

/// Combined logging service that provides both application logging and security auditing
pub trait LoggingService: ApplicationLogger + SecurityAuditor + Send + Sync {
    /// Get logging metrics
    fn get_metrics(&self) -> Pin<Box<dyn Future<Output = LoggingResult<LoggingMetrics>> + Send + '_>>;

    /// Shutdown the logging service gracefully
    fn shutdown(self: Arc<Self>) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send>>;
}

/// Basic logging metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingMetrics {
    /// Total number of log entries
    pub total_entries: u64,
    /// Number of entries by level
    pub entries_by_level: HashMap<LogLevel, u64>,
    /// Storage size in bytes
    pub storage_size_bytes: u64,
    /// Recent error rate
    pub error_rate_24h: f64,
}

/// No-op logger implementation for testing or when logging is disabled
pub struct NoOpLogger;

impl ApplicationLogger for NoOpLogger {
    type CorrelationId = NoOpCorrelationId;

    fn info(&self, _message: String, _module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn warn(&self, _message: String, _module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn error(&self, _message: String, _module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn debug(&self, _message: String, _module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn trace(&self, _message: String, _module: String) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn log_with_context(&self, _level: LogLevel, _message: String, _module: String, _context: LogContext) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn log_with_correlation(&self, _level: LogLevel, _message: String, _module: String, _correlation_id: Self::CorrelationId) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn log_with_context_and_correlation(&self, _level: LogLevel, _message: String, _module: String, _context: LogContext, _correlation_id: Self::CorrelationId) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn flush(&self) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

impl SecurityAuditor for NoOpLogger {
    type CorrelationId = NoOpCorrelationId;

    fn log_authentication(&self, _actor: String, _action: String, _result: String, _context: LogContext, _risk_score: Option<u8>) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }

    fn log_authorization(&self, _actor: String, _target: String, _action: String, _result: String, _context: LogContext, _risk_score: Option<u8>) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }

    fn log_data_access(&self, _actor: String, _target: String, _action: String, _context: LogContext) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }

    fn log_data_modification(&self, _actor: String, _target: String, _action: String, _context: LogContext) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }

    fn log_configuration_change(&self, _actor: String, _target: String, _action: String, _context: LogContext) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }

    fn log_security_violation(&self, _actor: String, _violation_type: String, _description: String, _context: LogContext) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }

    fn log_plugin_event(&self, _plugin_name: String, _action: String, _result: String, _context: LogContext) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }

    fn log_system_event(&self, _event_type: String, _description: String, _context: LogContext) -> Pin<Box<dyn Future<Output = LoggingResult<Uuid>> + Send + '_>> {
        Box::pin(async { Ok(Uuid::new_v4()) })
    }
}

impl LoggingService for NoOpLogger {
    fn get_metrics(&self) -> Pin<Box<dyn Future<Output = LoggingResult<LoggingMetrics>> + Send + '_>> {
        Box::pin(async {
            Ok(LoggingMetrics {
                total_entries: 0,
                entries_by_level: HashMap::new(),
                storage_size_bytes: 0,
                error_rate_24h: 0.0,
            })
        })
    }

    fn shutdown(self: Arc<Self>) -> Pin<Box<dyn Future<Output = LoggingResult<()>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

/// No-op correlation ID for testing
#[derive(Debug, Clone)]
pub struct NoOpCorrelationId;

impl CorrelationIdTrait for NoOpCorrelationId {
    fn new() -> Self {
        Self
    }

    fn from_str(_s: &str) -> Result<Self, AppError> {
        Ok(Self)
    }

    fn to_string(&self) -> String {
        "noop".to_string()
    }
}

impl std::fmt::Display for NoOpCorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "noop")
    }
} 