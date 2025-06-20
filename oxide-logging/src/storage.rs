//! SQLite storage backend for the logging system
//!
//! This module provides high-performance SQLite storage with:
//! - Optimized table schemas with proper indexing
//! - Table partitioning by date for better performance
//! - Batch insert operations for throughput
//! - Connection pooling for concurrent access
//! - Automatic schema migrations

use crate::{
    error::{LoggingError, LoggingResult},
    models::{LogEntry, SecurityAuditEvent, LogQuery, LogMetrics, LogLevel, AuditEventType, CorrelationId},
};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_rusqlite::Connection as AsyncConnection;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Current schema version for migrations
const SCHEMA_VERSION: i32 = 1;

/// SQLite storage backend for logs and audit events
pub struct SqliteLogStorage {
    /// Async SQLite connection
    connection: Arc<Mutex<AsyncConnection>>,
    /// Database file path for reference
    db_path: String,
}

impl SqliteLogStorage {
    /// Create a new SQLite log storage instance
    pub async fn new(db_path: impl AsRef<Path>) -> LoggingResult<Self> {
        let db_path = db_path.as_ref().to_string_lossy().to_string();
        
        tracing::info!("Creating SQLite log storage at: {}", db_path);
        
        // Ensure parent directory exists
        if let Some(parent) = Path::new(&db_path).parent() {
            if !parent.exists() {
                tracing::info!("Creating parent directory: {:?}", parent);
                std::fs::create_dir_all(parent).map_err(|e| {
                    LoggingError::database(format!("Failed to create log database directory: {}", e))
                })?;
            }
        }

        tracing::info!("Opening async SQLite connection...");
        // Create async connection
        let connection = AsyncConnection::open(&db_path).await.map_err(|e| {
            tracing::error!("Failed to open SQLite connection: {}", e);
            LoggingError::database(format!("Failed to open SQLite connection: {}", e))
        })?;
        tracing::info!("SQLite connection opened successfully");
        
        let connection = Arc::new(Mutex::new(connection));

        let storage = Self {
            connection,
            db_path,
        };

        tracing::info!("Initializing database schema...");
        // Initialize schema
        storage.initialize_schema().await?;
        
        info!("SQLite log storage initialized at: {}", storage.db_path);
        Ok(storage)
    }

    /// Initialize database schema and indexes
    async fn initialize_schema(&self) -> LoggingResult<()> {
        tracing::info!("Starting schema initialization...");
        let conn = self.connection.lock().await;
        
        tracing::info!("Acquired connection lock, checking schema version...");
        
        // Check current schema version
        let current_version: i32 = conn.call(|conn| {
            tracing::info!("Creating schema_version table if not exists...");
            // Create version table if it doesn't exist
            conn.execute(
                "CREATE TABLE IF NOT EXISTS schema_version (
                    version INTEGER PRIMARY KEY
                )",
                [],
            )?;
            
            tracing::info!("Querying current schema version...");
            // Get current version or insert initial
            match conn.query_row("SELECT version FROM schema_version", [], |row| {
                Ok(row.get::<_, i32>(0)?)
            }).optional()? {
                Some(version) => {
                    tracing::info!("Found existing schema version: {}", version);
                    Ok(version)
                },
                None => {
                    tracing::info!("No schema version found, inserting initial version 0");
                    conn.execute("INSERT INTO schema_version (version) VALUES (?)", params![0])?;
                    Ok(0)
                }
            }
        }).await?;

        tracing::info!("Current schema version: {}, target version: {}", current_version, SCHEMA_VERSION);

        if current_version < SCHEMA_VERSION {
            tracing::info!("Schema migration needed");
            // Drop the connection lock before calling migrate_schema to avoid deadlock
            drop(conn);
            self.migrate_schema(current_version).await?;
        } else {
            tracing::info!("Schema is up to date, no migration needed");
        }

