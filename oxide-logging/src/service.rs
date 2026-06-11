//! High-performance, non-blocking logging service
//!
//! This module provides the main LogService that coordinates all logging operations.
//! Key features:
//! - Non-blocking API using async channels
//! - Background worker for batch processing
//! - In-memory caching for recent logs
//! - Automatic batching and flushing
//! - Graceful shutdown handling

use crate::{
    audit::SecurityAuditService,
    error::{LoggingError, LoggingResult},
    models::{
        CorrelationId, LogContext, LogEntry, LogLevel, LogMetrics, LogQuery, SecurityAuditEvent,
    },
    storage::SqliteLogStorage,
    DEFAULT_CHANNEL_BUFFER, MAX_BATCH_SIZE,
};
use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
    time::{interval, timeout},
};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Configuration for the logging service
#[derive(Debug, Clone)]
pub struct LogServiceConfig {
    /// Database file path
    pub db_path: PathBuf,
    /// Channel buffer size for non-blocking operations
    pub channel_buffer_size: usize,
    /// Maximum batch size for database writes
    pub max_batch_size: usize,
    /// Batch flush interval in milliseconds
    pub batch_flush_interval_ms: u64,
    /// Maximum memory cache size for recent logs
    pub memory_cache_size: usize,
    /// Log retention period in days
    pub retention_days: u32,
    /// Cleanup interval in hours
    pub cleanup_interval_hours: u64,
    /// Enable performance metrics collection
    pub enable_metrics: bool,
    /// Maximum time to wait for graceful shutdown
    pub shutdown_timeout_seconds: u64,
}

impl Default for LogServiceConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("logs/oxidedb.log.sqlite"),
            channel_buffer_size: DEFAULT_CHANNEL_BUFFER,
            max_batch_size: MAX_BATCH_SIZE,
            batch_flush_interval_ms: 1000, // 1 second
            memory_cache_size: 1000,
            retention_days: 90,
            cleanup_interval_hours: 24,
            enable_metrics: true,
            shutdown_timeout_seconds: 30,
        }
    }
}

/// Builder for LogServiceConfig
#[derive(Debug)]
pub struct LogServiceBuilder {
    config: LogServiceConfig,
}

impl LogServiceBuilder {
    /// Create a new builder with default configuration
    pub fn new() -> Self {
        Self {
            config: LogServiceConfig::default(),
        }
    }

    /// Set database path
    pub fn db_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.db_path = path.into();
        self
    }

    /// Set channel buffer size
    pub fn channel_buffer_size(mut self, size: usize) -> Self {
        self.config.channel_buffer_size = size;
        self
    }

    /// Set maximum batch size
    pub fn max_batch_size(mut self, size: usize) -> Self {
        self.config.max_batch_size = size.min(MAX_BATCH_SIZE);
        self
    }

    /// Set batch flush interval
    pub fn batch_flush_interval(mut self, interval: Duration) -> Self {
        self.config.batch_flush_interval_ms = interval.as_millis() as u64;
        self
    }

    /// Set memory cache size
    pub fn memory_cache_size(mut self, size: usize) -> Self {
        self.config.memory_cache_size = size;
        self
    }

    /// Set retention period
    pub fn retention_days(mut self, days: u32) -> Self {
        self.config.retention_days = days;
        self
    }

    /// Set cleanup interval
    pub fn cleanup_interval(mut self, interval: Duration) -> Self {
        self.config.cleanup_interval_hours = interval.as_secs() / 3600;
        self
    }

    /// Enable or disable metrics collection
    pub fn enable_metrics(mut self, enable: bool) -> Self {
        self.config.enable_metrics = enable;
        self
    }

    /// Set shutdown timeout
    pub fn shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.config.shutdown_timeout_seconds = timeout.as_secs();
        self
    }

    /// Build the configuration
    pub fn build(self) -> LogServiceConfig {
        self.config
    }
}

