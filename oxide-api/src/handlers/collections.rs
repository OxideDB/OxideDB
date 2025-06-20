//! Collection management handlers
//!
//! This module provides HTTP handlers for collection-related operations
//! including CRUD operations, schema management, and statistics.

use axum::{extract::{Path, State}, http::StatusCode, response::Json};
use oxide_core::CollectionSchema;
use oxide_db::Db;
use serde::Serialize;
use std::sync::Arc;
use tracing::{debug, info};
use ts_rs::TS;

use crate::{
    errors::ApiError,
    responses::{ApiResponse, EmptyResponse},
    server::AppState,
};

/// Collection statistics response
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct CollectionStats {
    /// Collection name
    pub name: String,
    /// Number of records in the collection
    pub record_count: usize,
    /// Whether the collection exists
    pub exists: bool,
    /// Collection schema version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<u32>,
    /// Collection creation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    /// Last modification timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Handlers for collection operations
pub struct CollectionHandlers;

impl CollectionHandlers {
    /// Create a new collection with schema
    pub async fn create_collection(
        db: Arc<dyn Db>,
        schema: CollectionSchema,
    ) -> Result<(), ApiError> {
        debug!("Creating collection with schema: {}", schema.name);

        // Validate schema before creation
        Self::validate_collection_schema(&schema)?;

        db.create_collection(schema.clone()).await?;

        info!("Created collection: {}", schema.name);
        Ok(())
    }

    /// Delete a collection
    pub async fn delete_collection(db: Arc<dyn Db>, collection: String) -> Result<(), ApiError> {
        debug!("Deleting collection: {}", collection);

        // Check if collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        // Check if this is a system collection and prevent deletion
        let schema = db.get_collection_schema(&collection).await?;
        if schema.collection_type == oxide_core::collection::CollectionType::Auth {
            return Err(ApiError::forbidden("System collections cannot be deleted"));
        }

        db.delete_collection(&collection).await?;

        info!("Deleted collection: {}", collection);
        Ok(())
    }

    /// List all collections
    pub async fn list_collections(db: Arc<dyn Db>) -> Result<Vec<CollectionSchema>, ApiError> {
        debug!("Listing collections");

        let collection_schemas = db.list_collections().await?;

        debug!("Listed {} collections", collection_schemas.len());
        Ok(collection_schemas)
    }

    /// List all collections with relationship data populated
    pub async fn list_collections_with_relationships(db: Arc<dyn Db>) -> Result<Vec<CollectionSchema>, ApiError> {
        debug!("Listing collections with relationship data");

        let collection_schemas = db.list_collections().await?;

        debug!("Listed {} collections", collection_schemas.len());
        Ok(collection_schemas)
    }

    /// Get collection statistics
    pub async fn get_collection_stats(
        db: Arc<dyn Db>,
        collection: String,
    ) -> Result<CollectionStats, ApiError> {
        debug!("Getting stats for collection: {}", collection);

        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        let record_count = db.count_records(&collection).await?;

        let stats = CollectionStats {
            name: collection,
            record_count,
            exists,
            schema_version: None, // TODO: Track schema versions
            created_at: None,     // TODO: Track creation time
            updated_at: None,     // TODO: Track modification time
        };

        debug!("Retrieved stats for collection: {:?}", stats);
        Ok(stats)
    }

    /// Get collection schema
    pub async fn get_collection_schema(
        db: Arc<dyn Db>,
        collection: String,
    ) -> Result<CollectionSchema, ApiError> {
        debug!("Getting schema for collection: {}", collection);

        let schema = db.get_collection_schema(&collection).await?;

        debug!("Retrieved schema for collection: {}", collection);
        Ok(schema)
    }

    /// Update collection schema
    pub async fn update_collection_schema(
        db: Arc<dyn Db>,
        collection: String,
        schema: CollectionSchema,
    ) -> Result<(), ApiError> {
        debug!("Updating schema for collection: {}", collection);

        // Validate the new schema
        Self::validate_collection_schema(&schema)?;

        // Ensure the collection name matches
        if schema.name != collection {
            return Err(ApiError::bad_request(
                "Schema name must match collection name in URL",
            ));
        }

        // Check if collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        db.update_collection_schema(&collection, schema).await?;

        info!("Updated schema for collection: {}", collection);
        Ok(())
    }

    /// Validate collection schema
    fn validate_collection_schema(schema: &CollectionSchema) -> Result<(), ApiError> {
        // Check collection name
        if schema.name.is_empty() {
            return Err(ApiError::bad_request("Collection name cannot be empty"));
        }

        // Check for reserved names
        if schema.name.starts_with("_") {
            return Err(ApiError::bad_request(
                "Collection names cannot start with underscore (reserved for system collections)",
            ));
        }

        // Validate field definitions
        if schema.fields.is_empty() {
            return Err(ApiError::bad_request(
                "Collection must have at least one field",
            ));
        }

        // Check for duplicate field names
        let mut field_names = std::collections::HashSet::new();
        for field_name in schema.fields.keys() {
            if !field_names.insert(field_name) {
                return Err(ApiError::bad_request(format!(
                    "Duplicate field name: {}",
                    field_name
                )));
            }
        }

        Ok(())
    }
}

// HTTP Handler Functions

/// List all collections
///
/// GET /collections
pub async fn list_collections(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<CollectionSchema>>>, ApiError> {
    let collections = CollectionHandlers::list_collections(state.db).await?;
    Ok(Json(ApiResponse::success(collections)))
}

/// Create a new collection
///
/// POST /collections
pub async fn create_collection(
    State(state): State<AppState>,
    Json(schema): Json<CollectionSchema>,
) -> Result<(StatusCode, Json<EmptyResponse>), ApiError> {
    CollectionHandlers::create_collection(state.db, schema).await?;
    Ok((StatusCode::CREATED, Json(EmptyResponse::created())))
}

/// Delete a collection
///
/// DELETE /collections/{collection}
pub async fn delete_collection(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<(StatusCode, Json<EmptyResponse>), ApiError> {
    CollectionHandlers::delete_collection(state.db, collection).await?;
    Ok((StatusCode::OK, Json(EmptyResponse::deleted())))
}

/// Get collection statistics
///
/// GET /collections/{collection}/stats
pub async fn collection_stats(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<Json<ApiResponse<CollectionStats>>, ApiError> {
    let stats = CollectionHandlers::get_collection_stats(state.db, collection).await?;
    Ok(Json(ApiResponse::success(stats)))
}

/// Get collection schema
///
/// GET /collections/{collection}/schema
pub async fn collection_schema(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<Json<ApiResponse<CollectionSchema>>, ApiError> {
    let schema = CollectionHandlers::get_collection_schema(state.db, collection).await?;
    Ok(Json(ApiResponse::success(schema)))
}

/// Update collection schema
///
/// PUT /collections/{collection}/schema
pub async fn update_collection_schema(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(schema): Json<CollectionSchema>,
) -> Result<Json<EmptyResponse>, ApiError> {
    CollectionHandlers::update_collection_schema(state.db, collection, schema).await?;
    Ok(Json(EmptyResponse::updated()))
}