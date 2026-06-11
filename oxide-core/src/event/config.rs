//! Event System Configuration
//!
//! This module provides configuration structures and utilities for
//! setting up the event system in different environments and use cases.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Main configuration structure for the event system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventSystemConfig {
    /// Core event bus configuration
    pub bus: EventBusConfig,
    /// Metrics collection configuration
    pub metrics: MetricsConfig,
    /// Handler execution configuration
    pub handlers: HandlerConfig,
    /// Middleware configuration
    pub middleware: MiddlewareConfig,
    /// Performance tuning options
    pub performance: PerformanceConfig,
    /// Logging and debugging options
    pub logging: LoggingConfig,
}

impl EventSystemConfig {
    /// Create configuration optimized for development
    pub fn development() -> Self {
        Self {
            bus: EventBusConfig::development(),
            metrics: MetricsConfig::development(),
            handlers: HandlerConfig::development(),
            middleware: MiddlewareConfig::development(),
            performance: PerformanceConfig::development(),
            logging: LoggingConfig::development(),
        }
    }

    /// Create configuration optimized for production
    pub fn production() -> Self {
        Self {
            bus: EventBusConfig::production(),
            metrics: MetricsConfig::production(),
            handlers: HandlerConfig::production(),
            middleware: MiddlewareConfig::production(),
            performance: PerformanceConfig::production(),
            logging: LoggingConfig::production(),
        }
    }

