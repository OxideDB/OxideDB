//! VFS (Virtual File System) handlers
//!
//! This module provides HTTP handlers for file operations in OxideDB's virtual file system.
//! It supports secure file upload, download, and management within collection namespaces.

use axum::{
    extract::{Json, Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};

use oxide_core::{
    FileIdentifier, FileListRequest, FileMoveRequest, FileReadRequest, FileWriteRequest, VfsError,
    VfsNamespaceConfig, VfsUsageStats,
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

/// File move request payload
#[derive(Debug, Deserialize)]
pub struct MoveFilePayload {
    /// Destination path inside the collection namespace
    pub path: String,
    /// Whether to replace an existing file at the destination path
    pub overwrite: Option<bool>,
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

/// File usage query parameters
#[derive(Debug, Deserialize)]
pub struct VfsUsageQuery {
    pub namespace: Option<String>,
    pub include_default: Option<bool>,
}

/// Aggregate file usage statistics
#[derive(Debug, Serialize)]
pub struct VfsUsageSummaryResponse {
    pub namespace: String,
    pub file_count: usize,
    pub storage_used: u64,
    pub storage_quota: Option<u64>,
    pub directory_count: usize,
    pub last_updated: u64,
    pub namespaces: Vec<VfsUsageStats>,
}

fn parse_bool_form_value(field_name: &str, value: &str) -> AppResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(ApiError::bad_request(format!(
            "Invalid boolean value for '{}'",
            field_name
        ))),
    }
}

fn parse_tags_form_value(value: &str) -> AppResult<Vec<String>> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(trimmed)
            .map_err(|e| ApiError::bad_request(format!("Invalid tags JSON array: {}", e)));
    }

    Ok(trimmed
        .split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(ToString::to_string)
        .collect())
}

