//! Database abstraction layer
//!
//! This module defines the core database traits and types used throughout OxideDB.
//! The main `Db` trait provides a database-agnostic interface for CRUD operations,
//! while the `SchemaAdapter` trait handles database-specific schema operations.

use crate::Record;
use oxide_core::{AppError, CollectionSchema, CollectionPermissions};
use oxide_core::event::types::{RecordData, RecordId};
use oxide_core::auth::AuthCollectionConfig;
use oxide_core::user_preferences::UserPreferencesService;
use oxide_core::site_settings::SiteSettingsService;

/// Parameters for listing records
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct ListParams {
    /// Maximum number of records to return
    pub limit: Option<usize>,
    /// Number of records to skip (for pagination)
    pub offset: Option<usize>,
    /// Field to sort by
    pub sort_field: Option<String>,
    /// Whether to sort in ascending order (default: true)
    pub sort_ascending: Option<bool>,
    /// Whether to populate relationship fields (default: false)
    pub populate_relationships: Option<bool>,
    /// Specific relationship fields to populate (comma-separated field names)
    /// If provided, only these fields will be populated. If empty and populate_relationships is true, all relationships are populated.
    pub populate_fields: Option<String>,
}

/// Authentication request for generic auth collections
#[derive(Debug, Clone)]
pub struct AuthRequest {
    /// The auth collection to authenticate against
    pub collection: String,
    /// The identifier value (email, username, etc.)
    pub identifier: String,
    /// The credential value (password, etc.)
    pub credential: String,
}

/// Authentication response
#[derive(Debug, Clone)]
pub struct AuthResponse {
    /// The authenticated user's record ID
    pub user_id: String,
    /// JWT access token for the authenticated user
    pub token: String,
    /// Optional refresh token (if enabled for the collection)
    pub refresh_token: Option<String>,
    /// The auth collection the user authenticated from
    pub auth_collection: String,
    /// User's role
    pub role: String,
    /// Additional user data for custom claims
    pub user_data: serde_json::Value,
}

/// Registration request for generic auth collections
#[derive(Debug, Clone)]
pub struct RegisterRequest {
    /// The auth collection to register in
    pub collection: String,
    /// The identifier value (email, username, etc.)
    pub identifier: String,
    /// The credential value (password, etc.)
    pub credential: String,
    /// Additional user data
    pub additional_data: Option<serde_json::Value>,
}

/// Trait for database-specific schema operations
///
/// This trait abstracts the database-specific logic for handling collection schemas,
/// allowing different database implementations to provide their own SQL generation
/// and schema management logic.
pub trait SchemaAdapter {
    /// Get the table name for a collection
    fn get_table_name(&self, collection_name: &str) -> String;
    
    /// Generate SQL DDL for creating a collection's table
    fn generate_create_table_sql(&self, schema: &CollectionSchema) -> String;
    
    /// Generate SQL statements for creating indexes
    fn generate_index_sql(&self, schema: &CollectionSchema) -> Vec<String>;
    
    /// Convert a field type to the appropriate SQL column type
    fn field_type_to_sql(&self, field_type: &oxide_core::FieldType) -> &'static str;
    
    /// Generate SQL statements to migrate a table from old schema to new schema
    fn generate_migration_sql(&self, old_schema: &CollectionSchema, new_schema: &CollectionSchema) -> Vec<String>;
}

/// Main database trait for CRUD operations
///
/// This trait defines the core database operations that all database implementations
/// must provide. It follows the Hook-First Principle by dispatching events for
/// all operations, allowing plugins and listeners to hook into the process.
#[async_trait::async_trait]
pub trait Db: Send + Sync + UserPreferencesService + SiteSettingsService {
    /// Initialize the database connection and schema
    ///
    /// This method should establish the database connection, create necessary
    /// tables/collections, and perform any required setup.
    async fn initialize(&self) -> Result<(), AppError>;

    /// Create a new record in the specified collection
    ///
    /// This method dispatches `BeforeRecordCreate` and `AfterRecordCreate` events
    /// to allow plugins and listeners to hook into the creation process.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection to create the record in
    /// * `data` - The JSON data for the new record
    ///
    /// # Returns
    /// The created record with its assigned ID and timestamps
    async fn create_record(&self, collection: &str, data: RecordData) -> Result<Record, AppError>;

    /// Read a record by ID from the specified collection
    ///
    /// This method dispatches `BeforeRecordRead` and `AfterRecordRead` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    /// * `record_id` - The ID of the record to read
    ///
    /// # Returns
    /// The record if found, or a NotFound error
    async fn read_record(&self, collection: &str, record_id: &RecordId)
        -> Result<Record, AppError>;

