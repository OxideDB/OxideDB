//! HTTP request handlers
//!
//! This module contains the handler functions for various API endpoints.
//! Each handler translates HTTP requests into database operations and
//! returns appropriate HTTP responses.

use oxide_core::event::{RecordData, RecordId};
use oxide_core::{AppError, CollectionSchema};
use oxide_db::{db::ListParams, Db, Record};
use serde::Serialize;
use std::sync::Arc;
use tracing::{debug, info};
use ts_rs::TS;

/// Handlers for record operations
pub struct RecordHandlers;

impl RecordHandlers {
    /// Create a new record in a collection
    ///
    /// POST /collections/{collection}/records
    pub async fn create_record(
        db: Arc<dyn Db>,
        collection: String,
        data: RecordData,
    ) -> Result<Record, AppError> {
        debug!("Creating record in collection: {}", collection);

        let record = db.create_record(&collection, data).await?;

        info!("Created record {} in collection {}", record.id, collection);
        Ok(record)
    }

    /// Get a record by ID
    ///
    /// GET /collections/{collection}/records/{id}
    pub async fn get_record(
        db: Arc<dyn Db>,
        collection: String,
        record_id: RecordId,
    ) -> Result<Record, AppError> {
        debug!(
            "Getting record {} from collection {}",
            record_id, collection
        );

        let record = db.read_record(&collection, &record_id).await?;

        debug!(
            "Retrieved record {} from collection {}",
            record_id, collection
        );
        Ok(record)
    }

    /// Update a record by ID
    ///
    /// PUT /collections/{collection}/records/{id}
    pub async fn update_record(
        db: Arc<dyn Db>,
        collection: String,
        record_id: RecordId,
        data: RecordData,
    ) -> Result<Record, AppError> {
        debug!("Updating record {} in collection {}", record_id, collection);

        let record = db.update_record(&collection, &record_id, data).await?;

        info!("Updated record {} in collection {}", record_id, collection);
        Ok(record)
    }

    /// Delete a record by ID
    ///
    /// DELETE /collections/{collection}/records/{id}
    pub async fn delete_record(
        db: Arc<dyn Db>,
        collection: String,
        record_id: RecordId,
    ) -> Result<Record, AppError> {
        debug!(
            "Deleting record {} from collection {}",
            record_id, collection
        );

        let record = db.delete_record(&collection, &record_id).await?;

        info!(
            "Deleted record {} from collection {}",
            record_id, collection
        );
        Ok(record)
    }

    /// List records in a collection
    ///
    /// GET /collections/{collection}/records
    pub async fn list_records(
        db: Arc<dyn Db>,
        collection: String,
        params: ListParams,
    ) -> Result<Vec<Record>, AppError> {
        debug!("Listing records in collection: {}", collection);

        let records = db.list_records(&collection, params).await?;

        debug!(
            "Listed {} records from collection {}",
            records.len(),
            collection
        );
        Ok(records)
    }
}

/// Handlers for collection operations
pub struct CollectionHandlers;

impl CollectionHandlers {
    /// Create a new collection with schema
    ///
    /// POST /collections
    pub async fn create_collection(db: Arc<dyn Db>, schema: CollectionSchema) -> Result<(), AppError> {
        debug!("Creating collection with schema: {}", schema.name);

        db.create_collection(schema.clone()).await?;

        info!("Created collection with schema: {}", schema.name);
        Ok(())
    }

    /// Delete a collection
    ///
    /// DELETE /collections/{collection}
    pub async fn delete_collection(db: Arc<dyn Db>, collection: String) -> Result<(), AppError> {
        debug!("Deleting collection: {}", collection);

        // Check if this is a system collection and prevent deletion
        let schema = db.get_collection_schema(&collection).await?;
        if schema.collection_type == oxide_core::collection::CollectionType::Auth {
            return Err(AppError::auth("System collections cannot be deleted"));
        }

        db.delete_collection(&collection).await?;

        info!("Deleted collection: {}", collection);
        Ok(())
    }

    /// List all collections
    ///
    /// GET /collections
    pub async fn list_collections(db: Arc<dyn Db>) -> Result<Vec<CollectionSchema>, AppError> {
        debug!("Listing collections");

        let collection_schemas = db.list_collections().await?;

        debug!("Listed {} collections", collection_schemas.len());
        Ok(collection_schemas)
    }

    /// Get collection statistics
    ///
    /// GET /collections/{collection}/stats
    pub async fn get_collection_stats(
        db: Arc<dyn Db>,
        collection: String,
    ) -> Result<CollectionStats, AppError> {
        debug!("Getting stats for collection: {}", collection);

        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(AppError::not_found("collection", &collection));
        }

        let record_count = db.count_records(&collection).await?;

        let stats = CollectionStats {
            name: collection,
            record_count,
            exists,
        };

        debug!("Retrieved stats for collection: {:?}", stats);
        Ok(stats)
    }

    /// Get collection schema
    ///
    /// GET /collections/{collection}/schema
    pub async fn get_collection_schema(
        db: Arc<dyn Db>,
        collection: String,
    ) -> Result<CollectionSchema, AppError> {
        debug!("Getting schema for collection: {}", collection);

        let schema = db.get_collection_schema(&collection).await?;

        debug!("Retrieved schema for collection: {}", collection);
        Ok(schema)
    }

    /// Update collection schema
    ///
    /// PUT /collections/{collection}/schema
    pub async fn update_collection_schema(
        db: Arc<dyn Db>,
        collection: String,
        schema: CollectionSchema,
    ) -> Result<(), AppError> {
        debug!("Updating schema for collection: {}", collection);

        db.update_collection_schema(&collection, schema).await?;

        info!("Updated schema for collection: {}", collection);
        Ok(())
    }
}

/// Collection statistics response
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct CollectionStats {
    pub name: String,
    pub record_count: usize,
    pub exists: bool,
}

/// Health check handler
pub struct HealthHandlers;

impl HealthHandlers {
    /// Perform a health check
    ///
    /// GET /health
    pub async fn health_check(db: Arc<dyn Db>) -> Result<HealthStatus, AppError> {
        debug!("Performing health check");

        let database_status = match db.health_check().await {
            Ok(()) => "healthy".to_string(),
            Err(e) => format!("unhealthy: {}", e),
        };

        let status = HealthStatus {
            status: if database_status == "healthy" {
                "ok"
            } else {
                "error"
            }
            .to_string(),
            database: database_status,
        };

        debug!("Health check result: {:?}", status);
        Ok(status)
    }
}

/// Health status response
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct HealthStatus {
    pub status: String,
    pub database: String,
}