    /// Create configuration optimized for testing
    pub fn testing() -> Self {
        Self {
            bus: EventBusConfig::testing(),
            metrics: MetricsConfig::testing(),
            handlers: HandlerConfig::testing(),
            middleware: MiddlewareConfig::testing(),
            performance: PerformanceConfig::testing(),
            logging: LoggingConfig::testing(),
        }
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self, ConfigError> {
        let profile =
            std::env::var("OXIDE_EVENT_PROFILE").unwrap_or_else(|_| "development".to_string());

        let base_config = match profile.as_str() {
            "production" => Self::production(),
            "testing" => Self::testing(),
            _ => Self::development(),
        };

        base_config.apply_env_overrides()
    }

    /// Apply environment variable overrides to configuration
    fn apply_env_overrides(mut self) -> Result<Self, ConfigError> {
        // Bus configuration overrides
        if let Ok(max_concurrent) = std::env::var("OXIDE_EVENT_MAX_CONCURRENT_HANDLERS") {
            self.bus.max_concurrent_handlers = max_concurrent.parse().map_err(|_| {
                ConfigError::InvalidValue("OXIDE_EVENT_MAX_CONCURRENT_HANDLERS".to_string())
            })?;
        }

        if let Ok(timeout_ms) = std::env::var("OXIDE_EVENT_DEFAULT_HANDLER_TIMEOUT_MS") {
            self.bus.default_handler_timeout_ms = timeout_ms.parse().map_err(|_| {
                ConfigError::InvalidValue("OXIDE_EVENT_DEFAULT_HANDLER_TIMEOUT_MS".to_string())
            })?;
        }

        if let Ok(max_retries) = std::env::var("OXIDE_EVENT_MAX_HANDLER_RETRIES") {
            self.bus.max_handler_retries = max_retries.parse().map_err(|_| {
                ConfigError::InvalidValue("OXIDE_EVENT_MAX_HANDLER_RETRIES".to_string())
            })?;
        }

        // Metrics configuration overrides
        if let Ok(enabled) = std::env::var("OXIDE_EVENT_METRICS_ENABLED") {
            self.metrics.enabled = enabled.parse().map_err(|_| {
                ConfigError::InvalidValue("OXIDE_EVENT_METRICS_ENABLED".to_string())
            })?;
        }

        // Performance configuration overrides
        if let Ok(buffer_size) = std::env::var("OXIDE_EVENT_BUFFER_SIZE") {
            self.performance.event_buffer_size = buffer_size
                .parse()
                .map_err(|_| ConfigError::InvalidValue("OXIDE_EVENT_BUFFER_SIZE".to_string()))?;
        }

        Ok(self)
    }

    /// Validate the configuration for consistency
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate bus configuration
        if self.bus.max_concurrent_handlers == 0 {
            return Err(ConfigError::ValidationError(
                "max_concurrent_handlers must be greater than 0".to_string(),
            ));
        }

        if self.bus.default_handler_timeout_ms == 0 {
            return Err(ConfigError::ValidationError(
                "default_handler_timeout_ms must be greater than 0".to_string(),
            ));
        }

        // Validate performance configuration
        if self.performance.event_buffer_size == 0 {
            return Err(ConfigError::ValidationError(
                "event_buffer_size must be greater than 0".to_string(),
            ));
        }

        if self.performance.max_event_batch_size > self.performance.event_buffer_size {
            return Err(ConfigError::ValidationError(
                "max_event_batch_size cannot be larger than event_buffer_size".to_string(),
            ));
        }

        // Validate middleware configuration
        if self.middleware.timeout.enable_timeout && self.middleware.timeout.default_timeout_ms == 0
        {
            return Err(ConfigError::ValidationError(
                "timeout middleware default_timeout_ms must be greater than 0 when enabled"
                    .to_string(),
            ));
        }

        if self.middleware.retry.enable_retry && self.middleware.retry.max_retries == 0 {
            return Err(ConfigError::ValidationError(
                "retry middleware max_retries must be greater than 0 when enabled".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for EventSystemConfig {
    fn default() -> Self {
        Self::development()
    }
}

/// Event bus specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBusConfig {
    /// Maximum number of concurrent event handlers
    pub max_concurrent_handlers: usize,
    /// Default timeout for event handlers (milliseconds)
    pub default_handler_timeout_ms: u64,
    /// Maximum number of retries for failed handlers
    pub max_handler_retries: u32,
    /// Whether to continue processing if a handler fails
    pub continue_on_handler_failure: bool,
    /// Whether to enable handler filtering
    pub enable_filtering: bool,
    /// Graceful shutdown timeout (milliseconds)
    pub shutdown_timeout_ms: u64,
}

impl EventBusConfig {
    pub fn development() -> Self {
        Self {
            max_concurrent_handlers: 50,
            default_handler_timeout_ms: 30000, // 30 seconds for debugging
            max_handler_retries: 1,
            continue_on_handler_failure: false, // Fail fast in development
            enable_filtering: false,
            shutdown_timeout_ms: 5000,
        }
    }

    pub fn production() -> Self {
        Self {
            max_concurrent_handlers: 200,
            default_handler_timeout_ms: 5000, // 5 seconds
            max_handler_retries: 3,
            continue_on_handler_failure: true,
            enable_filtering: true,
            shutdown_timeout_ms: 30000,
        }
    }

    pub fn testing() -> Self {
        Self {
            max_concurrent_handlers: 10,
            default_handler_timeout_ms: 1000,
            max_handler_retries: 0,
            continue_on_handler_failure: false,
            enable_filtering: false,
            shutdown_timeout_ms: 1000,
        }
    }
}

/// Metrics collection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Whether to collect metrics
    pub enabled: bool,
    /// How often to update aggregated metrics (milliseconds)
    pub collection_interval_ms: u64,
    /// How long to retain detailed metrics (milliseconds)
    pub retention_duration_ms: u64,
    /// Maximum number of recent events to keep for analysis
    pub max_recent_events: usize,
    /// Whether to collect per-handler metrics
    pub per_handler_metrics: bool,
    /// Whether to collect execution time histograms
    pub execution_time_histograms: bool,
}

impl MetricsConfig {
    pub fn development() -> Self {
        Self {
            enabled: true,
            collection_interval_ms: 5000,
            retention_duration_ms: 3600000, // 1 hour
            max_recent_events: 100,
            per_handler_metrics: true,
            execution_time_histograms: false,
        }
    }

    pub fn production() -> Self {
        Self {
            enabled: true,
            collection_interval_ms: 1000,
            retention_duration_ms: 86400000, // 24 hours
            max_recent_events: 1000,
            per_handler_metrics: true,
            execution_time_histograms: true,
        }
    }

    pub fn testing() -> Self {
        Self {
            enabled: false,
            collection_interval_ms: 1000,
            retention_duration_ms: 60000, // 1 minute
            max_recent_events: 10,
            per_handler_metrics: false,
            execution_time_histograms: false,
        }
    }
}

/// Handler execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandlerConfig {
    /// Default priority for handlers
    pub default_priority: i32,
    /// Whether to enable handler metadata validation
    pub validate_metadata: bool,
    /// Whether to allow duplicate handler IDs
    pub allow_duplicate_ids: bool,
    /// Maximum number of handlers per event type
    pub max_handlers_per_event: Option<usize>,
    /// Custom timeout overrides per event type
    pub event_timeouts: HashMap<String, u64>,
}

impl HandlerConfig {
    pub fn development() -> Self {
        Self {
            default_priority: 0,
            validate_metadata: true,
            allow_duplicate_ids: false,
            max_handlers_per_event: None,
            event_timeouts: HashMap::new(),
        }
    }

    pub fn production() -> Self {
        Self {
            default_priority: 0,
            validate_metadata: true,
            allow_duplicate_ids: false,
            max_handlers_per_event: Some(50),
            event_timeouts: [
                ("BeforeRecordDelete".to_string(), 10000),
                ("BeforeCollectionDelete".to_string(), 30000),
            ]
            .iter()
            .cloned()
            .collect(),
        }
    }

    pub fn testing() -> Self {
        Self {
            default_priority: 0,
            validate_metadata: false,
            allow_duplicate_ids: true,
            max_handlers_per_event: Some(5),
            event_timeouts: HashMap::new(),
        }
    }
}

/// Middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MiddlewareConfig {
    /// Whether to enable middleware
    pub enabled: bool,
    /// Timeout middleware configuration
    pub timeout: TimeoutMiddlewareConfig,
    /// Retry middleware configuration
    pub retry: RetryMiddlewareConfig,
    /// Circuit breaker middleware configuration
    pub circuit_breaker: CircuitBreakerMiddlewareConfig,
}

impl MiddlewareConfig {
    pub fn development() -> Self {
        Self {
            enabled: false,
            timeout: TimeoutMiddlewareConfig::development(),
            retry: RetryMiddlewareConfig::development(),
            circuit_breaker: CircuitBreakerMiddlewareConfig::development(),
        }
    }