    /// Update a record by ID in the specified collection
    ///
    /// This method dispatches `BeforeRecordUpdate` and `AfterRecordUpdate` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    /// * `record_id` - The ID of the record to update
    /// * `data` - The new JSON data for the record
    ///
    /// # Returns
    /// The updated record
    async fn update_record(
        &self,
        collection: &str,
        record_id: &RecordId,
        data: RecordData,
    ) -> Result<Record, AppError>;

    /// Delete a record by ID from the specified collection
    ///
    /// This method dispatches `BeforeRecordDelete` and `AfterRecordDelete` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    /// * `record_id` - The ID of the record to delete
    ///
    /// # Returns
    /// The deleted record
    async fn delete_record(&self, collection: &str, record_id: &RecordId)
        -> Result<Record, AppError>;

    /// List records from the specified collection
    ///
    /// This method dispatches `BeforeRecordList` and `AfterRecordList` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    /// * `params` - Parameters for filtering, sorting, and pagination
    ///
    /// # Returns
    /// A vector of records matching the criteria
    async fn list_records(&self, collection: &str, params: ListParams)
        -> Result<Vec<Record>, AppError>;

    /// Create a new collection with the given schema
    ///
    /// This method dispatches `BeforeCollectionCreate` and `AfterCollectionCreate` events.
    ///
    /// # Arguments
    /// * `schema` - The collection schema definition
    async fn create_collection(&self, schema: CollectionSchema) -> Result<(), AppError>;

    /// Get the schema for a collection
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    ///
    /// # Returns
    /// The collection schema if found
    async fn get_collection_schema(&self, collection: &str) -> Result<CollectionSchema, AppError>;

    /// Update the schema for a collection
    ///
    /// This method dispatches `BeforeCollectionUpdate` and `AfterCollectionUpdate` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    /// * `schema` - The new schema definition
    async fn update_collection_schema(&self, collection: &str, schema: CollectionSchema) -> Result<(), AppError>;

    /// Delete a collection and all its records
    ///
    /// This method dispatches `BeforeCollectionDelete` and `AfterCollectionDelete` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection to delete
    async fn delete_collection(&self, collection: &str) -> Result<(), AppError>;

    /// List all collections
    ///
    /// # Returns
    /// A vector of collection schemas
    async fn list_collections(&self) -> Result<Vec<CollectionSchema>, AppError>;

    /// Check if a collection exists
    ///
    /// # Arguments
    /// * `collection` - The name of the collection to check
    ///
    /// # Returns
    /// True if the collection exists, false otherwise
    async fn collection_exists(&self, collection: &str) -> Result<bool, AppError>;

    /// Get the number of records in a collection
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    ///
    /// # Returns
    /// The number of records in the collection
    async fn count_records(&self, collection: &str) -> Result<usize, AppError>;

    /// Get the size of a collection in kilobytes
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    ///
    /// # Returns
    /// The size of the collection in kilobytes
    async fn get_collection_size_kb(&self, collection: &str) -> Result<f64, AppError>;

    /// Close the database connection and clean up resources
    async fn close(&self) -> Result<(), AppError>;

    /// Check if the database connection is healthy
    async fn health_check(&self) -> Result<(), AppError>;

    /// Get dashboard statistics for the admin interface
    ///
    /// This method collects comprehensive statistics about the database
    /// including collection counts, record counts, and system health metrics.
    ///
    /// # Returns
    /// Dashboard statistics or an error if collection fails
    async fn get_dashboard_statistics(&self) -> Result<oxide_core::DashboardStats, AppError>;

    /// Get basic system statistics
    ///
    /// A lighter-weight version that returns only core metrics
    /// for situations where full statistics are not needed.
    ///
    /// # Returns
    /// Basic system statistics or an error if collection fails
    async fn get_system_statistics(&self) -> Result<oxide_core::SystemStats, AppError>;

    /// Get statistics for all collections
    ///
    /// # Returns
    /// Vector of collection statistics entries
    async fn get_collection_statistics(&self) -> Result<Vec<oxide_core::CollectionStatsEntry>, AppError>;

    /// Get storage usage information
    ///
    /// # Returns
    /// Storage usage statistics including database size and disk usage
    async fn get_storage_usage(&self) -> Result<oxide_core::StorageUsage, AppError>;

    /// Record an activity entry for the dashboard
    ///
    /// This method stores user and system activities for display
    /// in the dashboard activity feed.
    ///
    /// # Arguments
    /// * `activity` - The activity entry to record
    ///
    /// # Returns
    /// Success or an error if recording fails
    async fn record_dashboard_activity(&self, activity: oxide_core::ActivityEntry) -> Result<(), AppError>;

