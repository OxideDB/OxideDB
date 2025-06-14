//! SQLite database connection and core structure

use crate::db::SchemaAdapter;
use super::schema_adapter::SqliteSchemaAdapter;
use oxide_core::{
    AppError, AuthService, EventBus,
    event::RecordId,
};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;
use tracing::{info, debug};
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
}

impl SqliteDb {
    /// Create a new SqliteDb instance
    ///
    /// # Arguments
    /// * `database_path` - Path to the SQLite database file (use ":memory:" for in-memory)
    /// * `event_bus` - The event bus for dispatching events
    /// * `auth_service` - The authentication service for password hashing
    pub fn new(database_path: &str, event_bus: Arc<dyn EventBus>, auth_service: Arc<AuthService>) -> Result<Self, AppError> {
        let connection = Connection::open(database_path)
            .map_err(|e| AppError::database(format!("Failed to open SQLite database: {}", e)))?;

        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
            event_bus,
            auth_service,
            schema_adapter: SqliteSchemaAdapter::new(),
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

            info!("SQLite database initialized successfully");
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Initialize the database with tables and system collections
    pub async fn initialize(&self) -> Result<(), AppError> {
        use oxide_core::{AfterEventType, AfterEventContext};

        // Create database tables
        self.create_tables().await?;

        // Run migrations for existing data
        self.migrate_to_collection_tables().await?;

        // Dispatch OnSystemStartup event
        self.event_bus
            .dispatch_after(AfterEventType::SystemStartup, &AfterEventContext::SystemStartup)
            .await?;

        // Initialize system collections
        self.initialize_system_collections().await?;

        // Initialize authentication collections
        self.initialize_auth_collections().await?;

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
                    .map_err(|e| AppError::database(format!("Failed to check table existence: {}", e)))?
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
                                    bind_values.push(Box::new(value.as_str().unwrap_or("").to_string()));
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
                        table_name,
                        field_names.join(", "),
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
            let mut stmt = conn
                .prepare("SELECT 1")
                .map_err(|e| AppError::database(format!("Failed to prepare health check query: {}", e)))?;

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
        use oxide_core::{AfterEventType, AfterEventContext};

        info!("Closing SQLite database connection");

        // Dispatch OnSystemShutdown event
        self.event_bus
            .dispatch_after(AfterEventType::SystemShutdown, &AfterEventContext::SystemShutdown)
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
                .map_err(|e| AppError::database(format!("Failed to check collection existence: {}", e)))?;

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

            let count_sql = format!("SELECT COUNT(*) FROM {}", table_name);
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

        debug!("Counted {} records in collection table {}", count, table_name_for_logging);
        Ok(count)
    }
}

// Make sure auth and collections modules are included for their impl blocks 