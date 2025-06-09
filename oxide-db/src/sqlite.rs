//! SQLite implementation of the database interface

use crate::{
    db::{Db, ListParams},
    Record,
};
use oxide_core::{
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType, EventBus, 
    event::{RecordData, RecordId},
    AppError, AuthService, UserRole,
    CollectionSchema, CollectionType, FieldDefinition, FieldType,
};
use rusqlite::{Connection, Row};
use std::sync::{Arc, Mutex};
use tokio::task::spawn_blocking;
use tracing::{debug, info};
use uuid::Uuid;

/// SQLite implementation of the Db trait
///
/// This implementation uses SQLite as the underlying database and integrates
/// with the EventBus to dispatch events for all operations. The connection
/// is wrapped in Arc<Mutex<>> to allow safe concurrent access.
pub struct SqliteDb {
    connection: Arc<Mutex<Connection>>,
    event_bus: Arc<dyn EventBus>,
    auth_service: Arc<AuthService>,
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
    fn generate_record_id() -> RecordId {
        Uuid::new_v4().to_string()
    }

    /// Convert a SQLite row to a Record
    fn row_to_record(row: &Row) -> Result<Record, rusqlite::Error> {
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

    /// Initialize system collections like _collections
    async fn initialize_system_collections(&self) -> Result<(), AppError> {
        // Create _collections collection to track collection metadata
        let collections_schema = CollectionSchema::new("_collections".to_string(), CollectionType::Base);
        
        // Only create if it doesn't exist
        if !self.collection_exists("_collections").await? {
            self.create_collection_with_schema(collections_schema).await?;
            info!("✅ Created _collections system collection");
        } else {
            debug!("_collections system collection already exists");
        }
        Ok(())
    }

    /// Initialize authentication collections (users and superusers)
    async fn initialize_auth_collections(&self) -> Result<(), AppError> {
        use oxide_core::auth::create_auth_collections;
        
        let (users_schema, superusers_schema) = create_auth_collections();
        
        // Create users collection if it doesn't exist
        if !self.collection_exists("users").await? {
            self.create_collection_with_schema(users_schema).await?;
            info!("✅ Created users auth collection");
        } else {
            debug!("Users auth collection already exists");
        }
        
        // Create superusers collection if it doesn't exist
        if !self.collection_exists("superusers").await? {
            self.create_collection_with_schema(superusers_schema).await?;
            info!("✅ Created superusers auth collection");
        } else {
            debug!("Superusers auth collection already exists");
        }
        
        info!("✅ Auth collections initialized");
        Ok(())
    }

    /// Authenticate a user with email and password
    pub async fn authenticate_user(&self, email: &str, password: &str) -> Result<(String, String), AppError> {
        info!("🔑 Authenticating user: {}", email);
        
        // Create context for BeforeUserAuth event
        let mut auth_context = BeforeEventContext {
            collection: "auth".to_string(),
            data: serde_json::json!({"email": email}),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };
        
        // Dispatch BeforeUserAuth event
        self.event_bus
            .dispatch_before(BeforeEventType::UserAuth, &mut auth_context)
            .await?;

        // Try to find user in users collection first
        let user_record = match self.find_user_by_email(email, "users").await {
            Ok(record) => record,
            Err(_) => self.find_user_by_email(email, "superusers").await?,
        };

        // Verify password
        let password_hash = user_record.data.get("passwordHash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| AppError::auth("Invalid user data: missing password hash"))?;

        if !self.auth_service.verify_password(password, password_hash)? {
            return Err(AppError::auth("Invalid email or password"));
        }

        // Determine user role based on collection
        let role = if user_record.collection == "superusers" {
            UserRole::Superuser
        } else {
            UserRole::User
        };

        // Generate JWT token
        let token = self.auth_service.generate_token(
            user_record.id.clone(),
            email.to_string(),
            role,
        )?;

        // Dispatch AfterUserAuth event
        self.event_bus
            .dispatch_after(AfterEventType::UserAuthenticated, &AfterEventContext::UserAuthenticated {
                user_id: user_record.id.clone(),
                email: email.to_string(),
            })
            .await?;

        info!("✅ User authenticated successfully: {}", email);
        Ok((user_record.id, token))
    }

    /// Find a user by email in the specified collection
    async fn find_user_by_email(&self, email: &str, collection: &str) -> Result<crate::Record, AppError> {
        let records = self.list_records(collection, crate::db::ListParams::default()).await?;
        
        for record in records {
            if let Some(user_email) = record.data.get("email").and_then(|e| e.as_str()) {
                if user_email == email {
                    return Ok(record);
                }
            }
        }
        
        Err(AppError::not_found("user", email))
    }

    /// Register a new user
    /// 
    /// This method creates a user record using the standard create_record flow,
    /// allowing all validation, sanitization, and business logic to be handled by hooks.
    pub async fn register_user(&self, email: &str, password: &str, is_superuser: bool) -> Result<String, AppError> {
        let collection = if is_superuser { "superusers" } else { "users" };
        
        info!("📝 Registering new user in {} collection: {}", collection, email);

        // Check if user already exists (this is domain logic that should remain in DB layer)
        if self.find_user_by_email(email, "users").await.is_ok() || 
           self.find_user_by_email(email, "superusers").await.is_ok() {
            return Err(AppError::conflict("User with this email already exists"));
        }

        // Create minimal user data - hooks will handle validation, hashing, and field additions
        let user_data = serde_json::json!({
            "email": email,
            "password": password,
            "verified": !is_superuser // superusers are verified by default
        });

        // Use the standard record creation flow - hooks will handle all transformations
        let record = self.create_record(collection, user_data).await?;

        // Dispatch user registration event (this may be redundant if hooks already handle it)
        self.event_bus
            .dispatch_after(AfterEventType::UserRegistered, &AfterEventContext::UserRegistered {
                user_id: record.id.clone(),
                email: email.to_string(),
                metadata: record.data.clone(),
            })
            .await?;

        info!("✅ User registered successfully: {}", email);
        Ok(record.id)
    }
}

#[async_trait::async_trait]
impl Db for SqliteDb {
    async fn initialize(&self) -> Result<(), AppError> {
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

    async fn create_record(&self, collection: &str, data: RecordData) -> Result<Record, AppError> {
        // Create a mutable context for Before events (allows data transformation)
        let mut context = BeforeEventContext::new_create(collection.to_string(), data);
        
        // Dispatch BeforeRecordCreate event - handlers can modify the data
        self.event_bus
            .dispatch_before(BeforeEventType::RecordCreate, &mut context)
            .await?;
        
        // Extract the potentially modified data from the context
        let data = context.data;
        
        // Validate data against collection schema (after hook transformations)
        if let Ok(schema) = self.get_collection_schema(collection).await {
            if let Err(validation_error) = schema.validate_data(&data) {
                return Err(AppError::validation("data", &validation_error));
            }
        }

        let record_id = Self::generate_record_id();
        let collection = collection.to_string();
        let connection = self.connection.clone();

        let record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let data_str = serde_json::to_string(&data)
                .map_err(|e| AppError::database(format!("Failed to serialize data: {}", e)))?;

            conn.execute(
                "INSERT INTO records (id, collection, data, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                [&record_id, &collection, &data_str, &now.to_string(), &now.to_string()],
            )
            .map_err(|e| AppError::database(format!("Failed to insert record: {}", e)))?;

            Ok::<Record, AppError>(Record {
                id: record_id,
                collection,
                data,
                created_at: now,
                updated_at: now,
            })
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordCreate event
        self.event_bus
            .dispatch_after(AfterEventType::RecordCreated, &AfterEventContext::RecordCreated {
                collection: record.collection.clone(),
                record_id: record.id.clone(),
                data: record.data.clone(),
            })
            .await?;

        debug!(
            "Created record {} in collection {}",
            record.id, record.collection
        );
        Ok(record)
    }

    async fn read_record(
        &self,
        collection: &str,
        record_id: &RecordId,
    ) -> Result<Record, AppError> {
        // Create a mutable context for BeforeRecordRead event
        let mut context = BeforeEventContext {
            collection: collection.to_string(),
            data: serde_json::json!({"record_id": record_id}),
            metadata: serde_json::json!({}),
            record_id: Some(record_id.clone()),
            old_data: None,
        };

        // Dispatch BeforeRecordRead event
        self.event_bus
            .dispatch_before(BeforeEventType::RecordRead, &mut context)
            .await?;

        let collection = collection.to_string();
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        let record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT id, collection, data, created_at, updated_at FROM records WHERE id = ?1 AND collection = ?2")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let record = stmt
                .query_row([&record_id, &collection], Self::row_to_record)
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::not_found("record", &record_id)
                    }
                    _ => AppError::database(format!("Failed to query record: {}", e)),
                })?;

            Ok::<Record, AppError>(record)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordRead event
        self.event_bus
            .dispatch_after(AfterEventType::RecordRead, &AfterEventContext::RecordRead {
                collection: record.collection.clone(),
                record_id: record.id.clone(),
                data: record.data.clone(),
            })
            .await?;

        debug!(
            "Read record {} from collection {}",
            record.id, record.collection
        );
        Ok(record)
    }

