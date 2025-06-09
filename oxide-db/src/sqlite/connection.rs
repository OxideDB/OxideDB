//! SQLite database connection and core structure

use crate::Record;
use oxide_core::{
    AppError, AuthService, EventBus,
    event::RecordId,
};
use rusqlite::{Connection, Row};
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
        })
    }

    /// Generate a new UUID for a record
    pub(super) fn generate_record_id() -> RecordId {
        Uuid::new_v4().to_string()
    }

    /// Convert a SQLite row to a Record
    pub(super) fn row_to_record(row: &Row) -> Result<Record, rusqlite::Error> {
        let data_str: String = row.get("data")?;
        let data = serde_json::from_str(&data_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;

        Ok(Record {
            id: row.get("id")?,
            collection: row.get("collection")?,
            data,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
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
        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let count = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT COUNT(*) FROM records WHERE collection = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let count: i64 = stmt
                .query_row([&collection_name], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to count records: {}", e)))?;

            Ok::<usize, AppError>(count as usize)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Counted {} records in collection {}", count, collection);
        Ok(count)
    }
}

// Make sure auth and collections modules are included for their impl blocks 