impl Default for LogServiceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Commands sent to the background worker
#[derive(Debug)]
enum LogCommand {
    /// Log a single entry
    LogEntry(LogEntry),
    /// Log multiple entries in batch
    LogBatch(Vec<LogEntry>),
    /// Log an audit event
    AuditEvent(SecurityAuditEvent),
    /// Query logs with response channel
    Query {
        query: LogQuery,
        response: oneshot::Sender<LoggingResult<Vec<LogEntry>>>,
    },
    /// Query audit events with response channel
    QueryAuditEvents {
        query: LogQuery,
        response: oneshot::Sender<LoggingResult<Vec<SecurityAuditEvent>>>,
    },
    /// Get metrics with response channel
    GetMetrics {
        response: oneshot::Sender<LoggingResult<LogMetrics>>,
    },
    /// Force flush pending batches
    Flush,
    /// Shutdown the worker
    Shutdown,
}

/// High-performance, non-blocking logging service
pub struct LogService {
    /// Command sender for non-blocking operations
    command_sender: mpsc::Sender<LogCommand>,
    /// Background worker handle
    worker_handle: JoinHandle<()>,
    /// In-memory cache for recent logs (for quick access)
    memory_cache: Arc<DashMap<Uuid, LogEntry>>,
    /// Security audit service
    audit_service: Arc<SecurityAuditService>,
    /// Service configuration
    config: LogServiceConfig,
}

impl LogService {
    /// Create a new logging service with default configuration
    pub async fn new() -> LoggingResult<Self> {
        Self::with_config(LogServiceConfig::default()).await
    }

    /// Create a new logging service with custom configuration
    pub async fn with_config(config: LogServiceConfig) -> LoggingResult<Self> {
        // Create storage backend
        let storage = Arc::new(SqliteLogStorage::new(&config.db_path).await?);

        // Create audit service
        let audit_service = Arc::new(SecurityAuditService::new(Arc::clone(&storage)));

        // Create memory cache
        let memory_cache = Arc::new(DashMap::new());

        // Create command channel
        let (command_sender, command_receiver) = mpsc::channel(config.channel_buffer_size);

        // Start background worker
        let worker_handle = tokio::spawn(Self::background_worker(
            command_receiver,
            Arc::clone(&storage),
            Arc::clone(&memory_cache),
            config.clone(),
        ));

        info!("LogService initialized with config: {:?}", config);

        Ok(Self {
            command_sender,
            worker_handle,
            memory_cache,
            audit_service,
            config,
        })
    }

    /// Log an entry (non-blocking)
    pub async fn log(&self, entry: LogEntry) -> LoggingResult<()> {
        // Add to memory cache
        if self.memory_cache.len() < self.config.memory_cache_size {
            self.memory_cache.insert(entry.id, entry.clone());
        }

        // Send to background worker
        self.command_sender
            .send(LogCommand::LogEntry(entry))
            .await
            .map_err(|_| LoggingError::channel("Failed to send log entry to worker"))?;

        Ok(())
    }

    /// Log multiple entries in batch (non-blocking)
    pub async fn log_batch(&self, entries: Vec<LogEntry>) -> LoggingResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        // Add to memory cache
        for entry in &entries {
            if self.memory_cache.len() < self.config.memory_cache_size {
                self.memory_cache.insert(entry.id, entry.clone());
            }
        }

        // Send to background worker
        self.command_sender
            .send(LogCommand::LogBatch(entries))
            .await
            .map_err(|_| LoggingError::channel("Failed to send log batch to worker"))?;

