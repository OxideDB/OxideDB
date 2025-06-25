//! Error handling for OxideDB
//!
//! This module defines the master `AppError` enum that standardizes error
//! handling across all components of the OxideDB system.

use thiserror::Error;

/// The master error type for OxideDB applications.
///
/// This enum provides standardized error handling across all crates and
/// components. All fallible operations should return `Result<T, AppError>`.
#[derive(Error, Debug, Clone)]
pub enum AppError {
    /// Database-related errors (connection, query execution, etc.)
    #[error("Database error: {message}")]
    Database { message: String },

    /// Validation errors for user input or data constraints
    #[error("Validation error: {field} - {message}")]
    Validation { field: String, message: String },

    /// Authentication and authorization errors
    #[error("Authentication error: {message}")]
    Auth { message: String },

    /// Plugin-related errors (loading, execution, communication)
    #[error("Plugin error: {plugin_name} - {message}")]
    Plugin {
        plugin_name: String,
        message: String,
    },

    /// Network and I/O related errors
    #[error("I/O error: {message}")]
    Io { message: String },

    /// Configuration and setup errors
    #[error("Configuration error: {message}")]
    Config { message: String },

    /// Internal application errors (should not happen in normal operation)
    #[error("Internal error: {message}")]
    Internal { message: String },

    /// Resource not found errors
    #[error("Not found: {resource_type} with identifier '{identifier}'")]
    NotFound {
        resource_type: String,
        identifier: String,
    },

    /// Conflict errors (duplicate resources, concurrent modifications)
    #[error("Conflict: {message}")]
    Conflict { message: String },

    /// Rate limiting and quota errors
    #[error("Rate limit exceeded: {message}")]
    RateLimit { message: String },

    /// Security-related errors (violations, unauthorized access)
    #[error("Security error: {message}")]
    Security { message: String },

    /// Virtual File System errors
    #[error("Virtual File System error: {message}")]
    VirtualFileSystem { message: String },
}

impl AppError {
    /// Create a new database error
    pub fn database<S: Into<String>>(message: S) -> Self {
        Self::Database {
            message: message.into(),
        }
    }

    /// Create a new validation error
    pub fn validation<S: Into<String>>(field: S, message: S) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a new authentication error
    pub fn auth<S: Into<String>>(message: S) -> Self {
        Self::Auth {
            message: message.into(),
        }
    }

    /// Create a new plugin error
    pub fn plugin<S: Into<String>>(plugin_name: S, message: S) -> Self {
        Self::Plugin {
            plugin_name: plugin_name.into(),
            message: message.into(),
        }
    }

    /// Create a new I/O error
    pub fn io<S: Into<String>>(message: S) -> Self {
        Self::Io {
            message: message.into(),
        }
    }

    /// Create a new configuration error
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    /// Create a new internal error
    pub fn internal<S: Into<String>>(message: S) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create a new not found error
    pub fn not_found<S: Into<String>>(resource_type: S, identifier: S) -> Self {
        Self::NotFound {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
        }
    }

    /// Create a new conflict error
    pub fn conflict<S: Into<String>>(message: S) -> Self {
        Self::Conflict {
            message: message.into(),
        }
    }

    /// Create a new rate limit error
    pub fn rate_limit<S: Into<String>>(message: S) -> Self {
        Self::RateLimit {
            message: message.into(),
        }
    }

    /// Create a new security error
    pub fn security<S: Into<String>>(message: S) -> Self {
        Self::Security {
            message: message.into(),
        }
    }

    /// Create a new virtual file system error
    pub fn vfs<S: Into<String>>(message: S) -> Self {
        Self::VirtualFileSystem {
            message: message.into(),
        }
    }
}

/// Convenience type alias for Results using AppError
pub type AppResult<T> = Result<T, AppError>;

// Conversions from common error types
impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::io(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::validation("json", &err.to_string())
    }
}
