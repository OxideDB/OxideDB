//! Backup and export handlers
//!
//! These endpoints provide a portable JSON snapshot of OxideDB collections
//! using only the database abstraction. That keeps the feature available across
//! SQLite and future database backends.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
    BoxError, Json,
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use bytes::Bytes;
use futures_util::TryStreamExt;
use oxide_core::{
    AppError, CollectionSchema, CollectionType, FileIdentifier, FileListRequest, FileMetadata,
    FileReadRequest, FileWriteRequest, VfsError, VfsNamespaceConfig, VirtualFileSystem,
};
use oxide_db::{db::ListParams, Db, Record};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::Arc};
use tracing::{debug, info, warn};

use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};

const BACKUP_FORMAT_VERSION: u32 = 1;
const EXPORT_PAGE_SIZE: usize = 500;
const DEFAULT_EXPORT_RECORD_LIMIT: usize = 100_000;
const EXPORT_RECORD_LIMIT_ENV: &str = "OXIDEDB_BACKUP_MAX_EXPORT_RECORDS";
const STREAM_EXPORT_FILE_PREFIX: &str = "oxidedb-backup";

fn default_restore_include_vfs() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackupQuery {
    #[serde(default)]
    pub include_system: bool,
    #[serde(default)]
    pub include_vfs: bool,
    pub collections: Option<String>,
    pub max_records: Option<usize>,
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
    pub include_vfs: bool,
    pub total_vfs_files: usize,
    pub total_vfs_size_bytes: u64,
    pub collections: Vec<BackupCollectionSummary>,
    pub vfs_namespaces: Vec<BackupVfsNamespaceSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupCollectionExport {
    pub schema: CollectionSchema,
    pub records: Vec<Record>,
    pub record_count: usize,
}

/// VFS namespace summary included in backup manifests.
#[derive(Debug, Clone, Serialize)]
pub struct BackupVfsNamespaceSummary {
    /// Namespace included in the snapshot.
    pub namespace: String,
    /// Number of VFS files in the namespace.
    pub file_count: usize,
    /// Total logical file size in bytes.
    pub total_size_bytes: u64,
}

/// A single VFS file embedded in a JSON backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVfsFileExport {
    /// Original VFS metadata for the file.
    pub metadata: FileMetadata,
    /// Encoding used for the embedded file content.
    pub content_encoding: String,
    /// File content encoded as base64.
    pub content_base64: String,
}