    pub fn production() -> Self {
        Self {
            enabled: true,
            timeout: TimeoutMiddlewareConfig::production(),
            retry: RetryMiddlewareConfig::production(),
            circuit_breaker: CircuitBreakerMiddlewareConfig::production(),
        }
    }

    pub fn testing() -> Self {
        Self {
            enabled: false,
            timeout: TimeoutMiddlewareConfig::testing(),
            retry: RetryMiddlewareConfig::testing(),
            circuit_breaker: CircuitBreakerMiddlewareConfig::testing(),
        }
    }
}

/// Timeout middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeoutMiddlewareConfig {
    /// Whether to enable timeout middleware
    pub enable_timeout: bool,
    /// Default timeout for handlers (milliseconds)
    pub default_timeout_ms: u64,
    /// Per-event-type timeout overrides
    pub event_timeouts: HashMap<String, u64>,
}

impl TimeoutMiddlewareConfig {
    pub fn development() -> Self {
        Self {
            enable_timeout: false,
            default_timeout_ms: 30000,
            event_timeouts: HashMap::new(),
        }
    }

    pub fn production() -> Self {
        Self {
            enable_timeout: true,
            default_timeout_ms: 5000,
            event_timeouts: [
                ("BeforeRecordDelete".to_string(), 10000),
                ("BeforeCollectionDelete".to_string(), 30000),
            ]
            .iter()
            .cloned()
            .collect(),
        }
    }

    pub fn testing() -> Self {
        Self {
            enable_timeout: false,
            default_timeout_ms: 1000,
            event_timeouts: HashMap::new(),
        }
    }
}

/// Retry middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryMiddlewareConfig {
    /// Whether to enable retry middleware
    pub enable_retry: bool,
    /// Maximum number of retries
    pub max_retries: u32,
    /// Initial delay between retries (milliseconds)
    pub initial_delay_ms: u64,
    /// Maximum delay between retries (milliseconds)
    pub max_delay_ms: u64,
    /// Backoff multiplier for exponential backoff
    pub backoff_multiplier: f64,
}

impl RetryMiddlewareConfig {
    pub fn development() -> Self {
        Self {
            enable_retry: false,
            max_retries: 1,
            initial_delay_ms: 100,
            max_delay_ms: 1000,
            backoff_multiplier: 2.0,
        }
    }

    pub fn production() -> Self {
        Self {
            enable_retry: true,
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
        }
    }

