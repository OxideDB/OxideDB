//! Record data structures and utilities

use oxide_core::event::{RecordData, RecordId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A record in the database
///
/// This represents a single record/document in a collection, containing
/// an ID, the collection it belongs to, and its JSON data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct Record {
    /// Unique identifier for this record
    pub id: RecordId,
    /// The collection this record belongs to
    pub collection: String,
    /// The record's data as JSON
    #[ts(type = "Record<string, any>")]
    pub data: RecordData,
    /// Timestamp when the record was created (Unix timestamp in seconds)
    pub created_at: i64,
    /// Timestamp when the record was last updated (Unix timestamp in seconds)
    pub updated_at: i64,
}

impl Record {
    /// Create a new record with the given ID, collection, and data
    pub fn new(id: RecordId, collection: String, data: RecordData) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            id,
            collection,
            data,
            created_at: now,
            updated_at: now,
        }
    }

    /// Update the record's data and timestamp
    pub fn update_data(&mut self, new_data: RecordData) {
        self.data = new_data;
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
    }

    /// Get the age of the record in seconds
    pub fn age_seconds(&self) -> i64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now - self.created_at
    }

    /// Check if the record has been modified since creation
    pub fn is_modified(&self) -> bool {
        self.updated_at > self.created_at
    }
}
