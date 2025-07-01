//! Record CRUD operation handlers
//!
//! This module provides HTTP handlers for record-related operations
//! including create, read, update, delete, and list operations.

use axum::{extract::{Path, Query, State}, http::StatusCode, response::Json};
use oxide_core::event::types::{RecordData, RecordId};
use oxide_db::{db::ListParams, Db, Record};
use std::sync::Arc;
use tracing::{debug, info};

use crate::{
    errors::ApiError,
    responses::{ApiResponse, PaginatedResponse},
    server::AppState,
};

/// Handlers for record operations
pub struct RecordHandlers;

impl RecordHandlers {
    /// Create a new record in a collection
    pub async fn create_record(
        db: Arc<dyn Db>,
        collection: String,
        mut data: RecordData,
    ) -> Result<Record, ApiError> {
        debug!("Creating record in collection: {}", collection);

        // Validate collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        // Apply default values for missing fields
        Self::apply_record_defaults(&db, &collection, &mut data).await?;

        // Validate record data against schema (including new validation rules)
        Self::validate_record_data(&db, &collection, &data).await?;

        let record = db.create_record(&collection, data).await?;

        info!("Created record {} in collection {}", record.id, collection);
        Ok(record)
    }

    /// Get a record by ID
    pub async fn get_record(
        db: Arc<dyn Db>,
        collection: String,
        record_id: RecordId,
    ) -> Result<Record, ApiError> {
        debug!(
            "Getting record {} from collection {}",
            record_id, collection
        );

        // Validate collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        let record = db.read_record(&collection, &record_id).await?;

        debug!(
            "Retrieved record {} from collection {}",
            record_id, collection
        );
        Ok(record)
    }

    /// Update a record by ID
    pub async fn update_record(
        db: Arc<dyn Db>,
        collection: String,
        record_id: RecordId,
        mut data: RecordData,
    ) -> Result<Record, ApiError> {
        debug!("Updating record {} in collection {}", record_id, collection);

        // Validate collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        // Validate record exists
        let _existing_record = db.read_record(&collection, &record_id).await?;

        // Apply default values for missing fields (partial updates)
        Self::apply_record_defaults(&db, &collection, &mut data).await?;

        // Validate record data against schema (including new validation rules)
        Self::validate_record_data(&db, &collection, &data).await?;

        let record = db.update_record(&collection, &record_id, data).await?;

        info!("Updated record {} in collection {}", record_id, collection);
        Ok(record)
    }

    /// Delete a record by ID
    pub async fn delete_record(
        db: Arc<dyn Db>,
        collection: String,
        record_id: RecordId,
    ) -> Result<Record, ApiError> {
        debug!(
            "Deleting record {} from collection {}",
            record_id, collection
        );

        // Validate collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        let record = db.delete_record(&collection, &record_id).await?;

        info!(
            "Deleted record {} from collection {}",
            record_id, collection
        );
        Ok(record)
    }

    /// List records in a collection with pagination
    pub async fn list_records(
        db: Arc<dyn Db>,
        collection: String,
        params: ListParams,
    ) -> Result<(Vec<Record>, u64), ApiError> {
        debug!("Listing records in collection: {}", collection);

        // Validate collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        let mut records = db.list_records(&collection, params.clone()).await?;
        let total_count = db.count_records(&collection).await? as u64;

        // Populate relationships if requested
        if params.populate_relationships.unwrap_or(false) {
            if let Some(ref populate_fields_str) = params.populate_fields {
                // Parse comma-separated field names and populate only specified fields
                let field_names: Vec<String> = populate_fields_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                
                if !field_names.is_empty() {
                    debug!("Populating specific relationship fields for records in collection: {} - fields: {:?}", collection, field_names);
                    db.populate_specific_relationships(&collection, &mut records, &field_names).await?;
                } else {
                    debug!("Populating all relationships for records in collection: {}", collection);
                    db.populate_relationships(&collection, &mut records).await?;
                }
            } else {
                debug!("Populating all relationships for records in collection: {}", collection);
                db.populate_relationships(&collection, &mut records).await?;
            }
        }

        debug!(
            "Listed {} records from collection {} (total: {})",
            records.len(),
            collection,
            total_count
        );
        
        Ok((records, total_count))
    }

