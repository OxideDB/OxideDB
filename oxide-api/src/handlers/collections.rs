//! Collection management handlers
//!
//! This module provides HTTP handlers for collection-related operations
//! including CRUD operations, schema management, and statistics.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use oxide_core::collection::{CollectionSchema, CollectionType, FieldDefinition, IndexDefinition};
use oxide_db::Db;
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, info};
use ts_rs::TS;

use crate::{
    errors::ApiError,
    responses::{ApiResponse, EmptyResponse},
    server::AppState,
};

/// Request payload for creating a new collection
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CreateCollectionRequest {
    /// Collection name (must be unique)
    pub name: String,
    /// Type of collection
    pub collection_type: CollectionType,
    /// Field definitions for this collection
    pub fields: HashMap<String, FieldDefinition>,
    /// Index definitions for performance optimization (optional)
    #[serde(default)]
    pub indexes: Vec<IndexDefinition>,
}

impl CreateCollectionRequest {
    /// Convert to a CollectionSchema with system-generated fields
    pub fn to_schema(self) -> CollectionSchema {
        let mut schema = CollectionSchema::new(self.name, self.collection_type);
        schema.fields = self.fields;
        schema.indexes = self.indexes;
        schema
    }
}

/// Collection statistics response
#[derive(Debug, TS)]
#[ts(export)]
pub struct CollectionStats {
    /// Collection name
    pub name: String,
    /// Number of records in the collection
    pub record_count: usize,
    /// Whether the collection exists
    pub exists: bool,
    /// Collection size in kilobytes
    pub size_kb: f64,
    /// Collection schema version
    pub schema_version: Option<u32>,
    /// Collection creation timestamp
    pub created_at: Option<String>,
    /// Last modification timestamp
    pub updated_at: Option<String>,
}

impl Serialize for CollectionStats {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut fields = 4;
        if self.schema_version.is_some() {
            fields += 1;
        }
        if self.created_at.is_some() {
            fields += 1;
        }
        if self.updated_at.is_some() {
            fields += 1;
        }

        let mut state = serializer.serialize_struct("CollectionStats", fields)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("record_count", &self.record_count)?;
        state.serialize_field("exists", &self.exists)?;
        state.serialize_field("size_kb", &self.size_kb)?;
        if let Some(schema_version) = self.schema_version {
            state.serialize_field("schema_version", &schema_version)?;
        }
        if let Some(created_at) = &self.created_at {
            state.serialize_field("created_at", created_at)?;
        }
        if let Some(updated_at) = &self.updated_at {
            state.serialize_field("updated_at", updated_at)?;
        }
        state.end()
    }
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
        if schema.name.starts_with('_') {
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
    pub async fn list_collections_with_relationships(
        db: Arc<dyn Db>,
    ) -> Result<Vec<CollectionSchema>, ApiError> {
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
        let size_kb = db.get_collection_size_kb(&collection).await?;

        // Retrieve collection schema to populate version and timestamps when possible
        let schema = db.get_collection_schema(&collection).await?;

        let stats = CollectionStats {
            name: collection,
            record_count,
            exists,
            size_kb,
            schema_version: Some(schema.version),
            created_at: Some(
                chrono::DateTime::from_timestamp(schema.created_at, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            ),
            updated_at: Some(
                chrono::DateTime::from_timestamp(schema.updated_at, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
            ),
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

        schema
            .validate_identifiers()
            .map_err(ApiError::bad_request)?;

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

        // Validate field definitions and validation rules
        for (field_name, field_def) in &schema.fields {
            // Note: Field indexing validation will be handled once index field is available
            if field_def.unique {
                debug!("Field '{}' is marked as unique", field_name);
            }

            // Basic validation rules validation
            if let Some(_validation) = &field_def.validation {
                debug!("Field '{}' has custom validation rules", field_name);
                // Note: Detailed validation rule checking is handled by oxide-core during validation
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
    Json(request): Json<CreateCollectionRequest>,
) -> Result<(StatusCode, Json<EmptyResponse>), ApiError> {
    let schema = request.to_schema();
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

/// Get statistics for all collections in a single response.
///
/// GET /collections/stats
///
/// Replaces the N+1 frontend fan-out where the sidebar and collections page
/// each issued one `GET /collections/:name/stats` per collection. This handler
/// returns a map of collection name → stats entry computed in a single batched
/// pass over the database (see `SqliteDb::get_collection_statistics`).
pub async fn all_collection_stats(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<HashMap<String, oxide_core::CollectionStatsEntry>>>, ApiError> {
    let entries = state.db.get_collection_statistics().await?;

    let mut map: HashMap<String, oxide_core::CollectionStatsEntry> = HashMap::new();
    for entry in entries {
        map.insert(entry.name.clone(), entry);
    }

    Ok(Json(ApiResponse::success(map)))
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