fn parse_custom_metadata_form_value(value: &str) -> AppResult<HashMap<String, String>> {
    serde_json::from_str::<HashMap<String, String>>(value).map_err(|e| {
        ApiError::bad_request(format!(
            "custom_metadata must be a JSON object with string values: {}",
            e
        ))
    })
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

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn empty_usage_stats(namespace: String) -> VfsUsageStats {
    VfsUsageStats {
        namespace,
        file_count: 0,
        storage_used: 0,
        storage_quota: None,
        directory_count: 0,
        last_updated: now_unix_seconds(),
    }
}

async fn usage_stats_for_namespace(
    vfs_service: &dyn oxide_core::VirtualFileSystem,
    namespace: &str,
) -> AppResult<VfsUsageStats> {
    let namespace = namespace.to_string();
    vfs_service.get_usage_stats(&namespace).await.map_err(|e| {
        error!(
            "Failed to get VFS usage stats for namespace '{}': {:?}",
            namespace, e
        );
        match e {
            VfsError::InvalidPath { .. } => ApiError::bad_request("Invalid namespace".to_string()),
            VfsError::AccessDenied { .. } => {
                ApiError::not_found(format!("VFS namespace '{}'", namespace))
            }
            VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
            _ => ApiError::internal("Failed to retrieve usage statistics".to_string()),
        }
    })
}

fn summarize_usage(namespace: String, namespaces: Vec<VfsUsageStats>) -> VfsUsageSummaryResponse {
    let mut file_count = 0;
    let mut storage_used = 0;
    let mut directory_count = 0;
    let mut quota_sum = 0;
    let mut has_unlimited_quota = false;
    let mut last_updated = 0;

    for stats in &namespaces {
        file_count += stats.file_count;
        storage_used += stats.storage_used;
        directory_count += stats.directory_count;
        last_updated = last_updated.max(stats.last_updated);

        match stats.storage_quota {
            Some(quota) => quota_sum += quota,
            None => has_unlimited_quota = true,
        }
    }

    VfsUsageSummaryResponse {
        namespace,
        file_count,
        storage_used,
        storage_quota: if has_unlimited_quota {
            None
        } else {
            Some(quota_sum)
        },
        directory_count,
        last_updated: if last_updated == 0 {
            now_unix_seconds()
        } else {
            last_updated
        },
        namespaces,
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
    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection)
        .await
        .map_err(|e| {
            error!(
                "Failed to ensure VFS namespace exists for collection '{}': {:?}",
                collection, e
            );
            match e {
                VfsError::InvalidPath { .. } => {
                    ApiError::bad_request("Invalid collection name".to_string())
                }
                VfsError::FileAlreadyExists { .. } => {
                    ApiError::internal("Namespace creation conflict".to_string())
                }
                _ => ApiError::internal("Failed to initialize collection file storage".to_string()),
            }
        })?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut mime_type: Option<String> = None;
    let mut custom_path: Option<String> = None;
    let mut overwrite = false;
    let mut custom_metadata: Option<HashMap<String, String>> = None;
    let mut tags: Option<Vec<String>> = None;

    // Process multipart form data
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("Failed to read multipart data: {}", e)))?
    {
        let field_name = field.name().unwrap_or("");

        match field_name {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                mime_type = field.content_type().map(|s| s.to_string());

                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            ApiError::bad_request(format!("Failed to read file data: {}", e))
                        })?
                        .to_vec(),
                );
            }
            "path" => {
                custom_path = Some(field.text().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read path field: {}", e))
                })?);
            }
            "overwrite" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read overwrite field: {}", e))
                })?;
                overwrite = parse_bool_form_value("overwrite", &value)?;
            }
            "tags" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read tags field: {}", e))
                })?;
                tags = Some(parse_tags_form_value(&value)?);
            }
            "custom_metadata" | "metadata" => {
                let value = field.text().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read custom_metadata field: {}", e))
                })?;
                custom_metadata = Some(parse_custom_metadata_form_value(&value)?);
            }
            "collection" => {
                // Already have collection from path, but allow override
                let form_collection = field.text().await.map_err(|e| {
                    ApiError::bad_request(format!("Failed to read collection field: {}", e))
                })?;
                if form_collection != collection {
                    warn!(
                        "Collection mismatch: path={}, form={}",
                        collection, form_collection
                    );
                }
            }
            _ => {
                debug!("Ignoring unknown multipart field: {}", field_name);
            }
        }
    }

    // Validate required fields
    let file_data =
        file_data.ok_or_else(|| ApiError::bad_request("No file data provided".to_string()))?;

    let file_name =
        file_name.ok_or_else(|| ApiError::bad_request("No filename provided".to_string()))?;

    let mime_type = mime_type.unwrap_or_else(|| {
        // Try to guess MIME type from file extension
        mime_guess::from_path(&file_name)
            .first_or_octet_stream()
            .to_string()
    });

    // Create namespace for the collection
    let namespace = collection.clone();

    // Generate file path
    let file_path = custom_path.unwrap_or_else(|| format!("uploads/{}", file_name));

    // Prepare file write request
    let write_request = FileWriteRequest {
        path: file_path.clone(),
        content: file_data,
        mime_type: Some(mime_type.clone()),
        custom_metadata,
        tags,
        overwrite,
    };

    // Write file to VFS
    let file_metadata = vfs_service
        .write_file(&namespace, write_request)
        .await
        .map_err(|e| {
            error!("Failed to write file to VFS: {:?}", e);
            match e {
                VfsError::InvalidPath { .. } => {
                    ApiError::bad_request("Invalid file path".to_string())
                }
                VfsError::FileAlreadyExists { .. } => ApiError::conflict(
                    "File already exists at this path; set overwrite=true to replace it"
                        .to_string(),
                ),
                VfsError::QuotaExceeded { .. } => ApiError::PayloadTooLarge,
                VfsError::AccessDenied { .. } => {
                    ApiError::forbidden("Permission denied".to_string())
                }
                VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
                _ => ApiError::internal("File upload failed".to_string()),
            }
        })?;

    info!(
        "📁 File uploaded successfully: {} ({})",
        file_name, file_metadata.id
    );

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
    debug!(
        "📁 Download file request: {} from collection: {}",
        file_id, collection
    );

    // Get VFS service from app state
    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection)
        .await
        .map_err(|e| {
            error!(
                "Failed to ensure VFS namespace exists for collection '{}': {:?}",
                collection, e
            );
            ApiError::internal("Failed to access collection file storage".to_string())
        })?;

    // Create namespace for the collection
    let namespace = collection;

    // Read file from VFS
    let read_request = FileReadRequest {
        identifier: FileIdentifier::Id(file_id.clone()),
        include_content: true,
    };

    let file_response = vfs_service
        .read_file(&namespace, read_request)
        .await
        .map_err(|e| {
            error!("Failed to read file from VFS: {:?}", e);
            match e {
                VfsError::FileNotFound { .. } => ApiError::not_found("File not found".to_string()),
                VfsError::AccessDenied { .. } => {
                    ApiError::forbidden("Permission denied".to_string())
                }
                VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
                _ => ApiError::internal("File download failed".to_string()),
            }
        })?;

    // Build response with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            file_response.metadata.mime_type.clone(),
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", file_response.metadata.name),
        )
        .header(
            header::CONTENT_LENGTH,
            file_response.content.as_ref().map(|c| c.len()).unwrap_or(0),
        );

    // Add file content to response
    let body = file_response.content.unwrap_or_default();

    response
        .body(axum::body::Body::from(body))
        .map_err(|e| ApiError::internal(format!("Failed to create response: {}", e)))
}