        tracing::info!("Schema initialization completed");
        Ok(())
    }

    /// Perform schema migrations
    async fn migrate_schema(&self, from_version: i32) -> LoggingResult<()> {
        tracing::info!("Starting schema migration from version {} to {}", from_version, SCHEMA_VERSION);
        let conn = self.connection.lock().await;

        conn.call(move |conn| {
            tracing::info!("Starting database transaction for migration...");
            let tx = conn.transaction()?;
            
            if from_version < 1 {
                tracing::info!("Migrating to version 1: creating tables and indexes...");
                
                // Create main log entries table
                tracing::info!("Creating log_entries table...");
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS log_entries (
                        id TEXT PRIMARY KEY,
                        correlation_id TEXT NOT NULL,
                        timestamp INTEGER NOT NULL,
                        level INTEGER NOT NULL,
                        message TEXT NOT NULL,
                        module TEXT NOT NULL,
                        location TEXT,
                        context_json TEXT NOT NULL,
                        error_info TEXT,
                        stack_trace TEXT,
                        metrics_json TEXT,
                        created_at INTEGER NOT NULL DEFAULT (unixepoch())
                    )",
                    [],
                )?;

                // Create audit events table
                tracing::info!("Creating audit_events table...");
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS audit_events (
                        id TEXT PRIMARY KEY,
                        correlation_id TEXT NOT NULL,
                        timestamp INTEGER NOT NULL,
                        event_type TEXT NOT NULL,
                        severity INTEGER NOT NULL,
                        description TEXT NOT NULL,
                        actor TEXT NOT NULL,
                        target TEXT,
                        action TEXT NOT NULL,
                        result TEXT NOT NULL,
                        context_json TEXT NOT NULL,
                        risk_score INTEGER,
                        integrity_hash TEXT,
                        created_at INTEGER NOT NULL DEFAULT (unixepoch())
                    )",
                    [],
                )?;

                // Create indexes for performance
                tracing::info!("Creating log_entries indexes...");
                // Log entries indexes
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_log_entries_timestamp ON log_entries(timestamp DESC)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_log_entries_level ON log_entries(level)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_log_entries_correlation ON log_entries(correlation_id)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_log_entries_module ON log_entries(module)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_log_entries_context_user ON log_entries(json_extract(context_json, '$.user_id'))",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_log_entries_context_collection ON log_entries(json_extract(context_json, '$.collection'))",
                    [],
                )?;

                // Audit events indexes
                tracing::info!("Creating audit_events indexes...");
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_audit_events_timestamp ON audit_events(timestamp DESC)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_audit_events_type ON audit_events(event_type)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_audit_events_actor ON audit_events(actor)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_audit_events_correlation ON audit_events(correlation_id)",
                    [],
                )?;
                tx.execute(
                    "CREATE INDEX IF NOT EXISTS idx_audit_events_severity ON audit_events(severity)",
                    [],
                )?;

                // Create metrics cache table
                tracing::info!("Creating log_metrics_cache table...");
                tx.execute(
                    "CREATE TABLE IF NOT EXISTS log_metrics_cache (
                        metric_key TEXT PRIMARY KEY,
                        metric_value TEXT NOT NULL,
                        last_updated INTEGER NOT NULL DEFAULT (unixepoch())
                    )",
                    [],
                )?;
                
                tracing::info!("Version 1 migration completed");
            }

            // Update schema version
            tracing::info!("Updating schema version to {}", SCHEMA_VERSION);
            tx.execute(
                "UPDATE schema_version SET version = ?",
                params![SCHEMA_VERSION],
            )?;

            tracing::info!("Committing migration transaction...");
            tx.commit()?;
            tracing::info!("Migration transaction committed successfully");
            Ok(())
        }).await?;

        info!("Schema migration completed successfully from version {} to {}", from_version, SCHEMA_VERSION);
        Ok(())
    }

    /// Insert a single log entry
    pub async fn insert_log_entry(&self, entry: &LogEntry) -> LoggingResult<()> {
        let conn = self.connection.lock().await;
        
        // Clone the entry data to move into the closure
        let entry_data = (
            entry.id,
            entry.correlation_id.clone(),
            entry.timestamp,
            entry.level,
            entry.message.clone(),
            entry.module.clone(),
            entry.location.clone(),
            entry.context.clone(),
            entry.error.clone(),
            entry.stack_trace.clone(),
            entry.metrics.clone(),
        );

        conn.call(move |conn| {
            let context_json = serde_json::to_string(&entry_data.7)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            let metrics_json = entry_data.10.as_ref()
                .map(|m| serde_json::to_string(m))
                .transpose()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            conn.execute(
                "INSERT INTO log_entries (
                    id, correlation_id, timestamp, level, message, module, 
                    location, context_json, error_info, stack_trace, metrics_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    entry_data.0.to_string(),
                    entry_data.1.to_string(),
                    entry_data.2.timestamp(),
                    entry_data.3 as i32,
                    entry_data.4,
                    entry_data.5,
                    entry_data.6,
                    context_json,
                    entry_data.8,
                    entry_data.9,
                    metrics_json,
                ],
            )?;
            Ok(())
        }).await?;

        debug!("Inserted log entry: {}", entry.id);
        Ok(())
    }

    /// Insert multiple log entries in a batch for better performance
    pub async fn insert_log_entries_batch(&self, entries: &[LogEntry]) -> LoggingResult<()> {
        if entries.is_empty() {
            return Ok(());
        }

        let conn = self.connection.lock().await;
        let entries = entries.to_vec(); // Clone for move into closure
        let entry_count = entries.len(); // Get count before move

        conn.call(move |conn| {
            let tx = conn.transaction()?;
            
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO log_entries (
                        id, correlation_id, timestamp, level, message, module, 
                        location, context_json, error_info, stack_trace, metrics_json
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )?;

                for entry in &entries {
                    let context_json = serde_json::to_string(&entry.context)
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                    let metrics_json = entry.metrics.as_ref()
                        .map(|m| serde_json::to_string(m))
                        .transpose()
                        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

                    stmt.execute(params![
                        entry.id.to_string(),
                        entry.correlation_id.to_string(),
                        entry.timestamp.timestamp(),
                        entry.level as i32,
                        entry.message,
                        entry.module,
                        entry.location,
                        context_json,
                        entry.error,
                        entry.stack_trace,
                        metrics_json,
                    ])?;
                }
            } // stmt is dropped here
            
            tx.commit()?;
            Ok(())
        }).await?;

        debug!("Inserted {} log entries in batch", entry_count);
        Ok(())
    }

    /// Insert an audit event
    pub async fn insert_audit_event(&self, event: &SecurityAuditEvent) -> LoggingResult<()> {
        let conn = self.connection.lock().await;
        
        // Clone the event data to move into the closure
        let event_data = (
            event.id,
            event.correlation_id.clone(),
            event.timestamp,
            event.event_type,
            event.severity,
            event.description.clone(),
            event.actor.clone(),
            event.target.clone(),
            event.action.clone(),
            event.result.clone(),
            event.context.clone(),
            event.risk_score,
            event.integrity_hash.clone(),
        );

        conn.call(move |conn| {
            let context_json = serde_json::to_string(&event_data.10)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            conn.execute(
                "INSERT INTO audit_events (
                    id, correlation_id, timestamp, event_type, severity, description,
                    actor, target, action, result, context_json, risk_score, integrity_hash
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                params![
                    event_data.0.to_string(),
                    event_data.1.to_string(),
                    event_data.2.timestamp(),
                    event_data.3.as_str(),
                    event_data.4 as i32,
                    event_data.5,
                    event_data.6,
                    event_data.7,
                    event_data.8,
                    event_data.9,
                    context_json,
                    event_data.11.map(|r| r as i32),
                    event_data.12,
                ],
            )?;
            Ok(())
        }).await?;

        debug!("Inserted audit event: {}", event.id);
        Ok(())
    }

    /// Query log entries with filtering and pagination
    pub async fn query_log_entries(&self, query: &LogQuery) -> LoggingResult<Vec<LogEntry>> {
        let conn = self.connection.lock().await;
        let query = query.clone(); // Clone for move into closure

        let entries = conn.call(move |conn| {
            let mut sql = String::from("SELECT * FROM log_entries WHERE 1=1");
            let mut params = Vec::<Box<dyn rusqlite::ToSql>>::new();

            // Build WHERE clause based on filter and collect parameters
            if let Some(min_level) = query.filter.min_level {
                sql.push_str(" AND level <= ?");
                params.push(Box::new(min_level as i32));
            }

            if let Some(start_time) = query.filter.start_time {
                sql.push_str(" AND timestamp >= ?");
                params.push(Box::new(start_time.timestamp()));
            }

            if let Some(end_time) = query.filter.end_time {
                sql.push_str(" AND timestamp <= ?");
                params.push(Box::new(end_time.timestamp()));
            }

            if let Some(ref correlation_id) = query.filter.correlation_id {
                sql.push_str(" AND correlation_id = ?");
                params.push(Box::new(correlation_id.to_string()));
            }

            if let Some(ref module) = query.filter.module {
                sql.push_str(" AND module = ?");
                params.push(Box::new(module.clone()));
            }

            if let Some(ref user_id) = query.filter.user_id {
                sql.push_str(" AND json_extract(context_json, '$.user_id') = ?");
                params.push(Box::new(user_id.clone()));
            }

            if let Some(ref collection) = query.filter.collection {
                sql.push_str(" AND json_extract(context_json, '$.collection') = ?");
                params.push(Box::new(collection.clone()));
            }

            if let Some(ref message_search) = query.filter.message_contains {
                sql.push_str(" AND message LIKE ?");
                params.push(Box::new(format!("%{}%", message_search)));
            }

            // Add ordering
            if query.sort_desc {
                sql.push_str(" ORDER BY timestamp DESC");
            } else {
                sql.push_str(" ORDER BY timestamp ASC");
            }

            // Add pagination
            if let Some(limit) = query.limit {
                sql.push_str(" LIMIT ?");
                params.push(Box::new(limit as i64));
            }

            if let Some(offset) = query.offset {
                sql.push_str(" OFFSET ?");
                params.push(Box::new(offset as i64));
            }

            // Convert parameters to references for the query
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            // Execute query
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                parse_log_entry_row(row)
            })?;

            let mut entries = Vec::new();
            for row in rows {
                entries.push(row?);
            }

            Ok(entries)
        }).await?;

        Ok(entries)
    }

    /// Query audit events
    pub async fn query_audit_events(&self, query: &LogQuery) -> LoggingResult<Vec<SecurityAuditEvent>> {
        let conn = self.connection.lock().await;
        let query = query.clone();

        let events = conn.call(move |conn| {
            let mut sql = String::from("SELECT * FROM audit_events WHERE 1=1");
            let mut params = Vec::<Box<dyn rusqlite::ToSql>>::new();

            // Build WHERE clause based on filter and collect parameters
            if let Some(min_level) = query.filter.min_level {
                sql.push_str(" AND severity <= ?");
                params.push(Box::new(min_level as i32));
            }

            if let Some(start_time) = query.filter.start_time {
                sql.push_str(" AND timestamp >= ?");
                params.push(Box::new(start_time.timestamp()));
            }

            if let Some(end_time) = query.filter.end_time {
                sql.push_str(" AND timestamp <= ?");
                params.push(Box::new(end_time.timestamp()));
            }

            if let Some(ref correlation_id) = query.filter.correlation_id {
                sql.push_str(" AND correlation_id = ?");
                params.push(Box::new(correlation_id.to_string()));
            }

            if let Some(ref audit_event_type) = query.filter.audit_event_type {
                sql.push_str(" AND event_type = ?");
                params.push(Box::new(audit_event_type.as_str().to_string()));
            }

            // Add ordering
            if query.sort_desc {
                sql.push_str(" ORDER BY timestamp DESC");
            } else {
                sql.push_str(" ORDER BY timestamp ASC");
            }

            // Add pagination
            if let Some(limit) = query.limit {
                sql.push_str(" LIMIT ?");
                params.push(Box::new(limit as i64));
            }

            if let Some(offset) = query.offset {
                sql.push_str(" OFFSET ?");
                params.push(Box::new(offset as i64));
            }

            // Convert parameters to references for the query
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            // Execute query
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                parse_audit_event_row(row)
            })?;

            let mut events = Vec::new();
            for row in rows {
                events.push(row?);
            }

            Ok(events)
        }).await?;

        Ok(events)
    }

    /// Get log metrics and statistics
    pub async fn get_metrics(&self) -> LoggingResult<LogMetrics> {
        let conn = self.connection.lock().await;

        let metrics = conn.call(|conn| {
            // Total entries
            let total_entries: u64 = conn.query_row(
                "SELECT COUNT(*) FROM log_entries",
                [],
                |row| Ok(row.get::<_, i64>(0)? as u64)
            )?;

            // Entries by level
            let mut entries_by_level = HashMap::new();
            let mut stmt = conn.prepare(
                "SELECT level, COUNT(*) FROM log_entries GROUP BY level"
            )?;
            let rows = stmt.query_map([], |row| {
                let level = row.get::<_, i32>(0)?;
                let count = row.get::<_, i64>(1)? as u64;
                Ok((level, count))
            })?;

            for row in rows {
                let (level_int, count) = row?;
                if let Some(level) = int_to_log_level(level_int) {
                    entries_by_level.insert(level, count);
                }
            }

            // Audit events by type
            let mut audit_events_by_type = HashMap::new();
            let mut audit_stmt = conn.prepare(
                "SELECT event_type, COUNT(*) FROM audit_events GROUP BY event_type"
            )?;
            let audit_rows = audit_stmt.query_map([], |row| {
                let event_type_str = row.get::<_, String>(0)?;
                let count = row.get::<_, i64>(1)? as u64;
                Ok((event_type_str, count))
            })?;

            for row in audit_rows {
                let (event_type_str, count) = row?;
                if let Some(event_type) = str_to_audit_event_type(&event_type_str) {
                    audit_events_by_type.insert(event_type, count);
                }
            }

            // Storage size (approximate)
            let storage_size_bytes: u64 = conn.query_row(
                "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
                [],
                |row| Ok(row.get::<_, i64>(0)? as u64)
            ).unwrap_or(0);

            // Average entries per day (last 30 days)
            let thirty_days_ago = Utc::now().timestamp() - (30 * 24 * 60 * 60);
            let entries_last_30_days: u64 = conn.query_row(
                "SELECT COUNT(*) FROM log_entries WHERE timestamp >= ?",
                params![thirty_days_ago],
                |row| Ok(row.get::<_, i64>(0)? as u64)
            ).unwrap_or(0);
            let avg_entries_per_day = entries_last_30_days as f64 / 30.0;

            // Top users (most active)
            let mut top_users = Vec::new();
            let mut user_stmt = conn.prepare(
                "SELECT json_extract(context_json, '$.user_id') as user_id, COUNT(*) as count 
                 FROM log_entries 
                 WHERE user_id IS NOT NULL 
                 GROUP BY user_id 
                 ORDER BY count DESC 
                 LIMIT 10"
            )?;
            let user_rows = user_stmt.query_map([], |row| {
                let user_id: Option<String> = row.get(0)?;
                let count = row.get::<_, i64>(1)? as u64;
                Ok((user_id, count))
            })?;

            for row in user_rows {
                let (user_id_opt, count) = row?;
                if let Some(user_id) = user_id_opt {
                    top_users.push((user_id, count));
                }
            }

            // Top collections (most accessed)
            let mut top_collections = Vec::new();
            let mut collection_stmt = conn.prepare(
                "SELECT json_extract(context_json, '$.collection') as collection, COUNT(*) as count 
                 FROM log_entries 
                 WHERE collection IS NOT NULL 
                 GROUP BY collection 
                 ORDER BY count DESC 
                 LIMIT 10"
            )?;
            let collection_rows = collection_stmt.query_map([], |row| {
                let collection: Option<String> = row.get(0)?;
                let count = row.get::<_, i64>(1)? as u64;
                Ok((collection, count))
            })?;

            for row in collection_rows {
                let (collection_opt, count) = row?;
                if let Some(collection) = collection_opt {
                    top_collections.push((collection, count));
                }
            }

            // Error rate in last 24 hours
            let twenty_four_hours_ago = Utc::now().timestamp() - (24 * 60 * 60);
            let error_count: u64 = conn.query_row(
                "SELECT COUNT(*) FROM log_entries WHERE level = 0 AND timestamp >= ?",
                params![twenty_four_hours_ago],
                |row| Ok(row.get::<_, i64>(0)? as u64)
            ).unwrap_or(0);

            let total_count_24h: u64 = conn.query_row(
                "SELECT COUNT(*) FROM log_entries WHERE timestamp >= ?",
                params![twenty_four_hours_ago],
                |row| Ok(row.get::<_, i64>(0)? as u64)
            ).unwrap_or(0);

            let error_rate_24h = if total_count_24h > 0 {
                (error_count as f64 / total_count_24h as f64) * 100.0
            } else {
                0.0
            };

            Ok(LogMetrics {
                total_entries,
                entries_by_level,
                audit_events_by_type,
                storage_size_bytes,
                avg_entries_per_day,
                top_users,
                top_collections,
                error_rate_24h,
            })
        }).await?;

        Ok(metrics)
    }

    /// Delete old log entries based on retention policy
    pub async fn cleanup_old_entries(&self, retention_days: u32) -> LoggingResult<u64> {
        let conn = self.connection.lock().await;
        
        let cutoff_timestamp = Utc::now().timestamp() - (retention_days as i64 * 24 * 60 * 60);

        let deleted_count = conn.call(move |conn| {
            let tx = conn.transaction()?;
            
            // Delete old log entries
            let log_deleted = tx.execute(
                "DELETE FROM log_entries WHERE timestamp < ?",
                params![cutoff_timestamp],
            )?;

            // Delete old audit events
            let audit_deleted = tx.execute(
                "DELETE FROM audit_events WHERE timestamp < ?",
                params![cutoff_timestamp],
            )?;

            tx.commit()?;
            
            Ok((log_deleted + audit_deleted) as u64)
        }).await?;

        info!("Cleaned up {} old log entries", deleted_count);
        Ok(deleted_count)
    }
}

