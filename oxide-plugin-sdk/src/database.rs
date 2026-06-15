//! Database utilities for plugin database operations

use crate::{
    CollectionExists, CollectionStats, DatabaseResult, Host, PluginError, PluginResult, Record,
};

/// Database utilities for plugins
pub struct Database;

impl Database {
    /// Create a record in a collection
    pub fn create<T: serde::Serialize>(collection: &str, data: &T) -> PluginResult<DatabaseResult> {
        Host::create_record(collection, data)
    }

    /// Read records from a collection
    pub fn read(collection: &str) -> PluginResult<DatabaseResult> {
        Host::read_records(collection, None)
    }

    /// Read records from a collection with a filter
    pub fn read_with_filter(
        collection: &str,
        filter: &serde_json::Value,
    ) -> PluginResult<DatabaseResult> {
        Host::read_records(collection, Some(filter))
    }

    /// Update a record in a collection
    pub fn update<T: serde::Serialize>(
        collection: &str,
        record_id: &str,
        data: &T,
    ) -> PluginResult<DatabaseResult> {
        Host::update_record(collection, record_id, data)
    }

    /// Delete a record from a collection
    pub fn delete(collection: &str, record_id: &str) -> PluginResult<DatabaseResult> {
        Host::delete_record(collection, record_id)
    }

    /// Create a collection from a serialized collection schema
    pub fn create_collection<T: serde::Serialize>(schema: &T) -> PluginResult<DatabaseResult> {
        Host::create_collection(schema)
    }

    /// List collection schemas visible to the plugin
    pub fn list_collections() -> PluginResult<DatabaseResult> {
        Host::list_collections()
    }

    /// Get the schema for a collection
    pub fn get_collection_schema(collection: &str) -> PluginResult<DatabaseResult> {
        Host::get_collection_schema(collection)
    }

    /// Update the schema for an existing collection
    pub fn update_collection_schema<T: serde::Serialize>(
        collection: &str,
        schema: &T,
    ) -> PluginResult<DatabaseResult> {
        Host::update_collection_schema(collection, schema)
    }

    /// Delete a collection and its records
    pub fn delete_collection(collection: &str) -> PluginResult<DatabaseResult> {
        Host::delete_collection(collection)
    }

    /// Check whether a collection exists
    pub fn collection_exists(collection: &str) -> PluginResult<DatabaseResult> {
        Host::collection_exists(collection)
    }

    /// Get collection statistics
    pub fn get_collection_stats(collection: &str) -> PluginResult<DatabaseResult> {
        Host::get_collection_stats(collection)
    }

    /// List collection schemas and parse them into a caller-provided type
    pub fn list_collection_schemas<T: serde::de::DeserializeOwned>() -> PluginResult<Vec<T>> {
        let result = Self::list_collections()?;
        if result.is_success() {
            result.parse_data().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Read a collection schema and parse it into a caller-provided type
    pub fn get_collection_schema_as<T: serde::de::DeserializeOwned>(
        collection: &str,
    ) -> PluginResult<T> {
        let result = Self::get_collection_schema(collection)?;
        if result.is_success() {
            result.parse_data().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Check whether a collection exists and return a boolean
    pub fn collection_exists_bool(collection: &str) -> PluginResult<bool> {
        let result = Self::collection_exists(collection)?;
        if result.is_success() {
            result
                .parse_data::<CollectionExists>()
                .map(|response| response.exists)
                .map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Read collection statistics and parse them into the SDK stats type
    pub fn get_collection_stats_typed(collection: &str) -> PluginResult<CollectionStats> {
        let result = Self::get_collection_stats(collection)?;
        if result.is_success() {
            result.parse_data().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Create a typed record in a collection
    pub fn create_typed<T: serde::Serialize>(collection: &str, data: &T) -> PluginResult<Record> {
        let result = Self::create(collection, data)?;
        if result.is_success() {
            result.parse_record().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Read typed records from a collection
    pub fn read_typed(collection: &str) -> PluginResult<Vec<Record>> {
        let result = Self::read(collection)?;
        if result.is_success() {
            result.parse_records().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Read typed records with a filter
    pub fn read_typed_with_filter(
        collection: &str,
        filter: &serde_json::Value,
    ) -> PluginResult<Vec<Record>> {
        let result = Self::read_with_filter(collection, filter)?;
        if result.is_success() {
            result.parse_records().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Update a typed record
    pub fn update_typed<T: serde::Serialize>(
        collection: &str,
        record_id: &str,
        data: &T,
    ) -> PluginResult<Record> {
        let result = Self::update(collection, record_id, data)?;
        if result.is_success() {
            result.parse_record().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }

    /// Delete a typed record
    pub fn delete_typed(collection: &str, record_id: &str) -> PluginResult<Record> {
        let result = Self::delete(collection, record_id)?;
        if result.is_success() {
            result.parse_record().map_err(PluginError::JsonError)
        } else {
            Err(PluginError::ExecutionError(
                result
                    .error()
                    .unwrap_or("Unknown database error")
                    .to_string(),
            ))
        }
    }
}

/// Query builder for database operations
pub struct QueryBuilder {
    collection: String,
    filter: serde_json::Value,
}

impl QueryBuilder {
    /// Create a new query builder for a collection
    pub fn new(collection: &str) -> Self {
        Self {
            collection: collection.to_string(),
            filter: serde_json::json!({}),
        }
    }

    /// Add a filter condition
    pub fn filter<V: serde::Serialize>(mut self, key: &str, value: &V) -> PluginResult<Self> {
        if let Some(obj) = self.filter.as_object_mut() {
            obj.insert(key.to_string(), serde_json::to_value(value)?);
        }
        Ok(self)
    }

    /// Execute the query and return results
    pub fn execute(self) -> PluginResult<Vec<Record>> {
        Database::read_typed_with_filter(&self.collection, &self.filter)
    }

    /// Execute the query and return raw results
    pub fn execute_raw(self) -> PluginResult<DatabaseResult> {
        Database::read_with_filter(&self.collection, &self.filter)
    }
}

/// Record builder for creating new records
pub struct RecordBuilder {
    data: serde_json::Map<String, serde_json::Value>,
}

impl RecordBuilder {
    /// Create a new record builder
    pub fn new() -> Self {
        Self {
            data: serde_json::Map::new(),
        }
    }

    /// Set a field value
    pub fn set<V: serde::Serialize>(mut self, key: &str, value: &V) -> PluginResult<Self> {
        self.data
            .insert(key.to_string(), serde_json::to_value(value)?);
        Ok(self)
    }

    /// Create the record in the specified collection
    pub fn create(self, collection: &str) -> PluginResult<Record> {
        let data = serde_json::Value::Object(self.data);
        Database::create_typed(collection, &data)
    }

    /// Get the built data as JSON value
    pub fn build(self) -> serde_json::Value {
        serde_json::Value::Object(self.data)
    }
}

impl Default for RecordBuilder {
    fn default() -> Self {
        Self::new()
    }
}
