//! OxideDB Logging System
//!
//! A high-performance, non-blocking logging and audit system designed for OxideDB.
//! This crate provides:
//!
//! - Non-blocking, async logging with channel-based buffering
//! - SQLite-based storage with automatic table partitioning
//! - Security audit trails with tamper detection
//! - High-performance in-memory caching and batching
//! - Structured logging with correlation IDs
//! - Log retention and archival policies
//! - Real-time log streaming capabilities
//!
//! ## Design Principles
//!
//! 1. **Non-blocking**: All logging operations use async channels and never block the caller
//! 2. **High Performance**: Batched writes, connection pooling, and optimized queries
//! 3. **Security First**: Tamper-evident logs with cryptographic integrity checks
//! 4. **Analytics Ready**: Structured data optimized for future analytics implementation
//!
//! ## Architecture
//!
//! ```text
//! Application -> LogService -> Channel -> Background Worker -> SQLite
//!                          |
//!                          -> Memory Cache (for recent logs)
//!                          -> Metrics Collection
//! ```

pub mod error;
pub mod models;
pub mod service;
pub mod storage;
pub mod audit;
pub mod retention;
pub mod api;
pub mod bridge;

// Re-export main types for public API
pub use error::{LoggingError, LoggingResult};
pub use models::{
    LogEntry, LogLevel, LogContext, SecurityAuditEvent, AuditEventType,
    LogQuery, LogFilter, LogMetrics, CorrelationId
};
pub use service::{LogService, LogServiceConfig, LogServiceBuilder};
pub use audit::{SecurityAuditService, AuditTrail, IntegrityCheck};
pub use bridge::LogServiceBridge;

/// Version of the logging system for compatibility checks
pub const LOGGING_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum batch size for log writes to prevent memory exhaustion
pub const MAX_BATCH_SIZE: usize = 1000;

/// Default channel buffer size for non-blocking operations
pub const DEFAULT_CHANNEL_BUFFER: usize = 10000;

/// Default retention period in days
pub const DEFAULT_RETENTION_DAYS: u32 = 90; 