    async fn update_record(
        &self,
        collection: &str,
        record_id: &RecordId,
        new_data: RecordData,
    ) -> Result<Record, AppError> {
        // First, get the existing record to include in events
        let old_record = self.read_record(collection, record_id).await?;

        // Create a mutable context for BeforeRecordUpdate event
        let mut context = BeforeEventContext::new_update(
            collection.to_string(),
            record_id.clone(),
            old_record.data.clone(),
            new_data,
        );

        // Dispatch BeforeRecordUpdate event
        self.event_bus
            .dispatch_before(BeforeEventType::RecordUpdate, &mut context)
            .await?;

        // Extract the potentially modified data from the context
        let new_data = context.data;

        // Validate data against collection schema
        if let Ok(schema) = self.get_collection_schema(collection).await {
            if let Err(validation_error) = schema.validate_data(&new_data) {
                return Err(AppError::validation("data", &validation_error));
            }
        }

        let collection = collection.to_string();
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        let updated_record = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;

            let data_str = serde_json::to_string(&new_data)
                .map_err(|e| AppError::database(format!("Failed to serialize data: {}", e)))?;

            let rows_affected = conn
                .execute(
                    "UPDATE records SET data = ?1, updated_at = ?2 WHERE id = ?3 AND collection = ?4",
                    [&data_str, &now.to_string(), &record_id, &collection],
                )
                .map_err(|e| AppError::database(format!("Failed to update record: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("record", &record_id));
            }

            Ok(Record {
                id: record_id,
                collection,
                data: new_data,
                created_at: old_record.created_at,
                updated_at: now,
            })
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordUpdate event
        self.event_bus
            .dispatch_after(AfterEventType::RecordUpdated, &AfterEventContext::RecordUpdated {
                collection: updated_record.collection.clone(),
                record_id: updated_record.id.clone(),
                old_data: old_record.data,
                new_data: updated_record.data.clone(),
            })
            .await?;

        debug!(
            "Updated record {} in collection {}",
            updated_record.id, updated_record.collection
        );
        Ok(updated_record)
    }

    async fn delete_record(
        &self,
        collection: &str,
        record_id: &RecordId,
    ) -> Result<Record, AppError> {
        // First, get the existing record to include in events
        let record = self.read_record(collection, record_id).await?;

        // Create a mutable context for BeforeRecordDelete event
        let mut context = BeforeEventContext::new_delete(
            collection.to_string(),
            record_id.clone(),
            record.data.clone(),
        );

        // Dispatch BeforeRecordDelete event
        self.event_bus
            .dispatch_before(BeforeEventType::RecordDelete, &mut context)
            .await?;

        let collection = collection.to_string();
        let record_id = record_id.clone();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let rows_affected = conn
                .execute(
                    "DELETE FROM records WHERE id = ?1 AND collection = ?2",
                    [&record_id, &collection],
                )
                .map_err(|e| AppError::database(format!("Failed to delete record: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("record", &record_id));
            }

            Ok(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterRecordDelete event
        self.event_bus
            .dispatch_after(AfterEventType::RecordDeleted, &AfterEventContext::RecordDeleted {
                collection: record.collection.clone(),
                record_id: record.id.clone(),
                data: record.data.clone(),
            })
            .await?;

        debug!(
            "Deleted record {} from collection {}",
            record.id, record.collection
        );
        Ok(record)
    }

    async fn list_records(
        &self,
        collection: &str,
        params: ListParams,
    ) -> Result<Vec<Record>, AppError> {
        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let records = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut query = "SELECT id, collection, data, created_at, updated_at FROM records WHERE collection = ?1".to_string();
            let mut bind_params: Vec<String> = vec![collection_name];

            // Add sorting
            if let Some(sort_field) = &params.sort_field {
                let direction = if params.sort_ascending.unwrap_or(true) {
                    "ASC"
                } else {
                    "DESC"
                };
                query.push_str(&format!(" ORDER BY {} {}", sort_field, direction));
            } else {
                query.push_str(" ORDER BY created_at ASC");
            }

            // Add limit and offset
            if let Some(limit) = params.limit {
                query.push_str(" LIMIT ?");
                bind_params.push(limit.to_string());
            }

            if let Some(offset) = params.offset {
                query.push_str(" OFFSET ?");
                bind_params.push(offset.to_string());
            }

            let mut stmt = conn
                .prepare(&query)
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let records: Result<Vec<Record>, rusqlite::Error> = stmt
                .query_map(
                    rusqlite::params_from_iter(bind_params.iter()),
                    Self::row_to_record,
                )
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            records.map_err(|e| AppError::database(format!("Failed to query records: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Listed {} records from collection {}", records.len(), collection);
        Ok(records)
    }

    async fn create_collection(&self, collection: &str) -> Result<(), AppError> {
        // Create a default base collection schema
        let schema = CollectionSchema::new(collection.to_string(), CollectionType::Base);
        self.create_collection_with_schema(schema).await
    }

    async fn create_collection_with_schema(&self, schema: CollectionSchema) -> Result<(), AppError> {
        // Create a mutable context for BeforeCollectionCreate event
        let mut context = BeforeEventContext {
            collection: schema.name.clone(),
            data: serde_json::to_value(&schema).unwrap_or_default(),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };

        // Dispatch BeforeCollectionCreate event
        self.event_bus
            .dispatch_before(BeforeEventType::CollectionCreate, &mut context)
            .await?;

        let connection = self.connection.clone();
        let schema_name = schema.name.clone();
        let schema_name_for_events = schema.name.clone();
        let schema_json = serde_json::to_string(&schema)
            .map_err(|e| AppError::database(format!("Failed to serialize schema: {}", e)))?;

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            conn.execute(
                "INSERT INTO collections (id, name, type, schema, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                [
                    &schema.id,
                    &schema.name,
                    &schema.collection_type.to_string(),
                    &schema_json,
                    &schema.created_at.to_string(),
                    &schema.updated_at.to_string(),
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: rusqlite::ErrorCode::ConstraintViolation,
                        ..
                    },
                    _,
                ) => AppError::conflict(&format!("Collection '{}' already exists", &schema_name)),
                _ => AppError::database(format!("Failed to create collection: {}", e)),
            })?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterCollectionCreate event
        self.event_bus
            .dispatch_after(AfterEventType::CollectionCreated, &AfterEventContext::CollectionCreated { 
                collection: schema_name_for_events.clone(),
            })
            .await?;

        info!("Created collection with schema: {}", schema_name_for_events);
        Ok(())
    }

    async fn get_collection_schema(&self, collection: &str) -> Result<CollectionSchema, AppError> {
        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let schema = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT schema FROM collections WHERE name = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let schema_json: String = stmt
                .query_row([&collection_name], |row| row.get(0))
                .map_err(|e| match e {
                    rusqlite::Error::QueryReturnedNoRows => {
                        AppError::not_found("collection", &collection_name)
                    }
                    _ => AppError::database(format!("Failed to query collection: {}", e)),
                })?;

            let schema: CollectionSchema = serde_json::from_str(&schema_json)
                .map_err(|e| AppError::database(format!("Failed to deserialize schema: {}", e)))?;

            Ok::<CollectionSchema, AppError>(schema)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(schema)
    }

    async fn update_collection_schema(&self, collection: &str, mut schema: CollectionSchema) -> Result<(), AppError> {
        let connection = self.connection.clone();
        let collection_name = collection.to_string();

        // Update the updated_at timestamp
        schema.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let schema_json = serde_json::to_string(&schema)
            .map_err(|e| AppError::database(format!("Failed to serialize schema: {}", e)))?;

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let rows_affected = conn
                .execute(
                    "UPDATE collections SET schema = ?1, updated_at = ?2 WHERE name = ?3",
                    [&schema_json, &schema.updated_at.to_string(), &collection_name],
                )
                .map_err(|e| AppError::database(format!("Failed to update collection schema: {}", e)))?;

            if rows_affected == 0 {
                return Err(AppError::not_found("collection", &collection_name));
            }

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        info!("Updated schema for collection: {}", collection);
        Ok(())
    }

    async fn delete_collection(&self, collection: &str) -> Result<(), AppError> {
        // Create a mutable context for BeforeCollectionDelete event
        let mut context = BeforeEventContext {
            collection: collection.to_string(),
            data: serde_json::json!({"collection": collection}),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };

        // Dispatch BeforeCollectionDelete event
        self.event_bus
            .dispatch_before(BeforeEventType::CollectionDelete, &mut context)
            .await?;

        let collection_name = collection.to_string();
        let collection_clone = collection_name.clone();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Delete all records in the collection first
            conn.execute(
                "DELETE FROM records WHERE collection = ?1",
                [&collection_name],
            )
            .map_err(|e| AppError::database(format!("Failed to delete records: {}", e)))?;

            // Delete the collection entry
            conn.execute(
                "DELETE FROM collections WHERE name = ?1",
                [&collection_name],
            )
            .map_err(|e| AppError::database(format!("Failed to delete collection: {}", e)))?;

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        // Dispatch AfterCollectionDelete event
        self.event_bus
            .dispatch_after(AfterEventType::CollectionDeleted, &AfterEventContext::CollectionDeleted {
                collection: collection_clone.clone(),
            })
            .await?;

        info!("Deleted collection: {}", collection_clone);
        Ok(())
    }

    async fn list_collections(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connection.clone();

        let collections = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT name FROM collections ORDER BY name")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let collection_names: Result<Vec<String>, rusqlite::Error> = stmt
                .query_map([], |row| row.get(0))
                .map_err(|e| AppError::database(format!("Failed to execute query: {}", e)))?
                .collect();

            collection_names
                .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("Listed {} collections", collections.len());
        Ok(collections)
    }

    async fn collection_exists(&self, collection: &str) -> Result<bool, AppError> {
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

    async fn count_records(&self, collection: &str) -> Result<usize, AppError> {
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

    async fn close(&self) -> Result<(), AppError> {
        info!("Closing SQLite database connection");

        // Dispatch OnSystemShutdown event
        self.event_bus
            .dispatch_after(AfterEventType::SystemShutdown, &AfterEventContext::SystemShutdown)
            .await?;

        info!("✅ Database connection closed");
        Ok(())
    }

    async fn health_check(&self) -> Result<(), AppError> {
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
} 