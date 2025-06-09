//! Database abstraction trait

use crate::Record;
use oxide_core::{
    event::{RecordData, RecordId},
    AppError,
};

/// Query parameters for listing records
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ListParams {
    /// Maximum number of records to return
    pub limit: Option<usize>,
    /// Number of records to skip
    pub offset: Option<usize>,
    /// Field to sort by
    pub sort_field: Option<String>,
    /// Sort direction (true for ascending, false for descending)
    pub sort_ascending: Option<bool>,
}

impl Default for ListParams {
    fn default() -> Self {
        Self {
            limit: None,
            offset: None,
            sort_field: None,
            sort_ascending: Some(true),
        }
    }
}

/// The database abstraction trait
///
/// This trait defines the interface for all database operations in OxideDB.
/// All implementations must integrate with the EventBus to dispatch events
/// before and after each operation, enabling the hook-first architecture.
///
/// The trait is designed to be async-first and error-safe, with all operations
/// returning `Result<T, AppError>`.
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
    async fn delete_record(
        &self,
        collection: &str,
        record_id: &RecordId,
    ) -> Result<Record, AppError>;

    /// List records in a collection with optional filtering and pagination
    ///
    /// # Arguments
    /// * `collection` - The name of the collection
    /// * `params` - Query parameters for filtering, pagination, and sorting
    ///
    /// # Returns
    /// A vector of records matching the criteria
    async fn list_records(
        &self,
        collection: &str,
        params: ListParams,
    ) -> Result<Vec<Record>, AppError>;

    /// Create a new collection
    ///
    /// This method dispatches `BeforeCollectionCreate` and `AfterCollectionCreate` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection to create
    async fn create_collection(&self, collection: &str) -> Result<(), AppError>;

    /// Delete a collection and all its records
    ///
    /// This method dispatches `BeforeCollectionDelete` and `AfterCollectionDelete` events.
    ///
    /// # Arguments
    /// * `collection` - The name of the collection to delete
    async fn delete_collection(&self, collection: &str) -> Result<(), AppError>;

    /// List all collections in the database
    ///
    /// # Returns
    /// A vector of collection names
    async fn list_collections(&self) -> Result<Vec<String>, AppError>;

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
}