    pub fn testing() -> Self {
        Self {
            enable_retry: false,
            max_retries: 0,
            initial_delay_ms: 10,
            max_delay_ms: 100,
            backoff_multiplier: 1.0,
        }
    }
}

/// Circuit breaker middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerMiddlewareConfig {
    /// Whether to enable circuit breaker middleware
    pub enable_circuit_breaker: bool,
    /// Number of failures before opening the circuit
    pub failure_threshold: u32,
    /// Time to wait before attempting recovery (milliseconds)
    pub recovery_timeout_ms: u64,
    /// Maximum number of calls to allow in half-open state
    pub half_open_max_calls: u32,
}

impl CircuitBreakerMiddlewareConfig {
    pub fn development() -> Self {
        Self {
            enable_circuit_breaker: false,
            failure_threshold: 10,
            recovery_timeout_ms: 30000,
            half_open_max_calls: 3,
        }
    }

    pub fn production() -> Self {
        Self {
            enable_circuit_breaker: true,
            failure_threshold: 5,
            recovery_timeout_ms: 60000,
            half_open_max_calls: 3,
        }
    }

    pub fn testing() -> Self {
        Self {
            enable_circuit_breaker: false,
            failure_threshold: 2,
            recovery_timeout_ms: 1000,
            half_open_max_calls: 1,
        }
    }
}

/// Performance tuning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Size of the event buffer for async processing
    pub event_buffer_size: usize,
    /// Maximum number of events to process in a batch
    pub max_event_batch_size: usize,
    /// Time to wait for a batch to fill before processing (milliseconds)
    pub batch_timeout_ms: u64,
    /// Number of worker threads for event processing
    pub worker_thread_count: Option<usize>,
    /// Whether to enable event prefetching
    pub enable_prefetching: bool,
    /// Size of the prefetch buffer
    pub prefetch_buffer_size: usize,
}

impl PerformanceConfig {
    pub fn development() -> Self {
        Self {
            event_buffer_size: 100,
            max_event_batch_size: 10,
            batch_timeout_ms: 100,
            worker_thread_count: None,
            enable_prefetching: false,
            prefetch_buffer_size: 0,
        }
    }

    pub fn production() -> Self {
        Self {
            event_buffer_size: 10000,
            max_event_batch_size: 100,
            batch_timeout_ms: 10,
            worker_thread_count: Some(num_cpus::get()),
            enable_prefetching: true,
            prefetch_buffer_size: 1000,
        }
    }

    pub fn testing() -> Self {
        Self {
            event_buffer_size: 10,
            max_event_batch_size: 1,
            batch_timeout_ms: 1,
            worker_thread_count: Some(1),
            enable_prefetching: false,
            prefetch_buffer_size: 0,
        }
    }
}

/// Logging and debugging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Whether to enable debug logging for events
    pub debug_events: bool,
    /// Whether to log handler execution details
    pub log_handler_execution: bool,
    /// Whether to log event timing information
    pub log_timing: bool,
    /// Whether to log event context details
    pub log_context: bool,
    /// Maximum size of context data to log (bytes)
    pub max_context_log_size: usize,
    /// Whether to log to structured format (JSON)
    pub structured_logging: bool,
}

impl LoggingConfig {
    pub fn development() -> Self {
        Self {
            debug_events: true,
            log_handler_execution: true,
            log_timing: true,
            log_context: true,
            max_context_log_size: 10000,
            structured_logging: false,
        }
    }

    pub fn production() -> Self {
        Self {
            debug_events: false,
            log_handler_execution: false,
            log_timing: false,
            log_context: false,
            max_context_log_size: 1000,
            structured_logging: true,
        }
    }

    pub fn testing() -> Self {
        Self {
            debug_events: false,
            log_handler_execution: false,
            log_timing: false,
            log_context: false,
            max_context_log_size: 0,
            structured_logging: false,
        }
    }
}

