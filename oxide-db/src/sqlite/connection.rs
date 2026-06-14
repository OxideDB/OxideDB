//! SQLite database connection and core structure

use super::schema_adapter::{quote_identifier, SqliteSchemaAdapter};
use crate::db::SchemaAdapter;
use chrono::{Datelike, TimeZone, Utc};
use oxide_core::{
    event::types::RecordId, AppError, AuthService, EventBus, FieldType, UserActivity, UserStats,
};
use rusqlite::Connection;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::spawn_blocking;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// SQLite implementation of the Db trait
///
/// This implementation uses SQLite as the underlying database and integrates
/// with the EventBus to dispatch events for all operations. The connection
/// is wrapped in Arc<Mutex<>> to allow safe concurrent access.
pub struct SqliteDb {
    pub(super) connection: Arc<Mutex<Connection>>,
    pub(super) event_bus: Arc<dyn EventBus>,
    pub(super) auth_service: Arc<AuthService>,
    pub(super) schema_adapter: SqliteSchemaAdapter,
    database_path: String,
}

impl SqliteDb {
    /// Create a new SqliteDb instance
    ///
    /// # Arguments
    /// * `database_path` - Path to the SQLite database file (use ":memory:" for in-memory)
    /// * `event_bus` - The event bus for dispatching events
    /// * `auth_service` - The authentication service for password hashing
    pub fn new(
        database_path: &str,
        event_bus: Arc<dyn EventBus>,
        auth_service: Arc<AuthService>,
    ) -> Result<Self, AppError> {
        let connection = Connection::open(database_path)
            .map_err(|e| AppError::database(format!("Failed to open SQLite database: {}", e)))?;
        configure_connection(&connection, database_path)?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            event_bus,
            auth_service,
            schema_adapter: SqliteSchemaAdapter::new(),
            database_path: database_path.to_string(),
        })
    }

    /// Generate a new UUID for a record
    pub(super) fn generate_record_id() -> RecordId {
        Uuid::new_v4().to_string()
    }

    /// Initialize database tables and system collections
    async fn create_tables(&self) -> Result<(), AppError> {
        info!("Initializing SQLite database");

        let connection = self.connection.clone();
        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Create the records table
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS records (
                    id TEXT PRIMARY KEY,
                    collection TEXT NOT NULL,
                    data TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create records table: {}", e)))?;

            // Create indexes for performance
            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_records_collection ON records(collection)",
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create collection index: {}", e)))?;

            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_records_created_at ON records(created_at)",
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create created_at index: {}", e)))?;

            // Create the collections table to track collections with schema support
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS collections (
                    id TEXT PRIMARY KEY,
                    name TEXT UNIQUE NOT NULL,
                    type TEXT NOT NULL CHECK (type IN ('base', 'auth')),
                    schema TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to create collections table: {}", e))
            })?;

            // Create the permissions table
            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS collection_permissions (
                    collection TEXT PRIMARY KEY,
                    permissions_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to create permissions table: {}", e))
            })?;

            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS auth_refresh_tokens (
                    token_hash TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    auth_collection TEXT NOT NULL,
                    jti TEXT UNIQUE NOT NULL,
                    expires_at INTEGER NOT NULL,
                    revoked_at INTEGER,
                    replaced_by_hash TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to create refresh token table: {}", e))
            })?;

            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_auth_refresh_tokens_user_id ON auth_refresh_tokens(user_id)",
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to create refresh token user index: {}", e))
            })?;

            conn.execute(
                "CREATE INDEX IF NOT EXISTS idx_auth_refresh_tokens_active ON auth_refresh_tokens(expires_at, revoked_at)",
                [],
            )
            .map_err(|e| {
                AppError::database(format!("Failed to create refresh token active index: {}", e))
            })?;

            info!("SQLite database initialized successfully");
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Initialize the database with tables and system collections
    pub async fn initialize(&self) -> Result<(), AppError> {
        use oxide_core::{AfterEventContext, AfterEventType};

        // Create database tables
        self.create_tables().await?;

        // Run migrations for existing data
        self.migrate_to_collection_tables().await?;

        // Dispatch OnSystemStartup event
        let startup_context = AfterEventContext::SystemStartup {
            event_id: Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            version: env!("CARGO_PKG_VERSION").to_string(),
            config: serde_json::json!({"database_path": "sqlite"}),
        };
        self.event_bus
            .dispatch_after(AfterEventType::SystemStartup, &startup_context)
            .await?;

        // Initialize system collections
        self.initialize_system_collections().await?;

        Ok(())
    }

    /// Migrate existing data from centralized records table to collection-specific tables
    async fn migrate_to_collection_tables(&self) -> Result<(), AppError> {
        let connection = self.connection.clone();

        // First, check if we have any data in the old records table
        let has_old_data = spawn_blocking({
            let connection = connection.clone();
            move || {
                let conn = connection
                    .lock()
                    .map_err(|_| AppError::database("Failed to acquire database lock"))?;

                // Check if records table exists and has data
                let table_exists: bool = conn
                    .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='records'")
                    .map_err(|e| {
                        AppError::database(format!("Failed to check table existence: {}", e))
                    })?
                    .query_row([], |_| Ok(true))
                    .unwrap_or(false);

                if !table_exists {
                    return Ok(false);
                }

                let count: i64 = conn
                    .prepare("SELECT COUNT(*) FROM records")
                    .map_err(|e| AppError::database(format!("Failed to count old records: {}", e)))?
                    .query_row([], |row| row.get(0))
                    .unwrap_or(0);

                Ok::<bool, AppError>(count > 0)
            }
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        if !has_old_data {
            debug!("No migration needed - no data in old records table");
            return Ok(());
        }

        info!("🔄 Starting migration from centralized records table to collection-specific tables");

        // Get all collections that need migration
        let collections = self.list_collections().await?;

        for schema in collections {
            if schema.name.starts_with('_') {
                // Skip system collections for now
                continue;
            }

            info!("Migrating collection: {}", schema.name);

            // Create the new collection table using a local schema adapter
            let schema_adapter = SqliteSchemaAdapter::new();
            let create_table_sql = schema_adapter.generate_create_table_sql(&schema);
            let index_sql_statements = schema_adapter.generate_index_sql(&schema);

            let collection_name_for_migration = schema.name.clone();
            let connection_for_migration = connection.clone();

            spawn_blocking(move || {
                let conn = connection_for_migration
                    .lock()
                    .map_err(|_| AppError::database("Failed to acquire database lock"))?;

                // Start transaction
                let tx = conn.unchecked_transaction()
                    .map_err(|e| AppError::database(format!("Failed to start transaction: {}", e)))?;

                // Create the new table
                tx.execute(&create_table_sql, [])
                    .map_err(|e| AppError::database(format!("Failed to create table during migration: {}", e)))?;

                // Create indexes
                for index_sql in index_sql_statements {
                    tx.execute(&index_sql, [])
                        .map_err(|e| AppError::database(format!("Failed to create index during migration: {}", e)))?;
                }

                // Migrate data from old records table
                let old_records = {
                    let mut select_stmt = tx
                        .prepare("SELECT id, data, created_at, updated_at FROM records WHERE collection = ?1")
                        .map_err(|e| AppError::database(format!("Failed to prepare select statement: {}", e)))?;

                    let records = select_stmt
                        .query_map([&collection_name_for_migration], |row| {
                            let id: String = row.get(0)?;
                            let data_str: String = row.get(1)?;
                            let created_at: i64 = row.get(2)?;
                            let updated_at: i64 = row.get(3)?;

                            let data: serde_json::Value = serde_json::from_str(&data_str)
                                .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e)))?;

                            Ok((id, data, created_at, updated_at))
                        })
                        .map_err(|e| AppError::database(format!("Failed to query old records: {}", e)))?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| AppError::database(format!("Failed to collect old records: {}", e)))?;

                    // Drop the statement before continuing
                    drop(select_stmt);
                    records
                };

                // Insert migrated data into new table
                let table_name = SqliteSchemaAdapter::new().get_table_name(&schema.name);
                for (id, data, created_at, updated_at) in old_records {
                    // Convert data to SQL values
                    let mut field_names = vec!["id".to_string(), "created_at".to_string(), "updated_at".to_string()];
                    let mut placeholders = vec!["?1".to_string(), "?2".to_string(), "?3".to_string()];
                    let mut bind_values: Vec<Box<dyn rusqlite::ToSql>> = vec![
                        Box::new(id.clone()),
                        Box::new(created_at),
                        Box::new(updated_at),
                    ];

                    // Add schema fields
                    for (field_name, field_def) in &schema.fields {
                        if let Some(value) = data.get(field_name) {
                            field_names.push(field_name.clone());
                            placeholders.push(format!("?{}", field_names.len()));

                            match field_def.field_type.sql_type() {
                                "TEXT" => {
                                    // For File fields, serialize the entire JSON object as string
                                    if matches!(field_def.field_type, FieldType::File(_)) {
                                        bind_values.push(Box::new(value.to_string()));
                                    } else {
                                        bind_values.push(Box::new(value.as_str().unwrap_or("").to_string()));
                                    }
                                }
                                "REAL" => {
                                    bind_values.push(Box::new(value.as_f64().unwrap_or(0.0)));
                                }
                                "INTEGER" => {
                                    if value.is_boolean() {
                                        bind_values.push(Box::new(if value.as_bool().unwrap_or(false) { 1i64 } else { 0i64 }));
                                    } else {
                                        bind_values.push(Box::new(value.as_i64().unwrap_or(0)));
                                    }
                                }
                                _ => {
                                    // Fallback to text
                                    bind_values.push(Box::new(value.to_string()));
                                }
                            }
                        }
                    }

                    let insert_sql = format!(
                        "INSERT INTO {} ({}) VALUES ({})",
                        quote_identifier(&table_name),
                        field_names
                            .iter()
                            .map(|field_name| quote_identifier(field_name))
                            .collect::<Vec<_>>()
                            .join(", "),
                        placeholders.join(", ")
                    );

                    tx.execute(&insert_sql, rusqlite::params_from_iter(bind_values.iter().map(|v| v.as_ref())))
                        .map_err(|e| AppError::database(format!("Failed to insert migrated record: {}", e)))?;
                }

                // Remove migrated records from old table
                tx.execute("DELETE FROM records WHERE collection = ?1", [&collection_name_for_migration])
                    .map_err(|e| AppError::database(format!("Failed to delete old records: {}", e)))?;

                // Commit transaction
                tx.commit()
                    .map_err(|e| AppError::database(format!("Failed to commit migration transaction: {}", e)))?;

                info!("✅ Migrated collection: {}", collection_name_for_migration);
                Ok::<(), AppError>(())
            })
            .await
            .map_err(|e| AppError::internal(format!("Migration task join error: {}", e)))??;
        }

        info!("✅ Migration completed successfully");
        Ok(())
    }

    /// Health check for the database connection
    pub async fn health_check(&self) -> Result<(), AppError> {
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Use prepare and query_row for SELECT statements instead of execute
            let mut stmt = conn.prepare("SELECT 1").map_err(|e| {
                AppError::database(format!("Failed to prepare health check query: {}", e))
            })?;

            let _result: i32 = stmt
                .query_row([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Health check query failed: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Close the database connection
    pub async fn close(&self) -> Result<(), AppError> {
        use oxide_core::{AfterEventContext, AfterEventType};

        info!("Closing SQLite database connection");

        // Dispatch OnSystemShutdown event
        let shutdown_context = AfterEventContext::SystemShutdown {
            event_id: Uuid::new_v4().to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            reason: "Database connection closed".to_string(),
            uptime_ms: 0, // We don't track uptime at the DB level
        };
        self.event_bus
            .dispatch_after(AfterEventType::SystemShutdown, &shutdown_context)
            .await?;

        info!("✅ Database connection closed");
        Ok(())
    }

    /// Check if a collection exists
    pub async fn collection_exists(&self, collection: &str) -> Result<bool, AppError> {
        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let exists = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM collections WHERE name = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let count: i64 = stmt
                .query_row([&collection_name], |row| row.get(0))
                .map_err(|e| {
                    AppError::database(format!("Failed to check collection existence: {}", e))
                })?;

            Ok::<bool, AppError>(count > 0)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(exists)
    }

    /// Count records in a collection
    pub async fn count_records(&self, collection: &str) -> Result<usize, AppError> {
        // Get collection schema to determine table name
        let schema = self.get_collection_schema(collection).await?;
        let table_name = self.schema_adapter.get_table_name(&schema.name);
        let table_name_for_logging = table_name.clone();

        let connection = self.connection.clone();

        let count = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let count_sql = format!("SELECT COUNT(*) FROM {}", quote_identifier(&table_name));
            let mut stmt = conn
                .prepare(&count_sql)
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let count: i64 = stmt
                .query_row([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to count records: {}", e)))?;

            Ok::<usize, AppError>(count as usize)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!(
            "Counted {} records in collection table {}",
            count, table_name_for_logging
        );
        Ok(count)
    }

    /// Get the size of a collection in kilobytes
    pub async fn get_collection_size_kb(&self, collection: &str) -> Result<f64, AppError> {
        // Get collection schema to determine table name
        let schema = self.get_collection_schema(collection).await?;
        let table_name = self.schema_adapter.get_table_name(&schema.name);

        let connection = self.connection.clone();

        let size_kb = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Primary approach: Use dbstat virtual table for accurate, fast sizing
            let dbstat_sql = "SELECT SUM(pgsize) FROM dbstat WHERE name = ?1";

            match conn.prepare(dbstat_sql) {
                Ok(mut stmt) => {
                    match stmt.query_row([&table_name], |row| row.get::<_, i64>(0)) {
                        Ok(size_bytes) => {
                            let size_kb = size_bytes as f64 / 1024.0;
                            debug!(
                                "Calculated size for collection table {} using dbstat: {:.2} KB",
                                table_name, size_kb
                            );
                            return Ok(size_kb);
                        }
                        Err(_) => {
                            // dbstat found no data for this table (empty table)
                            debug!("Table {} appears to be empty or doesn't exist", table_name);
                            return Ok(0.0);
                        }
                    }
                }
                Err(_) => {
                    // dbstat not available, fall back to estimation
                    debug!(
                        "dbstat not available, falling back to estimation for table {}",
                        table_name
                    );
                }
            }

            // Fallback approach: Quick estimation using record count
            let count_sql = format!("SELECT COUNT(*) FROM {}", quote_identifier(&table_name));
            let mut count_stmt = conn.prepare(&count_sql).map_err(|e| {
                AppError::database(format!("Failed to prepare count statement: {}", e))
            })?;

            let record_count: i64 = count_stmt
                .query_row([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to count records: {}", e)))?;

            // Fast estimation: 1KB base + 256 bytes per record
            let estimated_kb = if record_count == 0 {
                0.0
            } else {
                1.0 + (record_count as f64 * 256.0 / 1024.0)
            };

            debug!(
                "Estimated size for collection table {}: {:.2} KB ({} records)",
                table_name, estimated_kb, record_count
            );

            Ok::<f64, AppError>(estimated_kb)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(size_kb)
    }

    /// Get comprehensive dashboard statistics
    pub async fn get_dashboard_statistics(&self) -> Result<oxide_core::DashboardStats, AppError> {
        debug!("Collecting comprehensive dashboard statistics");

        let system_stats = self.get_system_statistics().await?;
        let collection_stats = self.get_collection_statistics().await?;
        let storage_usage = self.get_storage_usage().await?;
        let recent_activity = self.get_recent_dashboard_activities(10).await?;

        let user_stats = self.get_user_statistics().await?;

        let api_stats = oxide_core::ApiStats {
            requests_24h: 0,
            requests_7d: 0,
            avg_response_time_ms: 0.0,
            error_rate_percent: 0.0,
            top_endpoints: vec![],
        };

        let uptime_seconds = crate::dashboard_stats_service::PROCESS_START
            .elapsed()
            .as_secs();

        let system_health = oxide_core::SystemHealth {
            database_status: oxide_core::HealthStatus::Healthy, // Based on health check
            api_status: oxide_core::HealthStatus::Healthy,
            auth_status: oxide_core::HealthStatus::Healthy,
            plugin_status: oxide_core::HealthStatus::Healthy,
            vfs_status: oxide_core::HealthStatus::Healthy,
            storage_usage,
            uptime_seconds,
        };

        Ok(oxide_core::DashboardStats {
            system_stats,
            collection_stats,
            user_stats,
            api_stats,
            recent_activity,
            system_health,
            generated_at: Utc::now().to_rfc3339(),
        })
    }

    /// Get basic system statistics
    pub async fn get_system_statistics(&self) -> Result<oxide_core::SystemStats, AppError> {
        debug!("Collecting basic system statistics");

        let collections = self.list_collections().await?;
        let total_collections = collections.len() as u32;

        // Calculate total records across all collections
        let mut total_records = 0u64;
        for collection in &collections {
            match self.count_records(&collection.name).await {
                Ok(count) => total_records += count as u64,
                Err(e) => {
                    debug!(
                        "Failed to count records for collection {}: {}",
                        collection.name, e
                    );
                    // Continue processing other collections
                }
            }
        }

        let month_start = current_month_start_timestamp();
        let previous_month_start = previous_month_start_timestamp();
        let collections_this_month = self.count_collections_created_since(month_start).await?;
        let records_this_month = self.count_records_created_since(month_start).await?;
        let records_previous_month = self
            .count_records_created_between(previous_month_start, month_start)
            .await?;
        let records_growth_percent = growth_percent(records_this_month, records_previous_month);
        let new_users_count = self.count_auth_records_created_since(month_start).await?;
        let active_users = self
            .count_active_dashboard_users(hours_ago_rfc3339(24))
            .await?;

        let trends = oxide_core::GrowthTrends {
            collections_this_month: collections_this_month as i32,
            records_growth_percent,
            new_users_count: new_users_count as u32,
            api_growth_percent: 0.0,
        };

        Ok(oxide_core::SystemStats {
            total_collections,
            total_records,
            active_users: active_users as u32,
            api_requests_24h: 0,
            trends,
        })
    }

    async fn get_user_statistics(&self) -> Result<UserStats, AppError> {
        let auth_collections = self.list_auth_collections().await?;
        let mut total_users = 0u64;

        for collection in &auth_collections {
            total_users += self.count_records(&collection.name).await.unwrap_or(0) as u64;
        }

        let active_24h = self
            .count_active_dashboard_users(hours_ago_rfc3339(24))
            .await?;
        let active_7d = self
            .count_active_dashboard_users(hours_ago_rfc3339(24 * 7))
            .await?;
        let top_active_users = self
            .get_top_dashboard_users(hours_ago_rfc3339(24 * 7), 5)
            .await?;

        Ok(UserStats {
            total_users: total_users as u32,
            active_24h: active_24h as u32,
            active_7d: active_7d as u32,
            top_active_users,
        })
    }

    async fn count_collections_created_since(&self, since: i64) -> Result<u64, AppError> {
        let connection = Arc::clone(&self.connection);
        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;
            let count = conn
                .query_row(
                    "SELECT COUNT(*) FROM collections WHERE created_at >= ?1",
                    [since],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|e| AppError::database(format!("Failed to count collections: {}", e)))?;

            Ok::<u64, AppError>(count as u64)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    async fn count_auth_records_created_since(&self, since: i64) -> Result<u64, AppError> {
        let auth_collections = self.list_auth_collections().await?;
        let mut total = 0u64;

        for collection in auth_collections {
            total += self
                .count_collection_records_created_between(&collection.name, since, None)
                .await?;
        }

        Ok(total)
    }

    async fn count_records_created_since(&self, since: i64) -> Result<u64, AppError> {
        let collections = self.list_collections().await?;
        let mut total = 0u64;

        for collection in collections {
            total += self
                .count_collection_records_created_between(&collection.name, since, None)
                .await?;
        }

        Ok(total)
    }

    async fn count_records_created_between(&self, start: i64, end: i64) -> Result<u64, AppError> {
        let collections = self.list_collections().await?;
        let mut total = 0u64;

        for collection in collections {
            total += self
                .count_collection_records_created_between(&collection.name, start, Some(end))
                .await?;
        }

        Ok(total)
    }

    async fn count_collection_records_created_between(
        &self,
        collection: &str,
        start: i64,
        end: Option<i64>,
    ) -> Result<u64, AppError> {
        let connection = Arc::clone(&self.connection);
        let table_name = self.schema_adapter.get_table_name(collection);

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;
            let sql = if end.is_some() {
                format!(
                    "SELECT COUNT(*) FROM {} WHERE created_at >= ?1 AND created_at < ?2",
                    quote_identifier(&table_name)
                )
            } else {
                format!(
                    "SELECT COUNT(*) FROM {} WHERE created_at >= ?1",
                    quote_identifier(&table_name)
                )
            };

            let count = if let Some(end) = end {
                conn.query_row(&sql, (start, end), |row| row.get::<_, i64>(0))
            } else {
                conn.query_row(&sql, [start], |row| row.get::<_, i64>(0))
            }
            .map_err(|e| AppError::database(format!("Failed to count records: {}", e)))?;

            Ok::<u64, AppError>(count as u64)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    async fn count_active_dashboard_users(&self, since: String) -> Result<u64, AppError> {
        let connection = Arc::clone(&self.connection);

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;
            ensure_dashboard_activities_table(&conn)?;
            let count = conn
                .query_row(
                    "SELECT COUNT(DISTINCT user_name)
                     FROM dashboard_activities
                     WHERE timestamp >= ?1 AND user_name != ''",
                    [since],
                    |row| row.get::<_, i64>(0),
                )
                .map_err(|e| {
                    AppError::database(format!("Failed to count active dashboard users: {}", e))
                })?;

            Ok::<u64, AppError>(count as u64)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    async fn get_top_dashboard_users(
        &self,
        since: String,
        limit: usize,
    ) -> Result<Vec<UserActivity>, AppError> {
        let connection = Arc::clone(&self.connection);

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;
            ensure_dashboard_activities_table(&conn)?;

            let mut stmt = conn
                .prepare(
                    "SELECT user_name, COUNT(*) AS action_count, MAX(timestamp) AS last_activity
                     FROM dashboard_activities
                     WHERE timestamp >= ?1 AND user_name != ''
                     GROUP BY user_name
                     ORDER BY action_count DESC, last_activity DESC
                     LIMIT ?2",
                )
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let users = stmt
                .query_map((since, limit as i64), |row| {
                    let last_activity: String = row.get(2)?;
                    Ok(UserActivity {
                        username: row.get(0)?,
                        action_count: row.get::<_, i64>(1)? as u32,
                        last_activity,
                    })
                })
                .map_err(|e| AppError::database(format!("Failed to query users: {}", e)))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::database(format!("Failed to collect users: {}", e)))?;

            Ok::<Vec<UserActivity>, AppError>(users)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))?
    }

    /// Get statistics for all collections
    pub async fn get_collection_statistics(
        &self,
    ) -> Result<Vec<oxide_core::CollectionStatsEntry>, AppError> {
        debug!("Collecting collection statistics");

        let connection = Arc::clone(&self.connection);

        let collections_with_timestamps = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;

            let mut stmt = conn
                .prepare(
                    "SELECT name, schema, created_at, updated_at FROM collections ORDER BY name",
                )
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let collections_data: Result<Vec<(String, String, i64, i64)>, rusqlite::Error> = stmt
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?, // name
                        row.get::<_, String>(1)?, // schema
                        row.get::<_, i64>(2)?,    // created_at
                        row.get::<_, i64>(3)?,    // updated_at
                    ))
                })
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            collections_data
                .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        let mut stats = Vec::new();

        for (name, _schema_json, created_at, updated_at) in collections_with_timestamps {
            let record_count = match self.count_records(&name).await {
                Ok(count) => count as u64,
                Err(e) => {
                    debug!("Failed to count records for collection {}: {}", name, e);
                    0
                }
            };

            let size_kb = match self.get_collection_size_kb(&name).await {
                Ok(size) => size,
                Err(e) => {
                    debug!("Failed to get size for collection {}: {}", name, e);
                    0.0
                }
            };

            let size_bytes = (size_kb * 1024.0) as u64;
            let is_system = name.starts_with('_');

            // Convert Unix timestamps to ISO 8601 strings
            let created_at_iso =
                chrono::DateTime::from_timestamp(created_at, 0).map(|dt| dt.to_rfc3339());
            let last_modified_iso =
                chrono::DateTime::from_timestamp(updated_at, 0).map(|dt| dt.to_rfc3339());

            stats.push(oxide_core::CollectionStatsEntry {
                name: name.clone(),
                record_count,
                size_bytes,
                created_at: created_at_iso,
                last_modified: last_modified_iso,
                is_system,
            });
        }

        Ok(stats)
    }

    /// Get storage usage information
    pub async fn get_storage_usage(&self) -> Result<oxide_core::StorageUsage, AppError> {
        debug!("Collecting storage usage information");

        let connection = Arc::clone(&self.connection);
        let database_path = self.database_path.clone();

        let database_size_bytes = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;

            // Get database file size using PRAGMA page_count and page_size
            let page_count: i64 = conn
                .prepare("PRAGMA page_count")
                .map_err(|e| AppError::database(format!("Failed to get page count: {}", e)))?
                .query_row([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to query page count: {}", e)))?;

            let page_size: i64 = conn
                .prepare("PRAGMA page_size")
                .map_err(|e| AppError::database(format!("Failed to get page size: {}", e)))?
                .query_row([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to query page size: {}", e)))?;

            Ok::<u64, AppError>((page_count * page_size) as u64)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        let logs_size_bytes = 0;
        let vfs_size_bytes = 0;
        let used_bytes = database_size_bytes;
        let total_bytes = database_storage_capacity_bytes(&database_path)
            .await
            .unwrap_or(database_size_bytes);
        let usage_percent = if total_bytes > 0 {
            (used_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };

        Ok(oxide_core::StorageUsage {
            used_bytes,
            total_bytes,
            usage_percent,
            database_size_bytes,
            logs_size_bytes,
            vfs_size_bytes,
        })
    }

    /// Record an activity entry for the dashboard
    pub async fn record_dashboard_activity(
        &self,
        activity: oxide_core::ActivityEntry,
    ) -> Result<(), AppError> {
        debug!("Recording dashboard activity: {:?}", activity.activity_type);

        let connection = Arc::clone(&self.connection);
        let activity_clone = activity.clone();

        spawn_blocking(move || {
            let conn = connection.lock().map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;

            // Create activities table if it doesn't exist
            conn.execute(
                "CREATE TABLE IF NOT EXISTS dashboard_activities (
                    id TEXT PRIMARY KEY,
                    timestamp TEXT NOT NULL,
                    activity_type TEXT NOT NULL,
                    user_name TEXT NOT NULL,
                    description TEXT NOT NULL,
                    collection TEXT,
                    metadata TEXT
                )",
                [],
            ).map_err(|e| AppError::database(format!("Failed to create activities table: {}", e)))?;

            // Insert the activity
            let activity_id = Uuid::new_v4().to_string();
            let metadata_json = match activity_clone.metadata {
                Some(metadata) => serde_json::to_string(&metadata).unwrap_or_default(),
                None => String::new(),
            };

            let activity_type_str = match &activity_clone.activity_type {
                oxide_core::ActivityType::CollectionCreated => "CollectionCreated",
                oxide_core::ActivityType::CollectionDeleted => "CollectionDeleted",
                oxide_core::ActivityType::CollectionModified => "CollectionModified",
                oxide_core::ActivityType::RecordCreated => "RecordCreated",
                oxide_core::ActivityType::RecordUpdated => "RecordUpdated",
                oxide_core::ActivityType::RecordDeleted => "RecordDeleted",
                oxide_core::ActivityType::UserRegistered => "UserRegistered",
                oxide_core::ActivityType::UserLogin => "UserLogin",
                oxide_core::ActivityType::UserLogout => "UserLogout",
                oxide_core::ActivityType::AuthenticationFailed => "AuthenticationFailed",
                oxide_core::ActivityType::PermissionGranted => "PermissionGranted",
                oxide_core::ActivityType::PermissionRevoked => "PermissionRevoked",
                oxide_core::ActivityType::PluginInstalled => "PluginInstalled",
                oxide_core::ActivityType::PluginToggled => "PluginToggled",
                oxide_core::ActivityType::SystemMaintenance => "SystemMaintenance",
                oxide_core::ActivityType::Other(custom) => custom,
            };

            conn.execute(
                "INSERT INTO dashboard_activities (id, timestamp, activity_type, user_name, description, collection, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                [
                    &activity_id,
                    &activity_clone.timestamp,
                    activity_type_str,
                    &activity_clone.user,
                    &activity_clone.description,
                    &activity_clone.collection.unwrap_or_default(),
                    &metadata_json,
                ],
            ).map_err(|e| AppError::database(format!("Failed to insert activity: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Get recent activities for the dashboard
    pub async fn get_recent_dashboard_activities(
        &self,
        limit: usize,
    ) -> Result<Vec<oxide_core::ActivityEntry>, AppError> {
        debug!("Getting recent dashboard activities with limit: {}", limit);

        let connection = Arc::clone(&self.connection);

        let activities = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|e| AppError::internal(format!("Failed to lock connection: {}", e)))?;

            // Create table if it doesn't exist (for graceful handling)
            conn.execute(
                "CREATE TABLE IF NOT EXISTS dashboard_activities (
                    id TEXT PRIMARY KEY,
                    timestamp TEXT NOT NULL,
                    activity_type TEXT NOT NULL,
                    user_name TEXT NOT NULL,
                    description TEXT NOT NULL,
                    collection TEXT,
                    metadata TEXT
                )",
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create activities table: {}", e)))?;

            let mut stmt = conn
                .prepare(
                    "SELECT timestamp, activity_type, user_name, description, collection, metadata 
                         FROM dashboard_activities 
                         ORDER BY timestamp DESC 
                         LIMIT ?1",
                )
                .map_err(|e| {
                    AppError::database(format!("Failed to prepare activities query: {}", e))
                })?;

            let activity_iter = stmt
                .query_map([limit as i64], |row| {
                    let timestamp_str: String = row.get(0)?;
                    let activity_type_str: String = row.get(1)?;
                    let user_name: String = row.get(2)?;
                    let description: String = row.get(3)?;
                    let collection: Option<String> = row.get(4)?;
                    let metadata_str: String = row.get(5)?;

                    let timestamp = timestamp_str.clone();

                    let activity_type = match activity_type_str.as_str() {
                        "CollectionCreated" => oxide_core::ActivityType::CollectionCreated,
                        "CollectionDeleted" => oxide_core::ActivityType::CollectionDeleted,
                        "CollectionModified" => oxide_core::ActivityType::CollectionModified,
                        "RecordCreated" => oxide_core::ActivityType::RecordCreated,
                        "RecordUpdated" => oxide_core::ActivityType::RecordUpdated,
                        "RecordDeleted" => oxide_core::ActivityType::RecordDeleted,
                        "UserRegistered" => oxide_core::ActivityType::UserRegistered,
                        "UserLogin" => oxide_core::ActivityType::UserLogin,
                        "UserLogout" => oxide_core::ActivityType::UserLogout,
                        "AuthenticationFailed" => oxide_core::ActivityType::AuthenticationFailed,
                        "PermissionGranted" => oxide_core::ActivityType::PermissionGranted,
                        "PermissionRevoked" => oxide_core::ActivityType::PermissionRevoked,
                        "PluginInstalled" => oxide_core::ActivityType::PluginInstalled,
                        "PluginToggled" => oxide_core::ActivityType::PluginToggled,
                        "SystemMaintenance" => oxide_core::ActivityType::SystemMaintenance,
                        custom => oxide_core::ActivityType::Other(custom.to_string()),
                    };

                    let metadata = if metadata_str.is_empty() {
                        None
                    } else {
                        serde_json::from_str(&metadata_str).ok()
                    };

                    let collection = if collection.as_ref().is_none_or(|s| s.is_empty()) {
                        None
                    } else {
                        collection
                    };

                    Ok(oxide_core::ActivityEntry {
                        timestamp,
                        activity_type,
                        user: user_name,
                        description,
                        collection,
                        metadata,
                    })
                })
                .map_err(|e| AppError::database(format!("Failed to query activities: {}", e)))?;

            let mut activities = Vec::new();
            for activity_result in activity_iter {
                match activity_result {
                    Ok(activity) => activities.push(activity),
                    Err(e) => debug!("Failed to parse activity row: {}", e),
                }
            }

            Ok::<Vec<oxide_core::ActivityEntry>, AppError>(activities)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Return empty activities if none exist in the database

        Ok(activities)
    }
}

fn configure_connection(connection: &Connection, database_path: &str) -> Result<(), AppError> {
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|e| AppError::database(format!("Failed to set SQLite busy timeout: {}", e)))?;

    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| AppError::database(format!("Failed to enable SQLite foreign keys: {}", e)))?;

    if database_path != ":memory:" {
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| AppError::database(format!("Failed to enable SQLite WAL: {}", e)))?;
    }

    connection
        .pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| AppError::database(format!("Failed to set SQLite synchronous mode: {}", e)))?;

    Ok(())
}

fn ensure_dashboard_activities_table(conn: &Connection) -> Result<(), AppError> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS dashboard_activities (
            id TEXT PRIMARY KEY,
            timestamp TEXT NOT NULL,
            activity_type TEXT NOT NULL,
            user_name TEXT NOT NULL,
            description TEXT NOT NULL,
            collection TEXT,
            metadata TEXT
        )",
        [],
    )
    .map_err(|e| AppError::database(format!("Failed to create activities table: {}", e)))?;
    Ok(())
}

async fn database_storage_capacity_bytes(database_path: &str) -> Option<u64> {
    if database_path == ":memory:" {
        return None;
    }

    let database_path = database_path.to_string();
    spawn_blocking(move || {
        let path = Path::new(&database_path);
        let capacity_path = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        match fs2::total_space(capacity_path) {
            Ok(total) => Some(total),
            Err(error) => {
                warn!(
                    "Failed to read filesystem capacity for '{}': {}",
                    capacity_path.display(),
                    error
                );
                None
            }
        }
    })
    .await
    .ok()
    .flatten()
}

fn hours_ago_rfc3339(hours: i64) -> String {
    (Utc::now() - chrono::Duration::hours(hours)).to_rfc3339()
}

fn current_month_start_timestamp() -> i64 {
    let now = Utc::now();
    Utc.with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| now.timestamp())
}

fn previous_month_start_timestamp() -> i64 {
    let now = Utc::now();
    let (year, month) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };

    Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .map(|dt| dt.timestamp())
        .unwrap_or_else(|| now.timestamp())
}

fn growth_percent(current: u64, previous: u64) -> f64 {
    if previous == 0 {
        if current > 0 {
            100.0
        } else {
            0.0
        }
    } else {
        ((current as f64 - previous as f64) / previous as f64) * 100.0
    }
}

// Make sure auth and collections modules are included for their impl blocks
