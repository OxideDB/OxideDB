//! Backup and export handlers
//!
//! These endpoints provide a portable JSON snapshot of OxideDB collections
//! using only the database abstraction. That keeps the feature available across
//! SQLite and future database backends.

use axum::{
    extract::{Query, State},
    Json,
};
use oxide_core::{AppError, CollectionSchema, CollectionType};
use oxide_db::{db::ListParams, Db, Record};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};
use tracing::{debug, info};

use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};

const BACKUP_FORMAT_VERSION: u32 = 1;
const EXPORT_PAGE_SIZE: usize = 500;

#[derive(Debug, Clone, Deserialize)]
pub struct BackupQuery {
    #[serde(default)]
    pub include_system: bool,
    pub collections: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupCollectionSummary {
    pub name: String,
    pub collection_type: CollectionType,
    pub schema_version: u32,
    pub record_count: usize,
    pub size_kb: f64,
    pub included: bool,
    pub excluded_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupManifestResponse {
    pub generated_at: String,
    pub total_collections: usize,
    pub included_collections: usize,
    pub total_records: usize,
    pub total_size_kb: f64,
    pub include_system: bool,
    pub collections: Vec<BackupCollectionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupCollectionExport {
    pub schema: CollectionSchema,
    pub records: Vec<Record>,
    pub record_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupExportResponse {
    pub format_version: u32,
    pub generated_at: String,
    pub include_system: bool,
    pub total_collections: usize,
    pub total_records: usize,
    pub collections: Vec<BackupCollectionExport>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackupRestoreRequest {
    pub snapshot: BackupExportResponse,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub include_system: bool,
    #[serde(default)]
    pub replace_existing: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupRestoreCollectionResult {
    pub name: String,
    pub collection_type: CollectionType,
    pub status: String,
    pub source_records: usize,
    pub records_created: usize,
    pub records_updated: usize,
    pub records_skipped: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupRestoreResponse {
    pub dry_run: bool,
    pub generated_at: String,
    pub source_generated_at: String,
    pub include_system: bool,
    pub replace_existing: bool,
    pub total_collections: usize,
    pub total_records: usize,
    pub created_collections: usize,
    pub replaced_collections: usize,
    pub skipped_collections: usize,
    pub created_records: usize,
    pub updated_records: usize,
    pub skipped_records: usize,
    pub collections: Vec<BackupRestoreCollectionResult>,
    pub warnings: Vec<String>,
}

pub struct BackupHandlers;

impl BackupHandlers {
    pub async fn manifest(
        db: Arc<dyn Db>,
        query: BackupQuery,
    ) -> Result<BackupManifestResponse, ApiError> {
        debug!("Building backup manifest");

        let selected_collections = parse_collection_filter(&query.collections);
        let schemas = db.list_collections().await?;
        let mut collections = Vec::with_capacity(schemas.len());

        for schema in schemas {
            let included =
                should_include_collection(&schema, query.include_system, &selected_collections);
            let excluded_reason =
                excluded_reason(&schema, query.include_system, &selected_collections);
            let record_count = db.count_records(&schema.name).await?;
            let size_kb = db.get_collection_size_kb(&schema.name).await?;

            collections.push(BackupCollectionSummary {
                name: schema.name,
                collection_type: schema.collection_type,
                schema_version: schema.version,
                record_count,
                size_kb,
                included,
                excluded_reason,
            });
        }

        collections.sort_by(|a, b| a.name.cmp(&b.name));

        let included_collections = collections.iter().filter(|entry| entry.included).count();
        let total_records = collections
            .iter()
            .filter(|entry| entry.included)
            .map(|entry| entry.record_count)
            .sum();
        let total_size_kb = collections
            .iter()
            .filter(|entry| entry.included)
            .map(|entry| entry.size_kb)
            .sum();

        Ok(BackupManifestResponse {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_collections: collections.len(),
            included_collections,
            total_records,
            total_size_kb,
            include_system: query.include_system,
            collections,
        })
    }

    pub async fn export(
        db: Arc<dyn Db>,
        query: BackupQuery,
    ) -> Result<BackupExportResponse, ApiError> {
        debug!("Exporting backup snapshot");

        let selected_collections = parse_collection_filter(&query.collections);
        let mut schemas = db.list_collections().await?;
        schemas.sort_by(|a, b| a.name.cmp(&b.name));

        let mut collections = Vec::new();
        let mut total_records = 0;

        for schema in schemas {
            if !should_include_collection(&schema, query.include_system, &selected_collections) {
                continue;
            }

            let records = export_collection_records(Arc::clone(&db), &schema.name).await?;
            total_records += records.len();
            collections.push(BackupCollectionExport {
                schema,
                record_count: records.len(),
                records,
            });
        }

        info!(
            "Exported backup snapshot with {} collections and {} records",
            collections.len(),
            total_records
        );

        Ok(BackupExportResponse {
            format_version: BACKUP_FORMAT_VERSION,
            generated_at: chrono::Utc::now().to_rfc3339(),
            include_system: query.include_system,
            total_collections: collections.len(),
            total_records,
            collections,
        })
    }

    pub async fn restore(
        db: Arc<dyn Db>,
        request: BackupRestoreRequest,
    ) -> Result<BackupRestoreResponse, ApiError> {
        debug!("Restoring backup snapshot");

        if request.snapshot.format_version != BACKUP_FORMAT_VERSION {
            return Err(ApiError::bad_request(format!(
                "Unsupported backup format version {}",
                request.snapshot.format_version
            )));
        }
        validate_restore_snapshot(&request.snapshot, request.include_system)?;

        let source_generated_at = request.snapshot.generated_at.clone();
        let mut response = BackupRestoreResponse {
            dry_run: request.dry_run,
            generated_at: chrono::Utc::now().to_rfc3339(),
            source_generated_at,
            include_system: request.include_system,
            replace_existing: request.replace_existing,
            total_collections: request.snapshot.collections.len(),
            total_records: request
                .snapshot
                .collections
                .iter()
                .map(|collection| collection.records.len())
                .sum(),
            created_collections: 0,
            replaced_collections: 0,
            skipped_collections: 0,
            created_records: 0,
            updated_records: 0,
            skipped_records: 0,
            collections: Vec::new(),
            warnings: Vec::new(),
        };

        for collection_export in request.snapshot.collections {
            let schema = collection_export.schema;
            let collection_name = schema.name.clone();
            let source_records = collection_export.records.len();
            let mut collection_warnings = Vec::new();

            if collection_export.record_count != source_records {
                collection_warnings.push(format!(
                    "Snapshot record_count was {}, but {} records were present",
                    collection_export.record_count, source_records
                ));
            }

            if is_system_collection(&collection_name) && !request.include_system {
                response.skipped_collections += 1;
                response.skipped_records += source_records;
                collection_warnings.push(
                    "System collection skipped; enable include_system to restore it".to_string(),
                );
                response.collections.push(BackupRestoreCollectionResult {
                    name: collection_name,
                    collection_type: schema.collection_type,
                    status: "skipped_system".to_string(),
                    source_records,
                    records_created: 0,
                    records_updated: 0,
                    records_skipped: source_records,
                    warnings: collection_warnings,
                });
                continue;
            }

            let exists = db.collection_exists(&collection_name).await?;
            let mut status = if exists {
                "merged".to_string()
            } else {
                "created".to_string()
            };
            let mut records_created = 0;
            let records_updated = 0;
            let mut records_skipped = 0;
            let skip_existing_records = exists && !request.replace_existing;

            if exists && request.replace_existing {
                status = if request.dry_run {
                    "would_replace".to_string()
                } else {
                    db.delete_collection(&collection_name).await?;
                    db.create_collection(schema.clone()).await?;
                    "replaced".to_string()
                };
                if request.dry_run {
                    records_created = source_records;
                    response.replaced_collections += 1;
                    response.created_records += records_created;
                    response.collections.push(BackupRestoreCollectionResult {
                        name: collection_name,
                        collection_type: schema.collection_type,
                        status,
                        source_records,
                        records_created,
                        records_updated,
                        records_skipped,
                        warnings: collection_warnings,
                    });
                    continue;
                }
                response.replaced_collections += 1;
            } else if exists {
                let current_schema = db.get_collection_schema(&collection_name).await?;
                if !schemas_are_restore_compatible(&current_schema, &schema) {
                    response.skipped_collections += 1;
                    response.skipped_records += source_records;
                    collection_warnings.push(
                        "Existing collection schema differs from the snapshot; enable replace_existing to restore it"
                            .to_string(),
                    );
                    response.collections.push(BackupRestoreCollectionResult {
                        name: collection_name,
                        collection_type: schema.collection_type,
                        status: "skipped_schema_mismatch".to_string(),
                        source_records,
                        records_created: 0,
                        records_updated: 0,
                        records_skipped: source_records,
                        warnings: collection_warnings,
                    });
                    continue;
                }

                if current_schema.indexes != schema.indexes {
                    collection_warnings.push(
                        "Existing collection has different index definitions; records can be restored, but indexes were not changed"
                            .to_string(),
                    );
                }
            } else if request.dry_run {
                response.created_collections += 1;
                records_created = source_records;
                response.created_records += records_created;
                response.collections.push(BackupRestoreCollectionResult {
                    name: collection_name,
                    collection_type: schema.collection_type,
                    status: "would_create".to_string(),
                    source_records,
                    records_created,
                    records_updated,
                    records_skipped,
                    warnings: collection_warnings,
                });
                continue;
            } else {
                db.create_collection(schema.clone()).await?;
                response.created_collections += 1;
            }

            if !request.dry_run {
                for mut record in collection_export.records {
                    record.collection = collection_name.clone();
                    if skip_existing_records
                        && record_exists(&db, &collection_name, &record.id).await?
                    {
                        records_skipped += 1;
                        continue;
                    }

                    db.upsert_record_with_metadata(&collection_name, record)
                        .await?;
                    records_created += 1;
                }
            } else {
                for record in collection_export.records {
                    match record_exists(&db, &collection_name, &record.id).await? {
                        true => records_skipped += 1,
                        false => records_created += 1,
                    }
                }
            }

            response.created_records += records_created;
            response.updated_records += records_updated;
            response.skipped_records += records_skipped;

            response.collections.push(BackupRestoreCollectionResult {
                name: collection_name,
                collection_type: schema.collection_type,
                status,
                source_records,
                records_created,
                records_updated,
                records_skipped,
                warnings: collection_warnings,
            });
        }

        for collection in &response.collections {
            response.warnings.extend(
                collection
                    .warnings
                    .iter()
                    .map(|warning| format!("{}: {}", collection.name, warning)),
            );
        }

        info!(
            "{} backup restore with {} created records and {} skipped records",
            if response.dry_run {
                "Previewed"
            } else {
                "Completed"
            },
            response.created_records,
            response.skipped_records
        );

        Ok(response)
    }
}

pub async fn get_backup_manifest(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<BackupQuery>,
) -> Result<Json<ApiResponse<BackupManifestResponse>>, ApiError> {
    ensure_superuser(&authenticated_user, "preview backups")?;

    let response = BackupHandlers::manifest(state.db, query).await?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn export_backup(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<BackupQuery>,
) -> Result<Json<ApiResponse<BackupExportResponse>>, ApiError> {
    ensure_superuser(&authenticated_user, "export backups")?;

    let response = BackupHandlers::export(state.db, query).await?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn restore_backup(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(request): Json<BackupRestoreRequest>,
) -> Result<Json<ApiResponse<BackupRestoreResponse>>, ApiError> {
    ensure_superuser(&authenticated_user, "restore backups")?;

    let response = BackupHandlers::restore(state.db, request).await?;
    Ok(Json(ApiResponse::success(response)))
}

async fn export_collection_records(
    db: Arc<dyn Db>,
    collection: &str,
) -> Result<Vec<Record>, ApiError> {
    let mut records = Vec::new();
    let mut offset = 0;

    loop {
        let batch = db
            .list_records(
                collection,
                ListParams {
                    limit: Some(EXPORT_PAGE_SIZE),
                    offset: Some(offset),
                    ..Default::default()
                },
            )
            .await?;

        let batch_len = batch.len();
        records.extend(batch);

        if batch_len < EXPORT_PAGE_SIZE {
            break;
        }

        offset += EXPORT_PAGE_SIZE;
    }

    Ok(records)
}

fn ensure_superuser(authenticated_user: &AuthenticatedUser, action: &str) -> Result<(), ApiError> {
    if authenticated_user.is_superuser() {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!(
            "Superuser privileges are required to {}",
            action
        )))
    }
}

async fn record_exists(
    db: &Arc<dyn Db>,
    collection: &str,
    record_id: &str,
) -> Result<bool, ApiError> {
    let record_id = record_id.to_string();
    match db.read_record(collection, &record_id).await {
        Ok(_) => Ok(true),
        Err(AppError::NotFound { .. }) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn validate_restore_snapshot(
    snapshot: &BackupExportResponse,
    include_system: bool,
) -> Result<(), ApiError> {
    for collection in &snapshot.collections {
        if is_system_collection(&collection.schema.name) && !include_system {
            continue;
        }

        for record in &collection.records {
            if let Err(validation_error) = collection.schema.validate_data(&record.data) {
                return Err(ApiError::bad_request(format!(
                    "Record '{}' in collection '{}' failed validation: {}",
                    record.id, collection.schema.name, validation_error
                )));
            }
        }
    }

    Ok(())
}

fn schemas_are_restore_compatible(current: &CollectionSchema, incoming: &CollectionSchema) -> bool {
    if current.collection_type != incoming.collection_type {
        return false;
    }

    match (
        serde_json::to_value(&current.fields),
        serde_json::to_value(&incoming.fields),
    ) {
        (Ok(current_fields), Ok(incoming_fields)) => current_fields == incoming_fields,
        _ => false,
    }
}

fn parse_collection_filter(collections: &Option<String>) -> HashSet<String> {
    collections
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn should_include_collection(
    schema: &CollectionSchema,
    include_system: bool,
    selected_collections: &HashSet<String>,
) -> bool {
    if !include_system && is_system_collection(&schema.name) {
        return false;
    }

    selected_collections.is_empty() || selected_collections.contains(&schema.name)
}

fn excluded_reason(
    schema: &CollectionSchema,
    include_system: bool,
    selected_collections: &HashSet<String>,
) -> Option<String> {
    if !include_system && is_system_collection(&schema.name) {
        Some("system collection".to_string())
    } else if !selected_collections.is_empty() && !selected_collections.contains(&schema.name) {
        Some("not selected".to_string())
    } else {
        None
    }
}

fn is_system_collection(collection: &str) -> bool {
    collection.starts_with('_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::{
        auth::{AuthService, AuthServiceConfig},
        CollectionType, FieldDefinition, FieldType, InMemoryEventBus,
    };
    use oxide_db::SqliteDb;

    fn test_db() -> Arc<dyn Db> {
        let event_bus = Arc::new(InMemoryEventBus::new());
        let auth_service = Arc::new(AuthService::new(AuthServiceConfig::new(
            "restore-test-secret".to_string(),
        )));

        Arc::new(SqliteDb::new(":memory:", event_bus, auth_service).unwrap())
    }

    #[tokio::test]
    async fn restore_recreates_collection_and_preserves_record_metadata() {
        let db = test_db();
        db.initialize().await.unwrap();

        let mut schema = CollectionSchema::new("articles".to_string(), CollectionType::Base);
        schema.add_field("title".to_string(), FieldDefinition::new(FieldType::Text));
        db.create_collection(schema).await.unwrap();

        let original = db
            .create_record("articles", serde_json::json!({ "title": "Restorable" }))
            .await
            .unwrap();

        let snapshot = BackupHandlers::export(
            Arc::clone(&db),
            BackupQuery {
                include_system: false,
                collections: Some("articles".to_string()),
            },
        )
        .await
        .unwrap();

        db.delete_collection("articles").await.unwrap();

        let preview = BackupHandlers::restore(
            Arc::clone(&db),
            BackupRestoreRequest {
                snapshot: snapshot.clone(),
                dry_run: true,
                include_system: false,
                replace_existing: false,
            },
        )
        .await
        .unwrap();

        assert!(preview.dry_run);
        assert_eq!(preview.created_collections, 1);
        assert_eq!(preview.created_records, 1);

        let restored_summary = BackupHandlers::restore(
            Arc::clone(&db),
            BackupRestoreRequest {
                snapshot,
                dry_run: false,
                include_system: false,
                replace_existing: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(restored_summary.created_collections, 1);
        assert_eq!(restored_summary.created_records, 1);

        let restored = db.read_record("articles", &original.id).await.unwrap();
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.created_at, original.created_at);
        assert_eq!(restored.updated_at, original.updated_at);
        assert_eq!(restored.data, original.data);
    }
}
