//! Data models for the OxideDB logging system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Unique identifier for correlating related log entries across operations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub Uuid);

impl CorrelationId {
    /// Generate a new correlation ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    /// Get the inner UUID
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for CorrelationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Log severity levels
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

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Additional context information for log entries
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

/// Main log entry structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique identifier for this log entry
    pub id: Uuid,
    /// Correlation ID for grouping related entries
    pub correlation_id: CorrelationId,
    /// Timestamp when the log was created
    pub timestamp: DateTime<Utc>,
    /// Log level
    pub level: LogLevel,
    /// Log message
    pub message: String,
    /// Module or component that generated the log
    pub module: String,
    /// File and line where log was generated (optional)
    pub location: Option<String>,
    /// Additional context information
    pub context: LogContext,
    /// Error information if this is an error log
    pub error: Option<String>,
    /// Stack trace if available
    pub stack_trace: Option<String>,
    /// Performance metrics (duration, memory usage, etc.)
    pub metrics: Option<HashMap<String, f64>>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(
        level: LogLevel,
        message: impl Into<String>,
        module: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            correlation_id: CorrelationId::new(),
            timestamp: Utc::now(),
            level,
            message: message.into(),
            module: module.into(),
            location: None,
            context: LogContext::new(),
            error: None,
            stack_trace: None,
            metrics: None,
        }
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: CorrelationId) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    /// Set location information
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    /// Set context
    pub fn with_context(mut self, context: LogContext) -> Self {
        self.context = context;
        self
    }

    /// Set error information
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self
    }

    /// Set stack trace
    pub fn with_stack_trace(mut self, stack_trace: impl Into<String>) -> Self {
        self.stack_trace = Some(stack_trace.into());
        self
    }

    /// Add performance metric
    pub fn with_metric(mut self, key: impl Into<String>, value: f64) -> Self {
        if self.metrics.is_none() {
            self.metrics = Some(HashMap::new());
        }
        self.metrics.as_mut().unwrap().insert(key.into(), value);
        self
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

/// Security audit event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditEvent {
    /// Unique identifier for this audit event
    pub id: Uuid,
    /// Correlation ID for grouping related events
    pub correlation_id: CorrelationId,
    /// Timestamp when the event occurred
    pub timestamp: DateTime<Utc>,
    /// Type of audit event
    pub event_type: AuditEventType,
    /// Severity level
    pub severity: LogLevel,
    /// Event description
    pub description: String,
    /// Actor who performed the action (user, system, plugin)
    pub actor: String,
    /// Target of the action (resource, data, etc.)
    pub target: Option<String>,
    /// Action performed
    pub action: String,
    /// Result of the action (success, failure, etc.)
    pub result: String,
    /// Additional context and metadata
    pub context: LogContext,
    /// Risk score (0-100)
    pub risk_score: Option<u8>,
    /// Cryptographic hash for integrity verification
    pub integrity_hash: Option<String>,
}

impl SecurityAuditEvent {
    /// Create a new security audit event
    pub fn new(
        event_type: AuditEventType,
        severity: LogLevel,
        description: impl Into<String>,
        actor: impl Into<String>,
        action: impl Into<String>,
        result: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            correlation_id: CorrelationId::new(),
            timestamp: Utc::now(),
            event_type,
            severity,
            description: description.into(),
            actor: actor.into(),
            target: None,
            action: action.into(),
            result: result.into(),
            context: LogContext::new(),
            risk_score: None,
            integrity_hash: None,
        }
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: CorrelationId) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    /// Set target
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }

    /// Set context
    pub fn with_context(mut self, context: LogContext) -> Self {
        self.context = context;
        self
    }

    /// Set risk score
    pub fn with_risk_score(mut self, risk_score: u8) -> Self {
        self.risk_score = Some(risk_score.min(100));
        self
    }
}

/// Query structure for retrieving logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogQuery {
    /// Filter criteria
    pub filter: LogFilter,
    /// Number of entries to return
    pub limit: Option<usize>,
    /// Number of entries to skip
    pub offset: Option<usize>,
    /// Sort order (newest first by default)
    pub sort_desc: bool,
}

impl Default for LogQuery {
    fn default() -> Self {
        Self {
            filter: LogFilter::default(),
            limit: Some(100),
            offset: None,
            sort_desc: true,
        }
    }
}

/// Filter criteria for log queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LogFilter {
    /// Filter by log level (minimum level)
    pub min_level: Option<LogLevel>,
    /// Filter by maximum log level
    pub max_level: Option<LogLevel>,
    /// Filter by time range
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    /// Filter by correlation ID
    pub correlation_id: Option<CorrelationId>,
    /// Filter by module
    pub module: Option<String>,
    /// Filter by user ID
    pub user_id: Option<String>,
    /// Filter by collection
    pub collection: Option<String>,
    /// Filter by operation
    pub operation: Option<String>,
    /// Text search in message
    pub message_contains: Option<String>,
    /// Filter by audit event type
    pub audit_event_type: Option<AuditEventType>,
}

/// Metrics and statistics about logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMetrics {
    /// Total number of log entries
    pub total_entries: u64,
    /// Number of entries by level
    pub entries_by_level: HashMap<LogLevel, u64>,
    /// Number of audit events by type
    pub audit_events_by_type: HashMap<AuditEventType, u64>,
    /// Storage size in bytes
    pub storage_size_bytes: u64,
    /// Average entries per day
    pub avg_entries_per_day: f64,
    /// Most active users
    pub top_users: Vec<(String, u64)>,
    /// Most accessed collections
    pub top_collections: Vec<(String, u64)>,
    /// Recent error rate
    pub error_rate_24h: f64,
}