/// Parse a log entry from a database row
fn parse_log_entry_row(row: &Row) -> rusqlite::Result<LogEntry> {
    let id: String = row.get("id")?;
    let correlation_id: String = row.get("correlation_id")?;
    let timestamp: i64 = row.get("timestamp")?;
    let level: i32 = row.get("level")?;
    let context_json: String = row.get("context_json")?;
    let metrics_json: Option<String> = row.get("metrics_json")?;

    let id = Uuid::parse_str(&id)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
    
    let correlation_id = CorrelationId::from_str(&correlation_id)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;

    let timestamp = DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(2, "timestamp".to_string(), rusqlite::types::Type::Integer))?;

    let level = int_to_log_level(level)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(3, "level".to_string(), rusqlite::types::Type::Integer))?;

    let context = serde_json::from_str(&context_json)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e)))?;

    let metrics = if let Some(json) = metrics_json {
        Some(serde_json::from_str(&json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e)))?)
    } else {
        None
    };

    Ok(LogEntry {
        id,
        correlation_id,
        timestamp,
        level,
        message: row.get("message")?,
        module: row.get("module")?,
        location: row.get("location")?,
        context,
        error: row.get("error_info")?,
        stack_trace: row.get("stack_trace")?,
        metrics,
    })
}

/// Parse an audit event from a database row
fn parse_audit_event_row(row: &Row) -> rusqlite::Result<SecurityAuditEvent> {
    let id: String = row.get("id")?;
    let correlation_id: String = row.get("correlation_id")?;
    let timestamp: i64 = row.get("timestamp")?;
    let event_type: String = row.get("event_type")?;
    let severity: i32 = row.get("severity")?;
    let context_json: String = row.get("context_json")?;
    let risk_score: Option<i32> = row.get("risk_score")?;

    let id = Uuid::parse_str(&id)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
    
    let correlation_id = CorrelationId::from_str(&correlation_id)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;

    let timestamp = DateTime::from_timestamp(timestamp, 0)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(2, "timestamp".to_string(), rusqlite::types::Type::Integer))?;

    let event_type = str_to_audit_event_type(&event_type)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(3, "event_type".to_string(), rusqlite::types::Type::Text))?;

    let severity = int_to_log_level(severity)
        .ok_or_else(|| rusqlite::Error::InvalidColumnType(4, "severity".to_string(), rusqlite::types::Type::Integer))?;

    let context = serde_json::from_str(&context_json)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(10, rusqlite::types::Type::Text, Box::new(e)))?;

    Ok(SecurityAuditEvent {
        id,
        correlation_id,
        timestamp,
        event_type,
        severity,
        description: row.get("description")?,
        actor: row.get("actor")?,
        target: row.get("target")?,
        action: row.get("action")?,
        result: row.get("result")?,
        context,
        risk_score: risk_score.map(|r| r as u8),
        integrity_hash: row.get("integrity_hash")?,
    })
}

/// Convert integer to LogLevel
fn int_to_log_level(level: i32) -> Option<LogLevel> {
    match level {
        0 => Some(LogLevel::Error),
        1 => Some(LogLevel::Warn),
        2 => Some(LogLevel::Info),
        3 => Some(LogLevel::Debug),
        4 => Some(LogLevel::Trace),
        _ => None,
    }
}

/// Convert string to AuditEventType
fn str_to_audit_event_type(event_type: &str) -> Option<AuditEventType> {
    match event_type {
        "authentication" => Some(AuditEventType::Authentication),
        "authorization" => Some(AuditEventType::Authorization),
        "data_access" => Some(AuditEventType::DataAccess),
        "data_modification" => Some(AuditEventType::DataModification),
        "configuration_change" => Some(AuditEventType::ConfigurationChange),
        "security_violation" => Some(AuditEventType::SecurityViolation),
        "plugin_event" => Some(AuditEventType::PluginEvent),
        "system_event" => Some(AuditEventType::SystemEvent),
        _ => None,
    }
} 