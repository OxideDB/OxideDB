//! Logging utilities for plugins

use crate::{Host, LogLevel};

/// Logging utilities for plugins
pub struct Logger;

impl Logger {
    /// Log a message at a specific level
    pub fn log(level: LogLevel, message: &str) {
        Host::log(level, message);
    }

    /// Log an info message
    pub fn info(message: &str) {
        Host::log_info(message);
    }

    /// Log an error message
    pub fn error(message: &str) {
        Host::log_error(message);
    }

    /// Log a warning message
    pub fn warn(message: &str) {
        Host::log_warn(message);
    }

    /// Log a debug message
    pub fn debug(message: &str) {
        Host::log_debug(message);
    }

    /// Set an error that will prevent the operation from continuing
    pub fn set_error(message: &str) {
        Host::set_error(message);
    }
}

/// Structured logging builder
pub struct LogBuilder {
    level: LogLevel,
    message: String,
    fields: Vec<(String, String)>,
}

impl LogBuilder {
    /// Create a new log builder with level
    pub fn new(level: LogLevel) -> Self {
        Self {
            level,
            message: String::new(),
            fields: Vec::new(),
        }
    }

    /// Create an info log builder
    pub fn info() -> Self {
        Self::new(LogLevel::Info)
    }

    /// Create an error log builder
    pub fn error() -> Self {
        Self::new(LogLevel::Error)
    }

    /// Create a warning log builder
    pub fn warn() -> Self {
        Self::new(LogLevel::Warn)
    }

    /// Create a debug log builder
    pub fn debug() -> Self {
        Self::new(LogLevel::Debug)
    }

    /// Set the log message
    pub fn message<S: Into<String>>(mut self, message: S) -> Self {
        self.message = message.into();
        self
    }

    /// Add a field to the log
    pub fn field<K: Into<String>, V: std::fmt::Display>(mut self, key: K, value: V) -> Self {
        self.fields.push((key.into(), value.to_string()));
        self
    }

    /// Emit the log message
    pub fn emit(self) {
        let mut log_message = self.message;

        if !self.fields.is_empty() {
            log_message.push_str(" [");
            for (i, (key, value)) in self.fields.iter().enumerate() {
                if i > 0 {
                    log_message.push_str(", ");
                }
                log_message.push_str(&format!("{}={}", key, value));
            }
            log_message.push(']');
        }

        Logger::log(self.level, &log_message);
    }
}
