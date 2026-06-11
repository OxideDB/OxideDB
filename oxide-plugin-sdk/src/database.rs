//! Database utilities for plugin database operations

use crate::{DatabaseResult, Host, PluginError, PluginResult, Record};

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
