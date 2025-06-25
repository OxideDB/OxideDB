//! VFS (Virtual File System) handlers
//!
//! This module provides HTTP handlers for file operations in OxideDB's virtual file system.
//! It supports secure file upload, download, and management within collection namespaces.

use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::{
    errors::ApiError,
    extractors::AuthenticatedUser,
    responses::ApiResponse,
    server::AppState,
};

use oxide_core::{
    VfsError, VfsNamespaceConfig, FileWriteRequest,
    FileReadRequest, FileListRequest, FileIdentifier
};

/// Result type for API handlers
pub type AppResult<T> = Result<T, ApiError>;

/// File upload response
#[derive(Debug, Serialize)]
pub struct FileUploadResponse {
    pub file_id: String,
    pub name: String,
    pub path: String,
    pub mime_type: String,
    pub size: u64,
    pub content_hash: String,
}

/// File list query parameters
#[derive(Debug, Deserialize)]
pub struct FileListQuery {
    pub directory: Option<String>,
    pub recursive: Option<bool>,
    pub mime_filter: Option<String>,
    pub tag_filter: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// File usage statistics
#[derive(Debug, Serialize)]
pub struct VfsUsageResponse {
    pub file_count: usize,
    pub storage_used: u64,
    pub storage_quota: Option<u64>,
    pub directory_count: usize,
}

/// Ensure a VFS namespace exists for the given collection
async fn ensure_namespace_exists(
    vfs_service: &dyn oxide_core::VirtualFileSystem,
    collection: &str,
) -> Result<(), VfsError> {
    // Check if namespace already exists
    let collection_string = collection.to_string();
    match vfs_service.get_namespace_config(&collection_string).await {
        Ok(_) => {
            // Namespace exists, nothing to do
            debug!("VFS namespace '{}' already exists", collection);
            Ok(())
        }
        Err(VfsError::AccessDenied { .. }) => {
            // Namespace doesn't exist, create it with default configuration
            info!("Creating VFS namespace for collection: {}", collection);
            
            let config = VfsNamespaceConfig {
                namespace: collection.to_string(),
                quota_bytes: Some(1024 * 1024 * 1024), // 1GB default
                enable_compression: true,
                allowed_mime_types: None, // Allow all by default
                max_file_size: Some(100 * 1024 * 1024), // 100MB default
                enable_deduplication: true,
                backup_config: None,
            };
            
            vfs_service.create_namespace(config).await?;
            info!("✅ Created VFS namespace for collection: {}", collection);
            Ok(())
        }
        Err(e) => {
            // Some other error occurred
            Err(e)
        }
    }
}

/// Upload a file to the VFS for a specific collection
pub async fn upload_file(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(collection): Path<String>,
    mut multipart: Multipart,
) -> AppResult<impl IntoResponse> {
    debug!("📁 Upload file request for collection: {}", collection);

    // Get VFS service from app state
    let vfs_service = state.vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection).await.map_err(|e| {
        error!("Failed to ensure VFS namespace exists for collection '{}': {:?}", collection, e);
        match e {
            VfsError::InvalidPath { .. } => ApiError::bad_request("Invalid collection name".to_string()),
            VfsError::FileAlreadyExists { .. } => ApiError::internal("Namespace creation conflict".to_string()),
            _ => ApiError::internal("Failed to initialize collection file storage".to_string()),
        }
    })?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let mut custom_path: Option<String> = None;

    // Process multipart form data
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        ApiError::bad_request(format!("Failed to read multipart data: {}", e))
    })? {
        let field_name = field.name().unwrap_or("");
        
        match field_name {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                mime_type = field.content_type().map(|s| s.to_string());
                
                file_data = Some(field.bytes().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read file data: {}", e))
                })?.to_vec());
            }
            "path" => {
                custom_path = Some(field.text().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read path field: {}", e))
                })?);
            }
            "collection" => {
                // Already have collection from path, but allow override
                let form_collection = field.text().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read collection field: {}", e))
                })?;
                if form_collection != collection {
                    warn!("Collection mismatch: path={}, form={}", collection, form_collection);
                }
            }
            _ => {
                debug!("Ignoring unknown multipart field: {}", field_name);
            }
        }
    }

    // Validate required fields
    let file_data = file_data.ok_or_else(|| {
        ApiError::bad_request("No file data provided".to_string())
    })?;

    let file_name = file_name.ok_or_else(|| {
        ApiError::bad_request("No filename provided".to_string())
    })?;

    let mime_type = mime_type.unwrap_or_else(|| {
        // Try to guess MIME type from file extension
        mime_guess::from_path(&file_name)
            .first_or_octet_stream()
            .to_string()
    });

    // Create namespace for the collection
    let namespace = collection.clone();

    // Generate file path
    let file_path = custom_path.unwrap_or_else(|| {
        format!("uploads/{}", file_name)
    });

    // Prepare file write request
    let write_request = FileWriteRequest {
        path: file_path.clone(),
        content: file_data,
        mime_type: Some(mime_type.clone()),
        custom_metadata: Some(std::collections::HashMap::new()),
        tags: None,
        overwrite: false,
    };

    // Write file to VFS
    let file_metadata = vfs_service.write_file(&namespace, write_request).await.map_err(|e| {
        error!("Failed to write file to VFS: {:?}", e);
        match e {
            VfsError::InvalidPath { .. } => ApiError::bad_request("Invalid file path".to_string()),
            VfsError::AccessDenied { .. } => ApiError::forbidden("Permission denied".to_string()),
            VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
            _ => ApiError::internal("File upload failed".to_string()),
        }
    })?;

    info!("📁 File uploaded successfully: {} ({})", file_name, file_metadata.id);

    let response = FileUploadResponse {
        file_id: file_metadata.id,
        name: file_metadata.name,
        path: file_metadata.path,
        mime_type: file_metadata.mime_type,
        size: file_metadata.size,
        content_hash: file_metadata.content_hash,
    };

    Ok(ApiResponse::success(response))
}