        Ok(())
    }

    /// Log a security audit event (non-blocking)
    pub async fn audit(&self, event: SecurityAuditEvent) -> LoggingResult<()> {
        self.command_sender
            .send(LogCommand::AuditEvent(event))
            .await
            .map_err(|_| LoggingError::channel("Failed to send audit event to worker"))?;

        Ok(())
    }

    /// Convenience method to log an info message
    pub async fn info(
        &self,
        message: impl Into<String>,
        module: impl Into<String>,
    ) -> LoggingResult<()> {
        let entry = LogEntry::new(LogLevel::Info, message, module);
        self.log(entry).await
    }

    /// Convenience method to log an error message
    pub async fn error(
        &self,
        message: impl Into<String>,
        module: impl Into<String>,
    ) -> LoggingResult<()> {
        let entry = LogEntry::new(LogLevel::Error, message, module);
        self.log(entry).await
    }

    /// Convenience method to log a warning message
    pub async fn warn(
        &self,
        message: impl Into<String>,
        module: impl Into<String>,
    ) -> LoggingResult<()> {
        let entry = LogEntry::new(LogLevel::Warn, message, module);
        self.log(entry).await
    }

    /// Convenience method to log with context
    pub async fn log_with_context(
        &self,
        level: LogLevel,
        message: impl Into<String>,
        module: impl Into<String>,
        context: LogContext,
    ) -> LoggingResult<()> {
        let entry = LogEntry::new(level, message, module).with_context(context);
        self.log(entry).await
    }

    /// Convenience method to log with correlation ID
    pub async fn log_with_correlation(
        &self,
        level: LogLevel,
        message: impl Into<String>,
        module: impl Into<String>,
        correlation_id: CorrelationId,
    ) -> LoggingResult<()> {
        let entry = LogEntry::new(level, message, module).with_correlation_id(correlation_id);
        self.log(entry).await
    }

    /// Query log entries
    pub async fn query(&self, query: LogQuery) -> LoggingResult<Vec<LogEntry>> {
        let (sender, receiver) = oneshot::channel();

        self.command_sender
            .send(LogCommand::Query {
                query,
                response: sender,
            })
            .await
            .map_err(|_| LoggingError::channel("Failed to send query to worker"))?;

        receiver
            .await
            .map_err(|_| LoggingError::channel("Failed to receive query response"))?
    }

    /// Query audit events
    pub async fn query_audit_events(
        &self,
        query: LogQuery,
    ) -> LoggingResult<Vec<SecurityAuditEvent>> {
        let (sender, receiver) = oneshot::channel();

        self.command_sender
            .send(LogCommand::QueryAuditEvents {
                query,
                response: sender,
            })
            .await
            .map_err(|_| LoggingError::channel("Failed to send audit query to worker"))?;

        receiver
            .await
            .map_err(|_| LoggingError::channel("Failed to receive audit query response"))?
    }

    /// Get logging metrics and statistics
    pub async fn get_metrics(&self) -> LoggingResult<LogMetrics> {
        let (sender, receiver) = oneshot::channel();

        self.command_sender
            .send(LogCommand::GetMetrics { response: sender })
            .await
            .map_err(|_| LoggingError::channel("Failed to send metrics request to worker"))?;

        receiver
            .await
            .map_err(|_| LoggingError::channel("Failed to receive metrics response"))?
    }

    /// Force flush all pending logs to storage
    pub async fn flush(&self) -> LoggingResult<()> {
        self.command_sender
            .send(LogCommand::Flush)
            .await
            .map_err(|_| LoggingError::channel("Failed to send flush command to worker"))?;

        Ok(())
    }

    /// Get recent logs from memory cache
    pub fn get_recent_logs(&self, limit: usize) -> Vec<LogEntry> {
        let mut entries: Vec<_> = self
            .memory_cache
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        entries.sort_by_key(|entry| std::cmp::Reverse(entry.timestamp));
        entries.truncate(limit);
        entries
    }

    /// Get the audit service
    pub fn audit_service(&self) -> &SecurityAuditService {
        &self.audit_service
    }

    /// Gracefully shutdown the logging service
    pub async fn shutdown(self) -> LoggingResult<()> {
        // Send shutdown command
        if self
            .command_sender
            .send(LogCommand::Shutdown)
            .await
            .is_err()
        {
            warn!("Failed to send shutdown command to worker");
        }

        // Wait for worker to finish with timeout
        let shutdown_timeout = Duration::from_secs(self.config.shutdown_timeout_seconds);
        match timeout(shutdown_timeout, self.worker_handle).await {
            Ok(result) => {
                if let Err(e) = result {
                    error!("Background worker panicked during shutdown: {:?}", e);
                    return Err(LoggingError::internal("Worker panicked during shutdown"));
                }
            }
            Err(_) => {
                warn!("Shutdown timed out, background worker may still be running");
                return Err(LoggingError::timeout("shutdown"));
            }
        }

        info!("LogService shutdown completed");
        Ok(())
    }

    /// Background worker that handles all storage operations
    async fn background_worker(
        mut command_receiver: mpsc::Receiver<LogCommand>,
        storage: Arc<SqliteLogStorage>,
        memory_cache: Arc<DashMap<Uuid, LogEntry>>,
        config: LogServiceConfig,
    ) {
        let mut pending_logs = Vec::new();
        let mut last_flush = Instant::now();

        // Create flush timer
        let mut flush_interval = interval(Duration::from_millis(config.batch_flush_interval_ms));
        flush_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Create cleanup timer
        let mut cleanup_interval =
            interval(Duration::from_secs(config.cleanup_interval_hours * 3600));
        cleanup_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        info!("Background logging worker started");

        loop {
            tokio::select! {
                // Handle incoming commands
                command = command_receiver.recv() => {
                    match command {
                        Some(LogCommand::LogEntry(entry)) => {
                            pending_logs.push(entry);

                            // Flush if batch is full
                            if pending_logs.len() >= config.max_batch_size {
                                Self::flush_pending_logs(&storage, &mut pending_logs).await;
                                last_flush = Instant::now();
                            }
                        }
                        Some(LogCommand::LogBatch(mut entries)) => {
                            pending_logs.append(&mut entries);

                            // Flush if batch is full
                            if pending_logs.len() >= config.max_batch_size {
                                Self::flush_pending_logs(&storage, &mut pending_logs).await;
                                last_flush = Instant::now();
                            }
                        }
                        Some(LogCommand::AuditEvent(event)) => {
                            if let Err(e) = storage.insert_audit_event(&event).await {
                                error!("Failed to insert audit event: {}", e);
                            }
                        }
                        Some(LogCommand::Query { query, response }) => {
                            let result = storage.query_log_entries(&query).await;
                            if response.send(result).is_err() {
                                warn!("Failed to send query response");
                            }
                        }
                        Some(LogCommand::QueryAuditEvents { query, response }) => {
                            let result = storage.query_audit_events(&query).await;
                            if response.send(result).is_err() {
                                warn!("Failed to send audit query response");
                            }
                        }
                        Some(LogCommand::GetMetrics { response }) => {
                            let result = storage.get_metrics().await;
                            if response.send(result).is_err() {
                                warn!("Failed to send metrics response");
                            }
                        }
                        Some(LogCommand::Flush) => {
                            Self::flush_pending_logs(&storage, &mut pending_logs).await;
                            last_flush = Instant::now();
                        }
                        Some(LogCommand::Shutdown) => {
                            info!("Received shutdown command");
                            break;
                        }
                        None => {
                            warn!("Command channel closed");
                            break;
                        }
                    }
                }

                // Periodic flush
                _ = flush_interval.tick() => {
                    if !pending_logs.is_empty() && last_flush.elapsed().as_millis() >= config.batch_flush_interval_ms as u128 {
                        Self::flush_pending_logs(&storage, &mut pending_logs).await;
                        last_flush = Instant::now();
                    }
                }

                // Periodic cleanup
                _ = cleanup_interval.tick() => {
                    if let Err(e) = storage.cleanup_old_entries(config.retention_days).await {
                        error!("Failed to cleanup old log entries: {}", e);
                    }

                    // Clean memory cache
                    if memory_cache.len() > config.memory_cache_size {
                        let excess = memory_cache.len() - config.memory_cache_size;
                        let mut removed = 0;
                        memory_cache.retain(|_, _| {
                            if removed < excess {
                                removed += 1;
                                false
                            } else {
                                true
                            }
                        });
                        debug!("Cleaned {} entries from memory cache", removed);
                    }
                }
            }
        }

        // Final flush before shutdown
        if !pending_logs.is_empty() {
            Self::flush_pending_logs(&storage, &mut pending_logs).await;
        }

        info!("Background logging worker stopped");
    }

    /// Flush pending logs to storage
    async fn flush_pending_logs(storage: &SqliteLogStorage, pending_logs: &mut Vec<LogEntry>) {
        if pending_logs.is_empty() {
            return;
        }

        if let Err(e) = storage.insert_log_entries_batch(pending_logs).await {
            error!("Failed to flush {} log entries: {}", pending_logs.len(), e);
        } else {
            debug!("Flushed {} log entries to storage", pending_logs.len());
        }

        pending_logs.clear();
    }
}