/// Configuration error types
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration value for {0}")]
    InvalidValue(String),

    #[error("Configuration validation failed: {0}")]
    ValidationError(String),

    #[error("Missing required configuration: {0}")]
    MissingRequired(String),

    #[error("IO error reading configuration: {0}")]
    IoError(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

/// Utility functions for configuration management
impl EventSystemConfig {
    /// Save configuration to a TOML file
    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), ConfigError> {
        let toml_content = toml::to_string_pretty(self)
            .map_err(|e| ConfigError::SerializationError(e.to_string()))?;

        std::fs::write(path, toml_content).map_err(|e| ConfigError::IoError(e.to_string()))?;

        Ok(())
    }

    /// Load configuration from a TOML file
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        let content =
            std::fs::read_to_string(path).map_err(|e| ConfigError::IoError(e.to_string()))?;

        let config: Self =
            toml::from_str(&content).map_err(|e| ConfigError::SerializationError(e.to_string()))?;

        config.validate()?;
        Ok(config)
    }

    /// Convert durations from config values to Duration objects
    pub fn get_handler_timeout(&self, event_name: &str) -> Duration {
        let timeout_ms = self
            .handlers
            .event_timeouts
            .get(event_name)
            .copied()
            .unwrap_or(self.bus.default_handler_timeout_ms);

        Duration::from_millis(timeout_ms)
    }

    /// Get retry configuration as Duration objects
    pub fn get_retry_config(&self) -> (Duration, Duration) {
        (
            Duration::from_millis(self.middleware.retry.initial_delay_ms),
            Duration::from_millis(self.middleware.retry.max_delay_ms),
        )
    }

    /// Get circuit breaker recovery timeout as Duration
    pub fn get_circuit_breaker_recovery_timeout(&self) -> Duration {
        Duration::from_millis(self.middleware.circuit_breaker.recovery_timeout_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_presets() {
        let dev_config = EventSystemConfig::development();
        assert!(!dev_config.bus.continue_on_handler_failure);
        assert_eq!(dev_config.bus.max_handler_retries, 1);

        let prod_config = EventSystemConfig::production();
        assert!(prod_config.bus.continue_on_handler_failure);
        assert_eq!(prod_config.bus.max_handler_retries, 3);
        assert!(prod_config.metrics.enabled);

        let test_config = EventSystemConfig::testing();
        assert!(!test_config.metrics.enabled);
        assert_eq!(test_config.bus.max_handler_retries, 0);
    }

    #[test]
    fn test_config_validation() {
        let mut config = EventSystemConfig::development();
        config.bus.max_concurrent_handlers = 0;

        assert!(config.validate().is_err());

        config.bus.max_concurrent_handlers = 10;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_serialization() {
        let config = EventSystemConfig::production();
        let toml_content = toml::to_string_pretty(&config).unwrap();

        let deserialized: EventSystemConfig = toml::from_str(&toml_content).unwrap();
        assert_eq!(
            config.bus.max_concurrent_handlers,
            deserialized.bus.max_concurrent_handlers
        );
    }

    #[test]
    fn test_config_file_operations() {
        let config = EventSystemConfig::testing();
        let temp_file = NamedTempFile::new().unwrap();

        // Save to file
        config.save_to_file(temp_file.path()).unwrap();

        // Load from file
        let loaded_config = EventSystemConfig::load_from_file(temp_file.path()).unwrap();
        assert_eq!(
            config.bus.max_concurrent_handlers,
            loaded_config.bus.max_concurrent_handlers
        );
    }

    #[test]
    fn test_duration_helpers() {
        let config = EventSystemConfig::production();

        let timeout = config.get_handler_timeout("BeforeRecordDelete");
        assert_eq!(timeout, Duration::from_millis(10000));

        let default_timeout = config.get_handler_timeout("UnknownEvent");
        assert_eq!(
            default_timeout,
            Duration::from_millis(config.bus.default_handler_timeout_ms)
        );

        let (initial, max) = config.get_retry_config();
        assert_eq!(initial, Duration::from_millis(100));
        assert_eq!(max, Duration::from_millis(30000));
    }

    #[test]
    fn test_env_override() {
        std::env::set_var("OXIDE_EVENT_MAX_CONCURRENT_HANDLERS", "500");
        std::env::set_var("OXIDE_EVENT_METRICS_ENABLED", "false");

        let mut config = EventSystemConfig::development();
        config = config.apply_env_overrides().unwrap();

        assert_eq!(config.bus.max_concurrent_handlers, 500);
        assert!(!config.metrics.enabled);

        // Clean up
        std::env::remove_var("OXIDE_EVENT_MAX_CONCURRENT_HANDLERS");
        std::env::remove_var("OXIDE_EVENT_METRICS_ENABLED");
    }
}