/// Download a file from the VFS
pub async fn download_file(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path((collection, file_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    debug!("📁 Download file request: {} from collection: {}", file_id, collection);

    // Get VFS service from app state
    let vfs_service = state.vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection).await.map_err(|e| {
        error!("Failed to ensure VFS namespace exists for collection '{}': {:?}", collection, e);
        ApiError::internal("Failed to access collection file storage".to_string())
    })?;

    // Create namespace for the collection
    let namespace = collection;

    // Read file from VFS
    let read_request = FileReadRequest {
        identifier: FileIdentifier::Id(file_id.clone()),
        include_content: true,
    };

    let file_response = vfs_service.read_file(&namespace, read_request).await.map_err(|e| {
        error!("Failed to read file from VFS: {:?}", e);
        match e {
            VfsError::FileNotFound { .. } => ApiError::not_found("File not found".to_string()),
            VfsError::AccessDenied { .. } => ApiError::forbidden("Permission denied".to_string()),
            VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
            _ => ApiError::internal("File download failed".to_string()),
        }
    })?;

    // Build response with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, file_response.metadata.mime_type.clone())
        .header(header::CONTENT_DISPOSITION, 
                format!("attachment; filename=\"{}\"", file_response.metadata.name))
        .header(header::CONTENT_LENGTH, file_response.content.as_ref().map(|c| c.len()).unwrap_or(0));

    // Add file content to response
    let body = file_response.content.unwrap_or_default();
    
    response
        .body(axum::body::Body::from(body))
        .map_err(|e| ApiError::internal(format!("Failed to create response: {}", e)))
}

/// List files in a collection's VFS namespace
pub async fn list_files(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(collection): Path<String>,
    Query(query): Query<FileListQuery>,
) -> AppResult<impl IntoResponse> {
    debug!("📁 List files request for collection: {}", collection);

    // Get VFS service from app state
    let vfs_service = state.vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection).await.map_err(|e| {
        error!("Failed to ensure VFS namespace exists for collection '{}': {:?}", collection, e);
        ApiError::internal("Failed to access collection file storage".to_string())
    })?;

    // Create namespace for the collection
    let namespace = collection;

    // Prepare list request
    let list_request = FileListRequest {
        directory: query.directory.unwrap_or_default(),
        recursive: query.recursive.unwrap_or(false),
        mime_filter: query.mime_filter,
        tag_filter: query.tag_filter,
        offset: query.offset,
        limit: query.limit,
    };

    // List files in VFS
    let file_list = vfs_service.list_files(&namespace, list_request).await.map_err(|e| {
        error!("Failed to list files from VFS: {:?}", e);
        match e {
            VfsError::AccessDenied { .. } => ApiError::forbidden("Permission denied".to_string()),
            VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
            _ => ApiError::internal("File listing failed".to_string()),
        }
    })?;

    Ok(ApiResponse::success(file_list))
}

/// Delete a file from the VFS
pub async fn delete_file(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path((collection, file_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    debug!("📁 Delete file request: {} from collection: {}", file_id, collection);

    // Get VFS service from app state
    let vfs_service = state.vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection).await.map_err(|e| {
        error!("Failed to ensure VFS namespace exists for collection '{}': {:?}", collection, e);
        ApiError::internal("Failed to access collection file storage".to_string())
    })?;

    // Create namespace for the collection
    let namespace = collection;

    // Delete file from VFS
    vfs_service.delete_file(&namespace, FileIdentifier::Id(file_id.clone())).await.map_err(|e| {
        error!("Failed to delete file from VFS: {:?}", e);
        match e {
            VfsError::FileNotFound { .. } => ApiError::not_found("File not found".to_string()),
            VfsError::AccessDenied { .. } => ApiError::forbidden("Permission denied".to_string()),
            VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
            _ => ApiError::internal("File deletion failed".to_string()),
        }
    })?;

    info!("📁 File deleted successfully: {}", file_id);
    Ok(ApiResponse::success(crate::responses::EmptyResponse::deleted()))
}

/// Get VFS usage statistics
pub async fn get_usage_stats(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
) -> AppResult<impl IntoResponse> {
    debug!("📁 Get VFS usage statistics request");

    // Get VFS service from app state
    let vfs_service = state.vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // For now, get stats for the default namespace
    // TODO: Allow querying specific namespaces or aggregate across all
    let namespace = "default".to_string();

    let usage_stats = vfs_service.get_usage_stats(&namespace).await.map_err(|e| {
        error!("Failed to get VFS usage stats: {:?}", e);
        ApiError::internal("Failed to retrieve usage statistics".to_string())
    })?;

    let response = VfsUsageResponse {
        file_count: usage_stats.file_count,
        storage_used: usage_stats.storage_used,
        storage_quota: usage_stats.storage_quota,
        directory_count: usage_stats.directory_count,
    };

    Ok(ApiResponse::success(response))
} 