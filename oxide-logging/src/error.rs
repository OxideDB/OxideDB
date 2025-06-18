//! Error types for the OxideDB logging system

use thiserror::Error;

/// Result type alias for logging operations
pub type LoggingResult<T> = Result<T, LoggingError>;

/// Comprehensive error types for the logging system
#[derive(Error, Debug)]
pub enum LoggingError {
    /// Database-related errors
    #[error("Database error: {message}")]
    Database { message: String },

    /// SQLite-specific errors
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Tokio SQLite errors
    #[error("Async SQLite error: {0}")]
    TokioSqlite(#[from] tokio_rusqlite::Error),

    /// Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Channel communication errors
    #[error("Channel error: {message}")]
    Channel { message: String },

    /// Configuration errors
    #[error("Configuration error: {message}")]
    Configuration { message: String },

    /// Security and integrity errors
    #[error("Security error: {message}")]
    Security { message: String },

    /// Integrity check failures
    #[error("Integrity check failed: {message}")]
    IntegrityFailure { message: String },

    /// Storage capacity errors
    #[error("Storage capacity exceeded: {message}")]
    StorageCapacity { message: String },

    /// Query parsing and execution errors
    #[error("Query error: {message}")]
    Query { message: String },

    /// Invalid input or parameters
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    /// Service not available or shutting down
    #[error("Service unavailable: {message}")]
    ServiceUnavailable { message: String },

    /// Timeout errors for operations
    #[error("Operation timed out: {operation}")]
    Timeout { operation: String },

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// UUID parsing errors
    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    /// Generic internal errors
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl LoggingError {
    /// Create a database error
    pub fn database(message: impl Into<String>) -> Self {
        Self::Database {
            message: message.into(),
        }
    }

    /// Create a channel error
    pub fn channel(message: impl Into<String>) -> Self {
        Self::Channel {
            message: message.into(),
        }
    }

    /// Create a configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::Configuration {
            message: message.into(),
        }
    }

    /// Create a security error
    pub fn security(message: impl Into<String>) -> Self {
        Self::Security {
            message: message.into(),
        }
    }

    /// Create an integrity failure error
    pub fn integrity_failure(message: impl Into<String>) -> Self {
        Self::IntegrityFailure {
            message: message.into(),
        }
    }

    /// Create a storage capacity error
    pub fn storage_capacity(message: impl Into<String>) -> Self {
        Self::StorageCapacity {
            message: message.into(),
        }
    }

    /// Create a query error
    pub fn query(message: impl Into<String>) -> Self {
        Self::Query {
            message: message.into(),
        }
    }

    /// Create an invalid input error
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    /// Create a service unavailable error
    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self::ServiceUnavailable {
            message: message.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(operation: impl Into<String>) -> Self {
        Self::Timeout {
            operation: operation.into(),
        }
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            LoggingError::Channel { .. }
                | LoggingError::Timeout { .. }
                | LoggingError::StorageCapacity { .. }
                | LoggingError::ServiceUnavailable { .. }
        )
    }

    /// Check if this error indicates a critical system failure
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            LoggingError::Security { .. }
                | LoggingError::IntegrityFailure { .. }
                | LoggingError::Database { .. }
        )
    }
} 