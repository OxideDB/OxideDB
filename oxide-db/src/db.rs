//! Database abstraction layer
//!
//! This module defines the core database traits and types used throughout OxideDB.
//! The main `Db` trait provides a database-agnostic interface for CRUD operations,
//! while the `SchemaAdapter` trait handles database-specific schema operations.

use crate::Record;
use oxide_core::{AppError, CollectionSchema, CollectionPermissions};
use oxide_core::event::{RecordData, RecordId};

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
}

/// Main database trait for CRUD operations
///
/// This trait defines the core database operations that all database implementations
/// must provide. It follows the Hook-First Principle by dispatching events for
/// all operations, allowing plugins and listeners to hook into the process.
#[async_trait::async_trait]
pub trait Db: Send + Sync {
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

    /// Close the database connection and clean up resources
    async fn close(&self) -> Result<(), AppError>;

    /// Check if the database connection is healthy
    async fn health_check(&self) -> Result<(), AppError>;

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
}