/// Get file metadata without downloading content
pub async fn get_file_metadata(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path((collection, file_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    debug!(
        "📁 File metadata request: {} from collection: {}",
        file_id, collection
    );

    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    ensure_namespace_exists(vfs_service.as_ref(), &collection)
        .await
        .map_err(|e| {
            error!(
                "Failed to ensure VFS namespace exists for collection '{}': {:?}",
                collection, e
            );
            ApiError::internal("Failed to access collection file storage".to_string())
        })?;

    let namespace = collection;
    let read_request = FileReadRequest {
        identifier: FileIdentifier::Id(file_id),
        include_content: false,
    };

    let file_response = vfs_service
        .read_file(&namespace, read_request)
        .await
        .map_err(|e| {
            error!("Failed to read file metadata from VFS: {:?}", e);
            match e {
                VfsError::FileNotFound { .. } => ApiError::not_found("File not found".to_string()),
                VfsError::AccessDenied { .. } => {
                    ApiError::forbidden("Permission denied".to_string())
                }
                VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
                _ => ApiError::internal("File metadata lookup failed".to_string()),
            }
        })?;

    Ok(ApiResponse::success(file_response.metadata))
}

/// Move or rename a file in the VFS
pub async fn move_file(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path((collection, file_id)): Path<(String, String)>,
    Json(payload): Json<MoveFilePayload>,
) -> AppResult<impl IntoResponse> {
    debug!(
        "📁 Move file request: {} from collection: {}",
        file_id, collection
    );

    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    ensure_namespace_exists(vfs_service.as_ref(), &collection)
        .await
        .map_err(|e| {
            error!(
                "Failed to ensure VFS namespace exists for collection '{}': {:?}",
                collection, e
            );
            ApiError::internal("Failed to access collection file storage".to_string())
        })?;

    let move_request = FileMoveRequest {
        identifier: FileIdentifier::Id(file_id.clone()),
        new_path: payload.path,
        overwrite: payload.overwrite.unwrap_or(false),
    };

    let metadata = vfs_service
        .move_file(&collection, move_request)
        .await
        .map_err(|e| {
            error!("Failed to move file in VFS: {:?}", e);
            match e {
                VfsError::InvalidPath { .. } => {
                    ApiError::bad_request("Invalid destination file path".to_string())
                }
                VfsError::FileNotFound { .. } => ApiError::not_found("File not found".to_string()),
                VfsError::FileAlreadyExists { .. } => ApiError::conflict(
                    "File already exists at destination path; set overwrite=true to replace it"
                        .to_string(),
                ),
                VfsError::AccessDenied { .. } => {
                    ApiError::forbidden("Permission denied".to_string())
                }
                VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
                _ => ApiError::internal("File move failed".to_string()),
            }
        })?;

    info!(
        "📁 File moved successfully: {} -> {}",
        file_id, metadata.path
    );
    Ok(ApiResponse::success(metadata))
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
    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection)
        .await
        .map_err(|e| {
            error!(
                "Failed to ensure VFS namespace exists for collection '{}': {:?}",
                collection, e
            );
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
    let file_list = vfs_service
        .list_files(&namespace, list_request)
        .await
        .map_err(|e| {
            error!("Failed to list files from VFS: {:?}", e);
            match e {
                VfsError::AccessDenied { .. } => {
                    ApiError::forbidden("Permission denied".to_string())
                }
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
    debug!(
        "📁 Delete file request: {} from collection: {}",
        file_id, collection
    );

    // Get VFS service from app state
    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    // Ensure namespace exists for this collection
    ensure_namespace_exists(vfs_service.as_ref(), &collection)
        .await
        .map_err(|e| {
            error!(
                "Failed to ensure VFS namespace exists for collection '{}': {:?}",
                collection, e
            );
            ApiError::internal("Failed to access collection file storage".to_string())
        })?;

    // Create namespace for the collection
    let namespace = collection;

    // Delete file from VFS
    vfs_service
        .delete_file(&namespace, FileIdentifier::Id(file_id.clone()))
        .await
        .map_err(|e| {
            error!("Failed to delete file from VFS: {:?}", e);
            match e {
                VfsError::FileNotFound { .. } => ApiError::not_found("File not found".to_string()),
                VfsError::AccessDenied { .. } => {
                    ApiError::forbidden("Permission denied".to_string())
                }
                VfsError::IoError { .. } => ApiError::internal("Storage error".to_string()),
                _ => ApiError::internal("File deletion failed".to_string()),
            }
        })?;

    info!("📁 File deleted successfully: {}", file_id);
    Ok(ApiResponse::success(
        crate::responses::EmptyResponse::deleted(),
    ))
}

/// Get VFS usage statistics
pub async fn get_usage_stats(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Query(query): Query<VfsUsageQuery>,
) -> AppResult<impl IntoResponse> {
    debug!("📁 Get VFS usage statistics request");

    // Get VFS service from app state
    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    if let Some(namespace) = query.namespace {
        if namespace.trim().is_empty() {
            return Err(ApiError::bad_request(
                "Namespace cannot be empty".to_string(),
            ));
        }

        let stats = usage_stats_for_namespace(vfs_service.as_ref(), &namespace).await?;
        return Ok(ApiResponse::success(summarize_usage(
            namespace,
            vec![stats],
        )));
    }

    let include_default = query.include_default.unwrap_or(true);
    let mut namespaces = BTreeSet::new();
    if include_default {
        namespaces.insert("default".to_string());
    }

    for collection in state.db.list_collections().await? {
        namespaces.insert(collection.name);
    }

    let mut usage_by_namespace = Vec::new();
    for namespace in namespaces {
        match vfs_service.get_usage_stats(&namespace).await {
            Ok(stats) => usage_by_namespace.push(stats),
            Err(VfsError::AccessDenied { .. }) => {
                usage_by_namespace.push(empty_usage_stats(namespace))
            }
            Err(e) => {
                error!(
                    "Failed to get VFS usage stats for namespace '{}': {:?}",
                    namespace, e
                );
                return Err(ApiError::internal(
                    "Failed to retrieve usage statistics".to_string(),
                ));
            }
        }
    }

    Ok(ApiResponse::success(summarize_usage(
        "all".to_string(),
        usage_by_namespace,
    )))
}

/// Get VFS usage statistics for a collection namespace
pub async fn get_collection_usage_stats(
    State(state): State<AppState>,
    _auth: AuthenticatedUser,
    Path(collection): Path<String>,
) -> AppResult<impl IntoResponse> {
    debug!(
        "📁 Get VFS usage statistics request for collection: {}",
        collection
    );

    let vfs_service = state
        .vfs_service
        .as_ref()
        .ok_or_else(|| ApiError::internal("VFS service not available".to_string()))?;

    ensure_namespace_exists(vfs_service.as_ref(), &collection)
        .await
        .map_err(|e| {
            error!(
                "Failed to ensure VFS namespace exists for collection '{}': {:?}",
                collection, e
            );
            match e {
                VfsError::InvalidPath { .. } => {
                    ApiError::bad_request("Invalid collection name".to_string())
                }
                _ => ApiError::internal("Failed to access collection file storage".to_string()),
            }
        })?;

    let usage_stats = usage_stats_for_namespace(vfs_service.as_ref(), &collection).await?;
    Ok(ApiResponse::success(usage_stats))
}