    /// Get recent activities for the dashboard
    ///
    /// # Arguments
    /// * `limit` - Maximum number of activities to return
    ///
    /// # Returns
    /// Vector of recent activities or an error if retrieval fails
    async fn get_recent_dashboard_activities(&self, limit: usize) -> Result<Vec<oxide_core::ActivityEntry>, AppError>;

    /// Store permissions for a collection
    ///
    /// # Arguments
    /// * `permissions` - The collection permissions to store
    async fn store_permissions(&self, permissions: &CollectionPermissions) -> Result<(), AppError>;

    /// Get permissions for a collection
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    ///
    /// # Returns
    /// The collection permissions if found, or None if no custom permissions exist
    async fn get_permissions(&self, collection: &str) -> Result<Option<CollectionPermissions>, AppError>;

    /// Delete permissions for a collection (revert to defaults)
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    async fn delete_permissions(&self, collection: &str) -> Result<(), AppError>;

    /// List all collections that have custom permissions
    ///
    /// # Returns
    /// A vector of collection names that have custom permissions
    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError>;

    /// Authenticate a user against a specific auth collection
    ///
    /// This method handles user authentication by finding the user in the specified
    /// auth collection, verifying the credential, and generating a JWT token.
    /// It dispatches authentication events through the hook system.
    ///
    /// # Arguments
    /// * `auth_request` - The authentication request containing collection, identifier, and credential
    /// * `auth_config` - The authentication configuration for the collection
    ///
    /// # Returns
    /// An AuthResponse containing user ID, token, and additional data if authentication succeeds
    async fn authenticate_user(&self, auth_request: AuthRequest, auth_config: &AuthCollectionConfig) -> Result<AuthResponse, AppError>;

    /// Register a new user in a specific auth collection
    ///
    /// This method creates a new user record in the specified auth collection
    /// using the standard record creation flow, which allows all validation,
    /// sanitization, and business logic to be handled by hooks.
    ///
    /// # Arguments
    /// * `register_request` - The registration request containing collection, identifier, credential, and additional data
    /// * `auth_config` - The authentication configuration for the collection
    ///
    /// # Returns
    /// The ID of the newly created user record
    async fn register_user(&self, register_request: RegisterRequest, auth_config: &AuthCollectionConfig) -> Result<String, AppError>;

    /// Find a user by identifier in a specific auth collection
    ///
    /// This is a helper method for authentication and user management operations.
    ///
    /// # Arguments
    /// * `collection` - The name of the auth collection
    /// * `identifier_field` - The field name to search by (e.g., "email", "username")
    /// * `identifier_value` - The value to search for
    ///
    /// # Returns
    /// The user record if found
    async fn find_user_by_identifier(&self, collection: &str, identifier_field: &str, identifier_value: &str) -> Result<Record, AppError>;

    /// List all auth collections
    ///
    /// This method returns all collections with CollectionType::Auth.
    ///
    /// # Returns
    /// A vector of auth collection schemas
    async fn list_auth_collections(&self) -> Result<Vec<CollectionSchema>, AppError>;

    /// Populate relationship fields in records
    ///
    /// This method takes a list of records and populates their relationship fields
    /// by fetching the related records from the target collections.
    ///
    /// # Arguments
    /// * `collection` - The name of the source collection
    /// * `records` - A mutable reference to the records to populate
    ///
    /// # Returns
    /// The populated records with relationship data
    async fn populate_relationships(&self, collection: &str, records: &mut [Record]) -> Result<(), AppError>;

    /// Populate specific relationship fields in records
    ///
    /// This method takes a list of records and populates only the specified relationship fields
    /// by fetching the related records from the target collections.
    ///
    /// # Arguments
    /// * `collection` - The name of the source collection
    /// * `records` - A mutable reference to the records to populate
    /// * `field_names` - A vector of field names to populate
    ///
    /// # Returns
    /// The populated records with relationship data for specified fields only
    async fn populate_specific_relationships(&self, collection: &str, records: &mut [Record], field_names: &[String]) -> Result<(), AppError>;

    /// Get related records for a specific relationship field
    ///
    /// This method retrieves related records for a specific relationship field value.
    ///
    /// # Arguments
    /// * `target_collection` - The name of the target collection
    /// * `record_ids` - The IDs of the records to fetch
    /// * `display_field` - Optional field to use as display value
    ///
    /// # Returns
    /// A map of record ID to the related record data
    async fn get_related_records(
        &self, 
        target_collection: &str, 
        record_ids: &[String],
        display_field: Option<&str>
    ) -> Result<std::collections::HashMap<String, serde_json::Value>, AppError>;
}