/// VFS namespace data embedded in a JSON backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupVfsNamespaceExport {
    /// Namespace configuration to recreate on restore.
    pub config: VfsNamespaceConfig,
    /// Files included in the namespace.
    pub files: Vec<BackupVfsFileExport>,
    /// Number of files reported when the namespace was exported.
    pub file_count: usize,
    /// Total logical file size in bytes.
    pub total_size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupExportResponse {
    pub format_version: u32,
    pub generated_at: String,
    pub include_system: bool,
    #[serde(default)]
    pub include_vfs: bool,
    pub total_collections: usize,
    pub total_records: usize,
    #[serde(default)]
    pub total_vfs_files: usize,
    #[serde(default)]
    pub total_vfs_size_bytes: u64,
    pub collections: Vec<BackupCollectionExport>,
    #[serde(default)]
    pub vfs_namespaces: Vec<BackupVfsNamespaceExport>,
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
    #[serde(default = "default_restore_include_vfs")]
    pub include_vfs: bool,
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

/// Restore result for one VFS namespace.
#[derive(Debug, Clone, Serialize)]
pub struct BackupVfsRestoreNamespaceResult {
    /// Namespace restored from the snapshot.
    pub namespace: String,
    /// Restore status for the namespace.
    pub status: String,
    /// Files present in the source snapshot namespace.
    pub source_files: usize,
    /// Files written during restore.
    pub files_created: usize,
    /// Existing files skipped during merge restore.
    pub files_skipped: usize,
    /// Warnings emitted for this namespace.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BackupRestoreResponse {
    pub dry_run: bool,
    pub generated_at: String,
    pub source_generated_at: String,
    pub include_system: bool,
    pub include_vfs: bool,
    pub replace_existing: bool,
    pub total_collections: usize,
    pub total_records: usize,
    pub total_vfs_namespaces: usize,
    pub total_vfs_files: usize,
    pub created_collections: usize,
    pub replaced_collections: usize,
    pub skipped_collections: usize,
    pub created_records: usize,
    pub updated_records: usize,
    pub skipped_records: usize,
    pub created_vfs_namespaces: usize,
    pub replaced_vfs_namespaces: usize,
    pub skipped_vfs_namespaces: usize,
    pub created_vfs_files: usize,
    pub skipped_vfs_files: usize,
    pub collections: Vec<BackupRestoreCollectionResult>,
    pub vfs_namespaces: Vec<BackupVfsRestoreNamespaceResult>,
    pub warnings: Vec<String>,
}

struct BackupExportPlan {
    generated_at: String,
    include_system: bool,
    include_vfs: bool,
    collections: Vec<(CollectionSchema, usize)>,
    total_records: usize,
    vfs_namespaces: Vec<BackupVfsNamespacePlan>,
    total_vfs_files: usize,
    total_vfs_size_bytes: u64,
}

struct BackupVfsNamespacePlan {
    config: VfsNamespaceConfig,
    file_count: usize,
    total_size_bytes: u64,
}

enum RestoreJournalEntry {
    DeleteCreatedCollection {
        name: String,
    },
    RestoreCollection {
        schema: CollectionSchema,
        records: Vec<Record>,
    },
    DeleteCreatedRecord {
        collection: String,
        record_id: String,
    },
    DeleteCreatedVfsNamespace {
        namespace: String,
    },
    RestoreVfsNamespace {
        config: VfsNamespaceConfig,
        files: Vec<BackupVfsFileExport>,
    },
    DeleteCreatedVfsFile {
        namespace: String,
        file_id: String,
    },
}

struct RestoreJournal {
    db: Arc<dyn Db>,
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    entries: Vec<RestoreJournalEntry>,
}

impl RestoreJournal {
    fn new(db: Arc<dyn Db>, vfs_service: Option<Arc<dyn VirtualFileSystem>>) -> Self {
        Self {
            db,
            vfs_service,
            entries: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn commit(&mut self) {
        self.entries.clear();
    }

    fn delete_created_collection(&mut self, name: String) {
        self.entries
            .push(RestoreJournalEntry::DeleteCreatedCollection { name });
    }

    fn restore_collection(&mut self, schema: CollectionSchema, records: Vec<Record>) {
        self.entries
            .push(RestoreJournalEntry::RestoreCollection { schema, records });
    }

    fn delete_created_record(&mut self, collection: String, record_id: String) {
        self.entries.push(RestoreJournalEntry::DeleteCreatedRecord {
            collection,
            record_id,
        });
    }

    fn delete_created_vfs_namespace(&mut self, namespace: String) {
        self.entries
            .push(RestoreJournalEntry::DeleteCreatedVfsNamespace { namespace });
    }

    fn restore_vfs_namespace(
        &mut self,
        config: VfsNamespaceConfig,
        files: Vec<BackupVfsFileExport>,
    ) {
        self.entries
            .push(RestoreJournalEntry::RestoreVfsNamespace { config, files });
    }

    fn delete_created_vfs_file(&mut self, namespace: String, file_id: String) {
        self.entries
            .push(RestoreJournalEntry::DeleteCreatedVfsFile { namespace, file_id });
    }

    async fn rollback(&mut self) -> Vec<String> {
        let mut errors = Vec::new();

        while let Some(entry) = self.entries.pop() {
            if let Err(error) = self.rollback_entry(entry).await {
                let message = error.to_string();
                warn!("Backup restore rollback step failed: {}", message);
                errors.push(message);
            }
        }

        errors
    }

    async fn rollback_entry(&self, entry: RestoreJournalEntry) -> Result<(), ApiError> {
        match entry {
            RestoreJournalEntry::DeleteCreatedCollection { name } => {
                delete_collection_if_exists(Arc::clone(&self.db), &name).await
            }
            RestoreJournalEntry::RestoreCollection { schema, records } => {
                restore_collection_state(Arc::clone(&self.db), schema, records).await
            }
            RestoreJournalEntry::DeleteCreatedRecord {
                collection,
                record_id,
            } => delete_record_if_exists(Arc::clone(&self.db), &collection, &record_id).await,
            RestoreJournalEntry::DeleteCreatedVfsNamespace { namespace } => {
                let vfs = require_vfs_service(self.vfs_service.clone())?;
                delete_vfs_namespace_if_exists(vfs, &namespace).await
            }
            RestoreJournalEntry::RestoreVfsNamespace { config, files } => {
                let vfs = require_vfs_service(self.vfs_service.clone())?;
                restore_vfs_namespace_state(vfs, config, files).await
            }
            RestoreJournalEntry::DeleteCreatedVfsFile { namespace, file_id } => {
                let vfs = require_vfs_service(self.vfs_service.clone())?;
                delete_vfs_file_if_exists(vfs, &namespace, &file_id).await
            }
        }
    }
}

pub struct BackupHandlers;

impl BackupHandlers {
    pub async fn manifest(
        db: Arc<dyn Db>,
        vfs_service: Option<Arc<dyn VirtualFileSystem>>,
        query: BackupQuery,
    ) -> Result<BackupManifestResponse, ApiError> {
        debug!("Building backup manifest");

        let selected_collections = parse_collection_filter(&query.collections);
        let schemas = db.list_collections().await?;
        let mut collections = Vec::with_capacity(schemas.len());
        let mut included_collection_names = Vec::new();

        for schema in schemas {
            let included =
                should_include_collection(&schema, query.include_system, &selected_collections);
            let excluded_reason =
                excluded_reason(&schema, query.include_system, &selected_collections);
            let record_count = db.count_records(&schema.name).await?;
            let size_kb = db.get_collection_size_kb(&schema.name).await?;

            if included {
                included_collection_names.push(schema.name.clone());
            }

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
        let vfs_namespaces = if query.include_vfs {
            build_vfs_manifest_summaries(vfs_service, &included_collection_names).await?
        } else {
            Vec::new()
        };
        let total_vfs_files = vfs_namespaces
            .iter()
            .map(|namespace| namespace.file_count)
            .sum();
        let total_vfs_size_bytes = vfs_namespaces
            .iter()
            .map(|namespace| namespace.total_size_bytes)
            .sum();

        Ok(BackupManifestResponse {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_collections: collections.len(),
            included_collections,
            total_records,
            total_size_kb,
            include_system: query.include_system,
            include_vfs: query.include_vfs,
            total_vfs_files,
            total_vfs_size_bytes,
            collections,
            vfs_namespaces,
        })
    }

    pub async fn export(
        db: Arc<dyn Db>,
        vfs_service: Option<Arc<dyn VirtualFileSystem>>,
        query: BackupQuery,
    ) -> Result<BackupExportResponse, ApiError> {
        debug!("Exporting backup snapshot");

        let plan = build_backup_export_plan(Arc::clone(&db), vfs_service.clone(), query).await?;

        let mut collections = Vec::with_capacity(plan.collections.len());
        let mut total_records = 0;

        for (schema, record_count) in plan.collections {
            let records =
                export_collection_records(Arc::clone(&db), &schema.name, record_count).await?;
            total_records += records.len();
            collections.push(BackupCollectionExport {
                schema,
                record_count: records.len(),
                records,
            });
        }
        let vfs_namespaces = export_vfs_namespaces(vfs_service, &plan.vfs_namespaces).await?;

        info!(
            "Exported backup snapshot with {} collections, {} records, {} VFS namespaces, and {} VFS files",
            collections.len(),
            total_records,
            vfs_namespaces.len(),
            plan.total_vfs_files
        );

        Ok(BackupExportResponse {
            format_version: BACKUP_FORMAT_VERSION,
            generated_at: plan.generated_at,
            include_system: plan.include_system,
            include_vfs: plan.include_vfs,
            total_collections: collections.len(),
            total_records,
            total_vfs_files: plan.total_vfs_files,
            total_vfs_size_bytes: plan.total_vfs_size_bytes,
            collections,
            vfs_namespaces,
        })
    }

    pub async fn restore(
        db: Arc<dyn Db>,
        vfs_service: Option<Arc<dyn VirtualFileSystem>>,
        request: BackupRestoreRequest,
    ) -> Result<BackupRestoreResponse, ApiError> {
        debug!("Restoring backup snapshot");

        let mut journal = RestoreJournal::new(Arc::clone(&db), vfs_service.clone());
        let result = restore_backup_snapshot(db, vfs_service, request, &mut journal).await;

        match result {
            Ok(response) => {
                journal.commit();
                Ok(response)
            }
            Err(error) => {
                if journal.is_empty() {
                    return Err(error);
                }

                warn!("Backup restore failed; rolling back journaled changes");
                let rollback_errors = journal.rollback().await;
                Err(restore_error_with_rollback(error, rollback_errors))
            }
        }
    }
}

async fn restore_backup_snapshot(
    db: Arc<dyn Db>,
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    request: BackupRestoreRequest,
    journal: &mut RestoreJournal,
) -> Result<BackupRestoreResponse, ApiError> {
    let snapshot = request.snapshot;
    if snapshot.format_version != BACKUP_FORMAT_VERSION {
        return Err(ApiError::bad_request(format!(
            "Unsupported backup format version {}",
            snapshot.format_version
        )));
    }
    validate_restore_snapshot(&snapshot, request.include_system)?;

    let source_generated_at = snapshot.generated_at.clone();
    let total_collections = snapshot.collections.len();
    let total_records = snapshot
        .collections
        .iter()
        .map(|collection| collection.records.len())
        .sum();
    let total_vfs_namespaces = snapshot.vfs_namespaces.len();
    let total_vfs_files = snapshot
        .vfs_namespaces
        .iter()
        .map(|namespace| namespace.files.len())
        .sum();
    let BackupExportResponse {
        collections: snapshot_collections,
        vfs_namespaces: snapshot_vfs_namespaces,
        ..
    } = snapshot;
    let mut response = BackupRestoreResponse {
        dry_run: request.dry_run,
        generated_at: chrono::Utc::now().to_rfc3339(),
        source_generated_at,
        include_system: request.include_system,
        include_vfs: request.include_vfs,
        replace_existing: request.replace_existing,
        total_collections,
        total_records,
        total_vfs_namespaces,
        total_vfs_files,
        created_collections: 0,
        replaced_collections: 0,
        skipped_collections: 0,
        created_records: 0,
        updated_records: 0,
        skipped_records: 0,
        created_vfs_namespaces: 0,
        replaced_vfs_namespaces: 0,
        skipped_vfs_namespaces: 0,
        created_vfs_files: 0,
        skipped_vfs_files: 0,
        collections: Vec::new(),
        vfs_namespaces: Vec::new(),
        warnings: Vec::new(),
    };

    for collection_export in snapshot_collections {
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
            collection_warnings
                .push("System collection skipped; enable include_system to restore it".to_string());
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
        let collection_level_journaled = (request.replace_existing || !exists) && !request.dry_run;

        if exists && request.replace_existing {
            status = if request.dry_run {
                "would_replace".to_string()
            } else {
                let (previous_schema, previous_records) =
                    snapshot_collection_for_restore(Arc::clone(&db), &collection_name).await?;
                db.delete_collection(&collection_name).await?;
                journal.restore_collection(previous_schema, previous_records);
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
            journal.delete_created_collection(collection_name.clone());
            response.created_collections += 1;
        }

        if !request.dry_run {
            for mut record in collection_export.records {
                record.collection = collection_name.clone();
                let record_id = record.id.clone();
                if skip_existing_records && record_exists(&db, &collection_name, &record.id).await?
                {
                    records_skipped += 1;
                    continue;
                }

                db.upsert_record_with_metadata(&collection_name, record)
                    .await?;
                if !collection_level_journaled {
                    journal.delete_created_record(collection_name.clone(), record_id);
                }
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

    restore_vfs_namespaces(
        vfs_service,
        snapshot_vfs_namespaces,
        request.include_vfs,
        request.include_system,
        request.replace_existing,
        request.dry_run,
        journal,
        &mut response,
    )
    .await?;

    for collection in &response.collections {
        response.warnings.extend(
            collection
                .warnings
                .iter()
                .map(|warning| format!("{}: {}", collection.name, warning)),
        );
    }
    for namespace in &response.vfs_namespaces {
        response.warnings.extend(
            namespace
                .warnings
                .iter()
                .map(|warning| format!("{}: {}", namespace.namespace, warning)),
        );
    }

    info!(
        "{} backup restore with {} created records, {} skipped records, and {} restored VFS files",
        if response.dry_run {
            "Previewed"
        } else {
            "Completed"
        },
        response.created_records,
        response.skipped_records,
        response.created_vfs_files
    );

    Ok(response)
}

pub async fn get_backup_manifest(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<BackupQuery>,
) -> Result<Json<ApiResponse<BackupManifestResponse>>, ApiError> {
    ensure_superuser(&authenticated_user, "preview backups")?;

    let response = BackupHandlers::manifest(state.db, state.vfs_service, query).await?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn export_backup(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<BackupQuery>,
) -> Result<Json<ApiResponse<BackupExportResponse>>, ApiError> {
    ensure_superuser(&authenticated_user, "export backups")?;

    let response = BackupHandlers::export(state.db, state.vfs_service, query).await?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn export_backup_stream(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Query(query): Query<BackupQuery>,
) -> Result<Response<Body>, ApiError> {
    ensure_superuser(&authenticated_user, "stream export backups")?;

    let plan =
        build_backup_export_plan(Arc::clone(&state.db), state.vfs_service.clone(), query).await?;
    let filename = backup_stream_filename(&plan.generated_at);
    let body = stream_backup_json_body(state.db, state.vfs_service, plan);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
            .map_err(|e| ApiError::internal(format!("Invalid backup filename header: {}", e)))?,
    );

    Response::builder()
        .status(StatusCode::OK)
        .body(body)
        .map(|mut response| {
            *response.headers_mut() = headers;
            response
        })
        .map_err(|e| ApiError::internal(format!("Failed to build backup stream response: {}", e)))
}

pub async fn restore_backup(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(request): Json<BackupRestoreRequest>,
) -> Result<Json<ApiResponse<BackupRestoreResponse>>, ApiError> {
    ensure_superuser(&authenticated_user, "restore backups")?;

    let response = BackupHandlers::restore(state.db, state.vfs_service, request).await?;
    Ok(Json(ApiResponse::success(response)))
}

async fn build_backup_export_plan(
    db: Arc<dyn Db>,
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    query: BackupQuery,
) -> Result<BackupExportPlan, ApiError> {
    let selected_collections = parse_collection_filter(&query.collections);
    let mut schemas = db.list_collections().await?;
    schemas.sort_by(|a, b| a.name.cmp(&b.name));

    let record_limit = backup_export_record_limit(query.max_records);
    let mut collections = Vec::new();
    let mut total_records = 0usize;

    for schema in schemas {
        if !should_include_collection(&schema, query.include_system, &selected_collections) {
            continue;
        }

        let record_count = db.count_records(&schema.name).await?;
        total_records = total_records.checked_add(record_count).ok_or_else(|| {
            ApiError::bad_request("Backup export record count overflowed".to_string())
        })?;
        ensure_export_record_limit(record_limit, total_records, &schema.name)?;
        collections.push((schema, record_count));
    }
    let included_collection_names = collections
        .iter()
        .map(|(schema, _)| schema.name.clone())
        .collect::<Vec<_>>();
    let vfs_namespaces = if query.include_vfs {
        build_vfs_export_plan(vfs_service, &included_collection_names).await?
    } else {
        Vec::new()
    };
    let total_vfs_files = vfs_namespaces
        .iter()
        .map(|namespace| namespace.file_count)
        .sum();
    let total_vfs_size_bytes = vfs_namespaces
        .iter()
        .map(|namespace| namespace.total_size_bytes)
        .sum();

    Ok(BackupExportPlan {
        generated_at: chrono::Utc::now().to_rfc3339(),
        include_system: query.include_system,
        include_vfs: query.include_vfs,
        collections,
        total_records,
        vfs_namespaces,
        total_vfs_files,
        total_vfs_size_bytes,
    })
}

fn stream_backup_json_body(
    db: Arc<dyn Db>,
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    plan: BackupExportPlan,
) -> Body {
    let stream = async_stream::try_stream! {
        let mut header = serde_json::to_vec(&serde_json::json!({
            "format_version": BACKUP_FORMAT_VERSION,
            "generated_at": plan.generated_at,
            "include_system": plan.include_system,
            "include_vfs": plan.include_vfs,
            "total_collections": plan.collections.len(),
            "total_records": plan.total_records,
            "total_vfs_files": plan.total_vfs_files,
            "total_vfs_size_bytes": plan.total_vfs_size_bytes,
        }))
        .map_err(|e| ApiError::internal(format!("Failed to encode backup stream header: {}", e)))?;
        match header.pop() {
            Some(b'}') => {}
            _ => Err(ApiError::internal("Failed to prepare backup stream header".to_string()))?,
        }
        header.extend_from_slice(b",\"collections\":[");
        yield Bytes::from(header);

        for (collection_index, (schema, expected_records)) in plan.collections.into_iter().enumerate() {
            if collection_index > 0 {
                yield Bytes::from_static(b",");
            }

            let collection_name = schema.name.clone();
            yield Bytes::from_static(b"{\"schema\":");
            yield backup_json_chunk(&schema)?;
            yield Bytes::from_static(b",\"records\":[");

            let mut offset = 0usize;
            let mut records_written = 0usize;
            let mut first_record = true;

            loop {
                let batch = db
                    .list_records(
                        &collection_name,
                        ListParams {
                            limit: Some(EXPORT_PAGE_SIZE),
                            offset: Some(offset),
                            ..Default::default()
                        },
                    )
                    .await?;

                let batch_len = batch.len();
                if records_written.saturating_add(batch_len) > expected_records {
                    Err::<(), ApiError>(ApiError::bad_request(format!(
                        "Collection '{}' changed during backup export; retry the export",
                        collection_name
                    )))?;
                }

                for record in batch {
                    if !first_record {
                        yield Bytes::from_static(b",");
                    }
                    first_record = false;
                    yield backup_json_chunk(&record)?;
                    records_written += 1;
                }

                if batch_len < EXPORT_PAGE_SIZE {
                    break;
                }

                offset += EXPORT_PAGE_SIZE;
            }

            yield Bytes::from_static(b"],\"record_count\":");
            yield Bytes::from(records_written.to_string());
            yield Bytes::from_static(b"}");
        }

        yield Bytes::from_static(b"],\"vfs_namespaces\":[");

        for (namespace_index, namespace_plan) in plan.vfs_namespaces.into_iter().enumerate() {
            if namespace_index > 0 {
                yield Bytes::from_static(b",");
            }

            let vfs = vfs_service.as_ref().ok_or_else(|| {
                ApiError::bad_request("VFS service is not available for backup export".to_string())
            })?;
            let namespace = namespace_plan.config.namespace.clone();

            yield Bytes::from_static(b"{\"config\":");
            yield backup_json_chunk(&namespace_plan.config)?;
            yield Bytes::from_static(b",\"files\":[");

            let mut offset = 0usize;
            let mut files_written = 0usize;
            let mut first_file = true;

            loop {
                let file_list = vfs
                    .list_files(
                        &namespace,
                        FileListRequest {
                            directory: String::new(),
                            recursive: true,
                            mime_filter: None,
                            tag_filter: None,
                            offset: Some(offset),
                            limit: Some(EXPORT_PAGE_SIZE),
                        },
                    )
                    .await
                    .map_err(|e| vfs_export_error("list VFS files", &namespace, e))?;

                let batch_len = file_list.files.len();
                if files_written.saturating_add(batch_len) > namespace_plan.file_count {
                    Err::<(), ApiError>(ApiError::bad_request(format!(
                        "VFS namespace '{}' changed during backup export; retry the export",
                        namespace
                    )))?;
                }

                for metadata in file_list.files {
                    if !first_file {
                        yield Bytes::from_static(b",");
                    }
                    first_file = false;
                    let file_export = export_vfs_file(vfs.as_ref(), &namespace, metadata).await?;
                    yield backup_json_chunk(&file_export)?;
                    files_written += 1;
                }

                if batch_len < EXPORT_PAGE_SIZE {
                    break;
                }

                offset += EXPORT_PAGE_SIZE;
            }

            yield Bytes::from_static(b"],\"file_count\":");
            yield Bytes::from(files_written.to_string());
            yield Bytes::from_static(b",\"total_size_bytes\":");
            yield Bytes::from(namespace_plan.total_size_bytes.to_string());
            yield Bytes::from_static(b"}");
        }

        yield Bytes::from_static(b"]}");
    };

    Body::from_stream(stream.map_err(|error: ApiError| -> BoxError { Box::new(error) }))
}

fn backup_json_chunk<T: Serialize>(value: &T) -> Result<Bytes, ApiError> {
    serde_json::to_vec(value)
        .map(Bytes::from)
        .map_err(|e| ApiError::internal(format!("Failed to encode backup stream: {}", e)))
}

async fn build_vfs_manifest_summaries(
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    namespaces: &[String],
) -> Result<Vec<BackupVfsNamespaceSummary>, ApiError> {
    let vfs = require_vfs_service(vfs_service)?;
    let mut summaries = Vec::new();

    for namespace in namespaces {
        if optional_vfs_namespace_config(vfs.as_ref(), namespace)
            .await?
            .is_none()
        {
            continue;
        }

        let stats = vfs
            .get_usage_stats(namespace)
            .await
            .map_err(|e| vfs_export_error("read VFS usage stats", namespace, e))?;
        summaries.push(BackupVfsNamespaceSummary {
            namespace: namespace.clone(),
            file_count: stats.file_count,
            total_size_bytes: stats.storage_used,
        });
    }

    Ok(summaries)
}

async fn build_vfs_export_plan(
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    namespaces: &[String],
) -> Result<Vec<BackupVfsNamespacePlan>, ApiError> {
    let vfs = require_vfs_service(vfs_service)?;
    let mut plans = Vec::new();

    for namespace in namespaces {
        let Some(config) = optional_vfs_namespace_config(vfs.as_ref(), namespace).await? else {
            continue;
        };
        let stats = vfs
            .get_usage_stats(namespace)
            .await
            .map_err(|e| vfs_export_error("read VFS usage stats", namespace, e))?;

        plans.push(BackupVfsNamespacePlan {
            config,
            file_count: stats.file_count,
            total_size_bytes: stats.storage_used,
        });
    }

    Ok(plans)
}

async fn export_vfs_namespaces(
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    plans: &[BackupVfsNamespacePlan],
) -> Result<Vec<BackupVfsNamespaceExport>, ApiError> {
    if plans.is_empty() {
        return Ok(Vec::new());
    }

    let vfs = require_vfs_service(vfs_service)?;
    let mut namespaces = Vec::with_capacity(plans.len());

    for plan in plans {
        let files =
            export_vfs_namespace_files(vfs.as_ref(), &plan.config.namespace, plan.file_count)
                .await?;
        let total_size_bytes = files.iter().map(|file| file.metadata.size).sum();
        namespaces.push(BackupVfsNamespaceExport {
            config: plan.config.clone(),
            file_count: files.len(),
            total_size_bytes,
            files,
        });
    }

    Ok(namespaces)
}

async fn export_vfs_namespace_files(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
    expected_files: usize,
) -> Result<Vec<BackupVfsFileExport>, ApiError> {
    let mut files = Vec::with_capacity(expected_files);
    let mut offset = 0usize;

    loop {
        let file_list = vfs
            .list_files(
                &namespace.to_string(),
                FileListRequest {
                    directory: String::new(),
                    recursive: true,
                    mime_filter: None,
                    tag_filter: None,
                    offset: Some(offset),
                    limit: Some(EXPORT_PAGE_SIZE),
                },
            )
            .await
            .map_err(|e| vfs_export_error("list VFS files", namespace, e))?;

        let batch_len = file_list.files.len();
        if files.len().saturating_add(batch_len) > expected_files {
            return Err(ApiError::bad_request(format!(
                "VFS namespace '{}' changed during backup export; retry the export",
                namespace
            )));
        }

        for metadata in file_list.files {
            files.push(export_vfs_file(vfs, namespace, metadata).await?);
        }

        if batch_len < EXPORT_PAGE_SIZE {
            break;
        }

        offset += EXPORT_PAGE_SIZE;
    }

    Ok(files)
}

async fn export_vfs_file(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
    metadata: FileMetadata,
) -> Result<BackupVfsFileExport, ApiError> {
    let response = vfs
        .read_file(
            &namespace.to_string(),
            FileReadRequest {
                identifier: FileIdentifier::Id(metadata.id),
                include_content: true,
            },
        )
        .await
        .map_err(|e| vfs_export_error("read VFS file", namespace, e))?;
    let content = response.content.ok_or_else(|| {
        ApiError::internal(format!(
            "VFS read for namespace '{}' did not return file content",
            namespace
        ))
    })?;

    Ok(BackupVfsFileExport {
        metadata: response.metadata,
        content_encoding: "base64".to_string(),
        content_base64: BASE64_STANDARD.encode(content),
    })
}

#[allow(clippy::too_many_arguments)]
async fn restore_vfs_namespaces(
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
    namespaces: Vec<BackupVfsNamespaceExport>,
    include_vfs: bool,
    include_system: bool,
    replace_existing: bool,
    dry_run: bool,
    journal: &mut RestoreJournal,
    response: &mut BackupRestoreResponse,
) -> Result<(), ApiError> {
    if namespaces.is_empty() {
        return Ok(());
    }

    if !include_vfs {
        for namespace_export in namespaces {
            let namespace_name = namespace_export.config.namespace;
            let source_files = namespace_export.files.len();
            response.skipped_vfs_namespaces += 1;
            response.skipped_vfs_files += source_files;
            response
                .vfs_namespaces
                .push(BackupVfsRestoreNamespaceResult {
                    namespace: namespace_name,
                    status: "skipped_vfs_disabled".to_string(),
                    source_files,
                    files_created: 0,
                    files_skipped: source_files,
                    warnings: vec![
                        "VFS files skipped; enable include_vfs to restore them".to_string()
                    ],
                });
        }
        return Ok(());
    }

    let vfs = require_vfs_service(vfs_service)?;

    for namespace_export in namespaces {
        let namespace_name = namespace_export.config.namespace.clone();
        let source_files = namespace_export.files.len();
        let mut warnings = Vec::new();

        if namespace_export.file_count != source_files {
            warnings.push(format!(
                "Snapshot file_count was {}, but {} files were present",
                namespace_export.file_count, source_files
            ));
        }

        if is_system_collection(&namespace_name) && !include_system {
            response.skipped_vfs_namespaces += 1;
            response.skipped_vfs_files += source_files;
            warnings.push(
                "System VFS namespace skipped; enable include_system to restore it".to_string(),
            );
            response
                .vfs_namespaces
                .push(BackupVfsRestoreNamespaceResult {
                    namespace: namespace_name,
                    status: "skipped_system".to_string(),
                    source_files,
                    files_created: 0,
                    files_skipped: source_files,
                    warnings,
                });
            continue;
        }

        for file in &namespace_export.files {
            validate_vfs_file_export(&namespace_name, file)?;
        }

        let namespace_exists = optional_vfs_namespace_config(vfs.as_ref(), &namespace_name)
            .await?
            .is_some();
        let namespace_level_journaled = (replace_existing || !namespace_exists) && !dry_run;
        let mut status = if namespace_exists {
            "merged".to_string()
        } else if dry_run {
            "would_create".to_string()
        } else {
            vfs.create_namespace(namespace_export.config.clone())
                .await
                .map_err(|e| vfs_restore_error("create VFS namespace", &namespace_name, e))?;
            journal.delete_created_vfs_namespace(namespace_name.clone());
            response.created_vfs_namespaces += 1;
            "created".to_string()
        };

        if namespace_exists && replace_existing {
            status = if dry_run {
                "would_replace".to_string()
            } else {
                let (previous_config, previous_files) =
                    snapshot_vfs_namespace_for_restore(vfs.as_ref(), &namespace_name).await?;
                journal.restore_vfs_namespace(previous_config, previous_files);
                delete_vfs_namespace_contents(vfs.as_ref(), &namespace_name).await?;
                vfs.delete_namespace(&namespace_name)
                    .await
                    .map_err(|e| vfs_restore_error("delete VFS namespace", &namespace_name, e))?;
                vfs.create_namespace(namespace_export.config.clone())
                    .await
                    .map_err(|e| vfs_restore_error("create VFS namespace", &namespace_name, e))?;
                "replaced".to_string()
            };
            response.replaced_vfs_namespaces += 1;
        } else if !namespace_exists && dry_run {
            response.created_vfs_namespaces += 1;
        }

        let mut files_created = 0usize;
        let mut files_skipped = 0usize;

        if namespace_exists && !replace_existing {
            for file in namespace_export.files {
                if vfs_file_exists(vfs.as_ref(), &namespace_name, &file.metadata).await? {
                    files_skipped += 1;
                    continue;
                }

                if !dry_run {
                    let file_id = file.metadata.id.clone();
                    write_vfs_file_export(vfs.as_ref(), &namespace_name, file).await?;
                    if !namespace_level_journaled {
                        journal.delete_created_vfs_file(namespace_name.clone(), file_id);
                    }
                }
                files_created += 1;
            }
        } else {
            files_created = source_files;
            if !dry_run {
                let overwrite_stale_namespace_metadata = namespace_exists && replace_existing;
                for file in namespace_export.files {
                    let file_id = file.metadata.id.clone();
                    write_vfs_file_export_with_overwrite(
                        vfs.as_ref(),
                        &namespace_name,
                        file,
                        overwrite_stale_namespace_metadata,
                    )
                    .await?;
                    if !namespace_level_journaled {
                        journal.delete_created_vfs_file(namespace_name.clone(), file_id);
                    }
                }
            }
        }

        response.created_vfs_files += files_created;
        response.skipped_vfs_files += files_skipped;
        response
            .vfs_namespaces
            .push(BackupVfsRestoreNamespaceResult {
                namespace: namespace_name,
                status,
                source_files,
                files_created,
                files_skipped,
                warnings,
            });
    }

    Ok(())
}

async fn snapshot_collection_for_restore(
    db: Arc<dyn Db>,
    collection: &str,
) -> Result<(CollectionSchema, Vec<Record>), ApiError> {
    let schema = db.get_collection_schema(collection).await?;
    let record_count = db.count_records(collection).await?;
    let records = export_collection_records(Arc::clone(&db), collection, record_count).await?;

    Ok((schema, records))
}

async fn restore_collection_state(
    db: Arc<dyn Db>,
    schema: CollectionSchema,
    records: Vec<Record>,
) -> Result<(), ApiError> {
    let collection_name = schema.name.clone();

    delete_collection_if_exists(Arc::clone(&db), &collection_name).await?;
    db.create_collection(schema).await?;

    for mut record in records {
        record.collection = collection_name.clone();
        db.upsert_record_with_metadata(&collection_name, record)
            .await?;
    }

    Ok(())
}

async fn delete_collection_if_exists(db: Arc<dyn Db>, collection: &str) -> Result<(), ApiError> {
    if db.collection_exists(collection).await? {
        db.delete_collection(collection).await?;
    }

    Ok(())
}

async fn delete_record_if_exists(
    db: Arc<dyn Db>,
    collection: &str,
    record_id: &str,
) -> Result<(), ApiError> {
    if record_exists(&db, collection, record_id).await? {
        db.delete_record(collection, &record_id.to_string()).await?;
    }

    Ok(())
}

async fn snapshot_vfs_namespace_for_restore(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
) -> Result<(VfsNamespaceConfig, Vec<BackupVfsFileExport>), ApiError> {
    let config = vfs
        .get_namespace_config(&namespace.to_string())
        .await
        .map_err(|e| vfs_restore_error("read VFS namespace config", namespace, e))?;
    let stats = vfs
        .get_usage_stats(&namespace.to_string())
        .await
        .map_err(|e| vfs_restore_error("read VFS usage stats", namespace, e))?;
    let files = export_vfs_namespace_files(vfs, namespace, stats.file_count).await?;

    Ok((config, files))
}

async fn restore_vfs_namespace_state(
    vfs: Arc<dyn VirtualFileSystem>,
    config: VfsNamespaceConfig,
    files: Vec<BackupVfsFileExport>,
) -> Result<(), ApiError> {
    let namespace = config.namespace.clone();

    delete_vfs_namespace_if_exists(Arc::clone(&vfs), &namespace).await?;
    vfs.create_namespace(config)
        .await
        .map_err(|e| vfs_restore_error("create VFS namespace during rollback", &namespace, e))?;

    for file in files {
        write_vfs_file_export_with_overwrite(vfs.as_ref(), &namespace, file, true).await?;
    }

    Ok(())
}

async fn delete_vfs_namespace_if_exists(
    vfs: Arc<dyn VirtualFileSystem>,
    namespace: &str,
) -> Result<(), ApiError> {
    if optional_vfs_namespace_config(vfs.as_ref(), namespace)
        .await?
        .is_some()
    {
        delete_vfs_namespace_contents(vfs.as_ref(), namespace).await?;
        vfs.delete_namespace(&namespace.to_string())
            .await
            .map_err(|e| vfs_restore_error("delete VFS namespace during rollback", namespace, e))?;
    }

    Ok(())
}

async fn delete_vfs_file_if_exists(
    vfs: Arc<dyn VirtualFileSystem>,
    namespace: &str,
    file_id: &str,
) -> Result<(), ApiError> {
    match vfs
        .read_file(
            &namespace.to_string(),
            FileReadRequest {
                identifier: FileIdentifier::Id(file_id.to_string()),
                include_content: false,
            },
        )
        .await
    {
        Ok(_) => {
            vfs.delete_file(
                &namespace.to_string(),
                FileIdentifier::Id(file_id.to_string()),
            )
            .await
            .map_err(|e| vfs_restore_error("delete VFS file during rollback", namespace, e))?;
            Ok(())
        }
        Err(VfsError::FileNotFound { .. }) => Ok(()),
        Err(error) => Err(vfs_restore_error(
            "read VFS file during rollback",
            namespace,
            error,
        )),
    }
}

async fn delete_vfs_namespace_contents(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
) -> Result<(), ApiError> {
    loop {
        let file_list = vfs
            .list_files(
                &namespace.to_string(),
                FileListRequest {
                    directory: String::new(),
                    recursive: true,
                    mime_filter: None,
                    tag_filter: None,
                    offset: Some(0),
                    limit: Some(EXPORT_PAGE_SIZE),
                },
            )
            .await
            .map_err(|e| vfs_restore_error("list VFS namespace files", namespace, e))?;

        if file_list.files.is_empty() {
            break;
        }

        for metadata in file_list.files {
            match vfs
                .delete_file(&namespace.to_string(), FileIdentifier::Id(metadata.id))
                .await
            {
                Ok(()) | Err(VfsError::FileNotFound { .. }) => {}
                Err(error) => {
                    return Err(vfs_restore_error(
                        "delete VFS namespace file",
                        namespace,
                        error,
                    ));
                }
            }
        }
    }

    Ok(())
}

async fn vfs_file_exists(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
    metadata: &FileMetadata,
) -> Result<bool, ApiError> {
    match vfs
        .read_file(
            &namespace.to_string(),
            FileReadRequest {
                identifier: FileIdentifier::Id(metadata.id.clone()),
                include_content: false,
            },
        )
        .await
    {
        Ok(_) => return Ok(true),
        Err(VfsError::FileNotFound { .. }) => {}
        Err(e) => return Err(vfs_restore_error("read VFS file metadata", namespace, e)),
    }

    match vfs
        .read_file(
            &namespace.to_string(),
            FileReadRequest {
                identifier: FileIdentifier::Path(metadata.path.clone()),
                include_content: false,
            },
        )
        .await
    {
        Ok(_) => Ok(true),
        Err(VfsError::FileNotFound { .. }) => Ok(false),
        Err(e) => Err(vfs_restore_error("read VFS file metadata", namespace, e)),
    }
}

async fn write_vfs_file_export(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
    file: BackupVfsFileExport,
) -> Result<(), ApiError> {
    write_vfs_file_export_with_overwrite(vfs, namespace, file, false).await
}

async fn write_vfs_file_export_with_overwrite(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
    file: BackupVfsFileExport,
    overwrite: bool,
) -> Result<(), ApiError> {
    let content = decode_vfs_file_content(namespace, &file)?;
    let metadata = file.metadata;

    vfs.write_file(
        &namespace.to_string(),
        FileWriteRequest {
            file_id: Some(metadata.id),
            path: metadata.path,
            content,
            mime_type: Some(metadata.mime_type),
            custom_metadata: Some(metadata.custom_metadata),
            tags: Some(metadata.tags),
            overwrite,
            created_at: Some(metadata.created_at),
            modified_at: Some(metadata.modified_at),
        },
    )
    .await
    .map_err(|e| vfs_restore_error("write VFS file", namespace, e))?;

    Ok(())
}

fn decode_vfs_file_content(
    namespace: &str,
    file: &BackupVfsFileExport,
) -> Result<Vec<u8>, ApiError> {
    validate_vfs_file_export(namespace, file)?;
    let content = BASE64_STANDARD
        .decode(file.content_base64.as_bytes())
        .map_err(|e| {
            ApiError::bad_request(format!(
                "Invalid base64 content for VFS file '{}' in namespace '{}': {}",
                file.metadata.path, namespace, e
            ))
        })?;

    if content.len() as u64 != file.metadata.size {
        return Err(ApiError::bad_request(format!(
            "VFS file '{}' in namespace '{}' has {} decoded bytes but metadata declares {}",
            file.metadata.path,
            namespace,
            content.len(),
            file.metadata.size
        )));
    }

    Ok(content)
}

fn validate_vfs_file_export(namespace: &str, file: &BackupVfsFileExport) -> Result<(), ApiError> {
    if !file.content_encoding.eq_ignore_ascii_case("base64") {
        return Err(ApiError::bad_request(format!(
            "Unsupported VFS content encoding '{}' for namespace '{}'",
            file.content_encoding, namespace
        )));
    }

    if file.metadata.id.trim().is_empty() {
        return Err(ApiError::bad_request(format!(
            "VFS file in namespace '{}' is missing an id",
            namespace
        )));
    }

    if file.metadata.path.trim().is_empty() {
        return Err(ApiError::bad_request(format!(
            "VFS file '{}' in namespace '{}' has an empty path",
            file.metadata.id, namespace
        )));
    }

    Ok(())
}

async fn optional_vfs_namespace_config(
    vfs: &dyn VirtualFileSystem,
    namespace: &str,
) -> Result<Option<VfsNamespaceConfig>, ApiError> {
    match vfs.get_namespace_config(&namespace.to_string()).await {
        Ok(config) => Ok(Some(config)),
        Err(VfsError::AccessDenied { .. } | VfsError::FileNotFound { .. }) => Ok(None),
        Err(e) => Err(vfs_export_error("read VFS namespace config", namespace, e)),
    }
}

fn require_vfs_service(
    vfs_service: Option<Arc<dyn VirtualFileSystem>>,
) -> Result<Arc<dyn VirtualFileSystem>, ApiError> {
    vfs_service.ok_or_else(|| {
        ApiError::bad_request("VFS service is not available for this backup operation".to_string())
    })
}

fn vfs_export_error(action: &str, namespace: &str, error: VfsError) -> ApiError {
    match error {
        VfsError::InvalidPath { .. } => ApiError::bad_request(format!(
            "Invalid VFS namespace or path while attempting to {} for '{}'",
            action, namespace
        )),
        VfsError::AccessDenied { .. } | VfsError::FileNotFound { .. } => {
            ApiError::bad_request(format!(
                "VFS namespace '{}' changed while attempting to {}; retry the export",
                namespace, action
            ))
        }
        VfsError::QuotaExceeded { .. } => ApiError::bad_request(format!(
            "VFS quota prevented backup export while attempting to {} for '{}'",
            action, namespace
        )),
        other => ApiError::internal(format!(
            "Failed to {} for VFS namespace '{}': {}",
            action, namespace, other
        )),
    }
}

fn vfs_restore_error(action: &str, namespace: &str, error: VfsError) -> ApiError {
    match error {
        VfsError::InvalidPath { .. } => ApiError::bad_request(format!(
            "Invalid VFS data while attempting to {} for '{}'",
            action, namespace
        )),
        VfsError::FileAlreadyExists { .. } => ApiError::conflict(format!(
            "VFS file already exists while attempting to {} for '{}'",
            action, namespace
        )),
        VfsError::QuotaExceeded { .. } => ApiError::bad_request(format!(
            "VFS quota exceeded while attempting to {} for '{}'",
            action, namespace
        )),
        VfsError::AccessDenied { .. } => ApiError::forbidden(format!(
            "VFS access denied while attempting to {} for '{}'",
            action, namespace
        )),
        VfsError::FileNotFound { .. } => ApiError::not_found(format!(
            "VFS file not found while attempting to {} for '{}'",
            action, namespace
        )),
        other => ApiError::internal(format!(
            "Failed to {} for VFS namespace '{}': {}",
            action, namespace, other
        )),
    }
}

fn restore_error_with_rollback(error: ApiError, rollback_errors: Vec<String>) -> ApiError {
    let message = if rollback_errors.is_empty() {
        format!("{}; restore journal rollback completed", error)
    } else {
        format!(
            "{}; restore journal rollback also failed: {}",
            error,
            rollback_errors.join("; ")
        )
    };

    if rollback_errors.is_empty() {
        match error {
            ApiError::BadRequest { .. } => ApiError::bad_request(message),
            ApiError::Forbidden { .. } => ApiError::forbidden(message),
            ApiError::NotFound { .. } => ApiError::not_found(message),
            ApiError::Conflict { .. } => ApiError::conflict(message),
            ApiError::ServiceUnavailable { .. } => ApiError::service_unavailable(message),
            _ => ApiError::internal(message),
        }
    } else {
        ApiError::internal(message)
    }
}

fn backup_stream_filename(generated_at: &str) -> String {
    let stamp = generated_at
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    format!("{}-{}.json", STREAM_EXPORT_FILE_PREFIX, stamp)
}

async fn export_collection_records(
    db: Arc<dyn Db>,
    collection: &str,
    expected_records: usize,
) -> Result<Vec<Record>, ApiError> {
    let mut records = Vec::with_capacity(expected_records);
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
        if records.len().saturating_add(batch_len) > expected_records {
            return Err(ApiError::bad_request(format!(
                "Collection '{}' changed during backup export; retry the export",
                collection
            )));
        }

        records.extend(batch);

        if batch_len < EXPORT_PAGE_SIZE {
            break;
        }

        offset += EXPORT_PAGE_SIZE;
    }

    Ok(records)
}

fn backup_export_record_limit(query_limit: Option<usize>) -> Option<usize> {
    let server_limit = match std::env::var(EXPORT_RECORD_LIMIT_ENV) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed == "0" || trimmed.eq_ignore_ascii_case("none") {
                None
            } else {
                match trimmed.parse::<usize>() {
                    Ok(limit) => Some(limit),
                    Err(error) => {
                        warn!(
                            "Ignoring invalid {} value '{}': {}",
                            EXPORT_RECORD_LIMIT_ENV, value, error
                        );
                        Some(DEFAULT_EXPORT_RECORD_LIMIT)
                    }
                }
            }
        }
        Err(_) => Some(DEFAULT_EXPORT_RECORD_LIMIT),
    };

    match (server_limit, query_limit) {
        (Some(server_limit), Some(query_limit)) => Some(server_limit.min(query_limit)),
        (Some(server_limit), None) => Some(server_limit),
        (None, Some(query_limit)) => Some(query_limit),
        (None, None) => None,
    }
}

fn ensure_export_record_limit(
    record_limit: Option<usize>,
    planned_records: usize,
    collection_name: &str,
) -> Result<(), ApiError> {
    if let Some(record_limit) = record_limit {
        if planned_records > record_limit {
            return Err(ApiError::bad_request(format!(
                "Backup export would include more than {} records after collection '{}'. Narrow the collection filter or raise {}.",
                record_limit, collection_name, EXPORT_RECORD_LIMIT_ENV
            )));
        }
    }

    Ok(())
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
        CollectionType, FieldDefinition, FieldType, FileIdentifier, FileListRequest,
        FileListResponse, FileMetadata, FileMoveRequest, FileReadRequest, FileReadResponse,
        FileWriteRequest, InMemoryEventBus, VfsError, VfsNamespace, VfsNamespaceConfig, VfsResult,
        VfsUsageStats, VirtualFileSystem,
    };
    use oxide_db::SqliteDb;
    use oxide_vfs::VfsService;

    struct FailingWriteVfs {
        inner: Arc<VfsService>,
        fail_path: String,
    }

    #[async_trait::async_trait]
    impl VirtualFileSystem for FailingWriteVfs {
        async fn create_namespace(&self, config: VfsNamespaceConfig) -> VfsResult<()> {
            self.inner.create_namespace(config).await
        }

        async fn delete_namespace(&self, namespace: &VfsNamespace) -> VfsResult<()> {
            self.inner.delete_namespace(namespace).await
        }

        async fn write_file(
            &self,
            namespace: &VfsNamespace,
            request: FileWriteRequest,
        ) -> VfsResult<FileMetadata> {
            if request.path == self.fail_path {
                return Err(VfsError::IoError {
                    message: "injected restore failure".to_string(),
                });
            }

            self.inner.write_file(namespace, request).await
        }

        async fn move_file(
            &self,
            namespace: &VfsNamespace,
            request: FileMoveRequest,
        ) -> VfsResult<FileMetadata> {
            self.inner.move_file(namespace, request).await
        }

        async fn read_file(
            &self,
            namespace: &VfsNamespace,
            request: FileReadRequest,
        ) -> VfsResult<FileReadResponse> {
            self.inner.read_file(namespace, request).await
        }

        async fn delete_file(
            &self,
            namespace: &VfsNamespace,
            identifier: FileIdentifier,
        ) -> VfsResult<()> {
            self.inner.delete_file(namespace, identifier).await
        }

        async fn list_files(
            &self,
            namespace: &VfsNamespace,
            request: FileListRequest,
        ) -> VfsResult<FileListResponse> {
            self.inner.list_files(namespace, request).await
        }

        async fn get_usage_stats(&self, namespace: &VfsNamespace) -> VfsResult<VfsUsageStats> {
            self.inner.get_usage_stats(namespace).await
        }

        async fn create_backup(&self, namespace: &VfsNamespace) -> VfsResult<String> {
            self.inner.create_backup(namespace).await
        }

        async fn restore_backup(&self, namespace: &VfsNamespace, backup_id: &str) -> VfsResult<()> {
            self.inner.restore_backup(namespace, backup_id).await
        }

        async fn get_namespace_config(
            &self,
            namespace: &VfsNamespace,
        ) -> VfsResult<VfsNamespaceConfig> {
            self.inner.get_namespace_config(namespace).await
        }

        async fn update_namespace_config(&self, config: VfsNamespaceConfig) -> VfsResult<()> {
            self.inner.update_namespace_config(config).await
        }
    }

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
            None,
            BackupQuery {
                include_system: false,
                include_vfs: false,
                collections: Some("articles".to_string()),
                max_records: None,
            },
        )
        .await
        .unwrap();

        db.delete_collection("articles").await.unwrap();

        let preview = BackupHandlers::restore(
            Arc::clone(&db),
            None,
            BackupRestoreRequest {
                snapshot: snapshot.clone(),
                dry_run: true,
                include_system: false,
                replace_existing: false,
                include_vfs: true,
            },
        )
        .await
        .unwrap();

        assert!(preview.dry_run);
        assert_eq!(preview.created_collections, 1);
        assert_eq!(preview.created_records, 1);

        let restored_summary = BackupHandlers::restore(
            Arc::clone(&db),
            None,
            BackupRestoreRequest {
                snapshot,
                dry_run: false,
                include_system: false,
                replace_existing: false,
                include_vfs: true,
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

    #[tokio::test]
    async fn export_and_restore_include_vfs_files_with_stable_ids(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = test_db();
        db.initialize().await?;

        let mut schema = CollectionSchema::new("articles".to_string(), CollectionType::Base);
        schema.add_field("title".to_string(), FieldDefinition::new(FieldType::Text));
        db.create_collection(schema).await?;

        let source_path = std::env::temp_dir().join(format!(
            "oxidedb-backup-vfs-source-{}",
            uuid::Uuid::new_v4()
        ));
        let target_path = std::env::temp_dir().join(format!(
            "oxidedb-backup-vfs-target-{}",
            uuid::Uuid::new_v4()
        ));

        let source_vfs = Arc::new(VfsService::new(source_path.clone(), None)?);
        source_vfs.initialize().await?;
        source_vfs
            .create_namespace(VfsNamespaceConfig {
                namespace: "articles".to_string(),
                ..Default::default()
            })
            .await?;
        let original_file = source_vfs
            .write_file(
                &"articles".to_string(),
                FileWriteRequest {
                    file_id: Some("stable-file-id".to_string()),
                    path: "uploads/original.txt".to_string(),
                    content: b"portable file content".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: Some(vec!["backup".to_string()]),
                    overwrite: false,
                    created_at: Some(123),
                    modified_at: Some(456),
                },
            )
            .await?;

        let source_vfs_trait: Arc<dyn VirtualFileSystem> = source_vfs.clone();
        let snapshot = BackupHandlers::export(
            Arc::clone(&db),
            Some(source_vfs_trait),
            BackupQuery {
                include_system: false,
                include_vfs: true,
                collections: Some("articles".to_string()),
                max_records: None,
            },
        )
        .await?;

        assert_eq!(snapshot.total_vfs_files, 1);
        assert_eq!(snapshot.vfs_namespaces.len(), 1);

        let target_vfs = Arc::new(VfsService::new(target_path.clone(), None)?);
        target_vfs.initialize().await?;
        let target_vfs_trait: Arc<dyn VirtualFileSystem> = target_vfs.clone();
        let restored_summary = BackupHandlers::restore(
            Arc::clone(&db),
            Some(target_vfs_trait),
            BackupRestoreRequest {
                snapshot,
                dry_run: false,
                include_system: false,
                replace_existing: false,
                include_vfs: true,
            },
        )
        .await?;

        assert_eq!(restored_summary.created_vfs_namespaces, 1);
        assert_eq!(restored_summary.created_vfs_files, 1);

        let restored_file = target_vfs
            .read_file(
                &"articles".to_string(),
                FileReadRequest {
                    identifier: FileIdentifier::Id(original_file.id.clone()),
                    include_content: true,
                },
            )
            .await?;

        assert_eq!(restored_file.metadata.id, original_file.id);
        assert_eq!(restored_file.metadata.created_at, 123);
        assert_eq!(restored_file.metadata.modified_at, 456);
        assert_eq!(
            restored_file.content.unwrap_or_default(),
            b"portable file content".to_vec()
        );

        let _ = tokio::fs::remove_dir_all(source_path).await;
        let _ = tokio::fs::remove_dir_all(target_path).await;

        Ok(())
    }

    #[tokio::test]
    async fn restore_rolls_back_collection_and_vfs_namespace_after_partial_failure(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let db = test_db();
        db.initialize().await?;

        let mut schema = CollectionSchema::new("articles".to_string(), CollectionType::Base);
        schema.add_field("title".to_string(), FieldDefinition::new(FieldType::Text));
        db.create_collection(schema.clone()).await?;

        let old_record = db
            .create_record("articles", serde_json::json!({ "title": "Old" }))
            .await?;

        let source_path = std::env::temp_dir().join(format!(
            "oxidedb-backup-vfs-rollback-source-{}",
            uuid::Uuid::new_v4()
        ));
        let target_path = std::env::temp_dir().join(format!(
            "oxidedb-backup-vfs-rollback-target-{}",
            uuid::Uuid::new_v4()
        ));

        let target_vfs = Arc::new(VfsService::new(target_path.clone(), None)?);
        target_vfs.initialize().await?;
        target_vfs
            .create_namespace(VfsNamespaceConfig {
                namespace: "articles".to_string(),
                ..Default::default()
            })
            .await?;
        let old_file = target_vfs
            .write_file(
                &"articles".to_string(),
                FileWriteRequest {
                    file_id: Some("old-file".to_string()),
                    path: "uploads/old.txt".to_string(),
                    content: b"old file content".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: Some(vec!["existing".to_string()]),
                    overwrite: false,
                    created_at: Some(100),
                    modified_at: Some(200),
                },
            )
            .await?;

        let source_vfs = Arc::new(VfsService::new(source_path.clone(), None)?);
        source_vfs.initialize().await?;
        let source_config = VfsNamespaceConfig {
            namespace: "articles".to_string(),
            ..Default::default()
        };
        source_vfs.create_namespace(source_config.clone()).await?;
        source_vfs
            .write_file(
                &"articles".to_string(),
                FileWriteRequest {
                    file_id: Some("new-file-1".to_string()),
                    path: "uploads/new-1.txt".to_string(),
                    content: b"new file one".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: false,
                    created_at: Some(300),
                    modified_at: Some(400),
                },
            )
            .await?;
        source_vfs
            .write_file(
                &"articles".to_string(),
                FileWriteRequest {
                    file_id: Some("new-file-2".to_string()),
                    path: "uploads/new-2.txt".to_string(),
                    content: b"new file two".to_vec(),
                    mime_type: Some("text/plain".to_string()),
                    custom_metadata: None,
                    tags: None,
                    overwrite: false,
                    created_at: Some(500),
                    modified_at: Some(600),
                },
            )
            .await?;

        let source_files = export_vfs_namespace_files(source_vfs.as_ref(), "articles", 2).await?;
        let source_total_size = source_files.iter().map(|file| file.metadata.size).sum();
        let snapshot = BackupExportResponse {
            format_version: BACKUP_FORMAT_VERSION,
            generated_at: "2026-06-14T00:00:00Z".to_string(),
            include_system: false,
            include_vfs: true,
            total_collections: 1,
            total_records: 1,
            total_vfs_files: source_files.len(),
            total_vfs_size_bytes: source_total_size,
            collections: vec![BackupCollectionExport {
                schema,
                records: vec![Record {
                    id: "new-record".to_string(),
                    collection: "articles".to_string(),
                    data: serde_json::json!({ "title": "New" }),
                    created_at: 700,
                    updated_at: 800,
                }],
                record_count: 1,
            }],
            vfs_namespaces: vec![BackupVfsNamespaceExport {
                config: source_config,
                files: source_files,
                file_count: 2,
                total_size_bytes: source_total_size,
            }],
        };
        let failing_vfs: Arc<dyn VirtualFileSystem> = Arc::new(FailingWriteVfs {
            inner: Arc::clone(&target_vfs),
            fail_path: "uploads/new-2.txt".to_string(),
        });

        let error = BackupHandlers::restore(
            Arc::clone(&db),
            Some(failing_vfs),
            BackupRestoreRequest {
                snapshot,
                dry_run: false,
                include_system: false,
                replace_existing: true,
                include_vfs: true,
            },
        )
        .await
        .unwrap_err();

        let error_message = error.to_string();
        assert!(
            error_message.contains("restore journal rollback completed"),
            "{}",
            error_message
        );

        let restored_record = db.read_record("articles", &old_record.id).await?;
        assert_eq!(restored_record.data["title"], serde_json::json!("Old"));
        let new_record_result = db.read_record("articles", &"new-record".to_string()).await;
        assert!(matches!(new_record_result, Err(AppError::NotFound { .. })));

        let restored_file = target_vfs
            .read_file(
                &"articles".to_string(),
                FileReadRequest {
                    identifier: FileIdentifier::Id(old_file.id),
                    include_content: true,
                },
            )
            .await?;
        assert_eq!(
            restored_file.content.unwrap_or_default(),
            b"old file content".to_vec()
        );

        let new_file_result = target_vfs
            .read_file(
                &"articles".to_string(),
                FileReadRequest {
                    identifier: FileIdentifier::Id("new-file-1".to_string()),
                    include_content: false,
                },
            )
            .await;
        assert!(matches!(
            new_file_result,
            Err(VfsError::FileNotFound { .. })
        ));

        let _ = tokio::fs::remove_dir_all(source_path).await;
        let _ = tokio::fs::remove_dir_all(target_path).await;

        Ok(())
    }

    #[tokio::test]
    async fn export_rejects_snapshots_above_requested_record_limit() {
        let db = test_db();
        db.initialize().await.unwrap();

        let mut schema = CollectionSchema::new("articles".to_string(), CollectionType::Base);
        schema.add_field("title".to_string(), FieldDefinition::new(FieldType::Text));
        db.create_collection(schema).await.unwrap();
        db.create_record("articles", serde_json::json!({ "title": "One" }))
            .await
            .unwrap();

        let error = BackupHandlers::export(
            Arc::clone(&db),
            None,
            BackupQuery {
                include_system: false,
                include_vfs: false,
                collections: Some("articles".to_string()),
                max_records: Some(0),
            },
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("Backup export would include"));
    }
}