    /// Validate record data against collection schema
    async fn validate_record_data(
        db: &Arc<dyn Db>,
        collection: &str,
        data: &RecordData,
    ) -> Result<(), ApiError> {
        // Get collection schema
        let schema = db.get_collection_schema(collection).await?;

        // Use the comprehensive validation system from oxide-core
        match schema.validate_data(data) {
            Ok(()) => {
                debug!("Record validation passed for collection: {}", collection);
                Ok(())
            }
            Err(validation_error) => {
                debug!("Record validation failed for collection {}: {}", collection, validation_error);
                Err(ApiError::bad_request(validation_error))
            }
        }
    }

    /// Apply default values to record data (when available)
    async fn apply_record_defaults(
        db: &Arc<dyn Db>,
        collection: &str,
        data: &mut RecordData,
    ) -> Result<(), ApiError> {
        // Get collection schema
        let schema = db.get_collection_schema(collection).await?;

        // Apply default values to missing fields
        if let serde_json::Value::Object(data_obj) = data {
            for (field_name, field_def) in &schema.fields {
                // Apply default value if field is missing and has a default
                if !data_obj.contains_key(field_name) {
                    if let Some(default_value) = &field_def.default {
                        data_obj.insert(field_name.clone(), default_value.clone());
                        debug!("Applied default value for field '{}' in collection '{}'", field_name, collection);
                    }
                }
            }
        }

        Ok(())
    }
}

// HTTP Handler Functions

/// Create a new record
///
/// POST /collections/{collection}/records
pub async fn create_record(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(data): Json<RecordData>,
) -> Result<(StatusCode, Json<ApiResponse<Record>>), ApiError> {
    let record = RecordHandlers::create_record(state.db, collection, data).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::success(record))))
}

/// Get a specific record
///
/// GET /collections/{collection}/records/{id}
pub async fn get_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, RecordId)>,
    Query(params): Query<ListParams>,
) -> Result<Json<ApiResponse<Record>>, ApiError> {
    let mut record = RecordHandlers::get_record(state.db.clone(), collection.clone(), id).await?;
    
    // Populate relationships if requested
    if params.populate_relationships.unwrap_or(false) {
        let mut records = vec![record];
        
        if let Some(ref populate_fields_str) = params.populate_fields {
            // Parse comma-separated field names and populate only specified fields
            let field_names: Vec<String> = populate_fields_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            
            if !field_names.is_empty() {
                state.db.populate_specific_relationships(&collection, &mut records, &field_names).await?;
            } else {
                state.db.populate_relationships(&collection, &mut records).await?;
            }
        } else {
            state.db.populate_relationships(&collection, &mut records).await?;
        }
        
        record = records.into_iter().next().unwrap();
    }
    
    Ok(Json(ApiResponse::success(record)))
}

/// Update a specific record
///
/// PUT /collections/{collection}/records/{id}
pub async fn update_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, RecordId)>,
    Json(data): Json<RecordData>,
) -> Result<Json<ApiResponse<Record>>, ApiError> {
    let record = RecordHandlers::update_record(state.db, collection, id, data).await?;
    Ok(Json(ApiResponse::success(record)))
}

/// Delete a specific record
///
/// DELETE /collections/{collection}/records/{id}
pub async fn delete_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, RecordId)>,
) -> Result<Json<ApiResponse<Record>>, ApiError> {
    let record = RecordHandlers::delete_record(state.db, collection, id).await?;
    Ok(Json(ApiResponse::success(record)))
}

/// List records in a collection
///
/// GET /collections/{collection}/records
pub async fn list_records(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<PaginatedResponse<Record>>, ApiError> {
    let (records, total_count) = RecordHandlers::list_records(state.db, collection, params.clone()).await?;
    
    // Calculate page from offset and limit
    let per_page = params.limit.unwrap_or(50) as u32;
    let page = if per_page > 0 {
        (params.offset.unwrap_or(0) / params.limit.unwrap_or(50)) + 1
    } else {
        1
    } as u32;
    
    let response = PaginatedResponse::new(records, page, per_page, total_count);
    Ok(Json(response))
} 