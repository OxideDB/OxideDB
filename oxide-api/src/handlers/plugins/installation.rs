//! Plugin installation and package management

use axum::{
    extract::{Multipart, State},
    Json,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::HashMap;
use std::convert::TryInto;
use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};
use zip::ZipArchive;

use super::{
    audit::record_plugin_audit_event,
    capabilities::{
        capability_satisfies, is_capability_allowed_for_trust_level,
        minimum_trust_level_for_capabilities, parse_capability_strings, parse_trust_level_string,
    },
    types::*,
};
use crate::{errors::ApiError, responses::ApiResponse, server::AppState};
use oxide_core::plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits};

const TRUSTED_PLUGIN_KEYS_ENV: &str = "OXIDEDB_PLUGIN_TRUSTED_KEYS";
const TRUSTED_PLUGIN_KEYS_FILE_ENV: &str = "OXIDEDB_PLUGIN_TRUSTED_KEYS_FILE";
const SIGNATURE_PAYLOAD_MAGIC: &[u8] = b"OxideDB plugin package signature v1\n";
const MAX_PLUGIN_PACKAGE_SIZE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PLUGIN_EXTRACTED_SIZE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PLUGIN_FILE_COUNT: usize = 512;
const MAX_PLUGIN_MANIFEST_SIZE_BYTES: u64 = 256 * 1024;
const MAX_PLUGIN_SIGNATURE_SIZE_BYTES: u64 = 16 * 1024;
const MAX_PLUGIN_WASM_SIZE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PLUGIN_SUPPLEMENTAL_FILE_SIZE_BYTES: u64 = 16 * 1024 * 1024;

static TEMP_EXTRACTION_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Register/Install a new plugin from a ZIP package
pub async fn register_plugin(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<PluginInfo>>, ApiError> {
    debug!("🔌 Starting plugin registration from ZIP package");

    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let mut package_data: Option<Vec<u8>> = None;
    let mut user_trust_level = PluginTrustLevel::Untrusted;
    let mut user_capabilities: Vec<PluginCapability> = Vec::new();

    // Parse multipart form data
    debug!("🔌 Parsing multipart form data");
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        error!("Multipart field parsing error: {}", e);
        ApiError::bad_request(format!("Error parsing multipart field: {}", e))
    })? {
        let field_name = field.name().unwrap_or("unknown");
        debug!("🔌 Processing field: {}", field_name);

        match field_name {
            "plugin_package" => {
                debug!("🔌 Reading plugin package");
                let file_name = field.file_name().unwrap_or("plugin.zip");
                debug!("🔌 Package file name: {}", file_name);

                package_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            error!("Error reading plugin package: {}", e);
                            ApiError::bad_request(format!("Error reading plugin package: {}", e))
                        })?
                        .to_vec(),
                );

                if let Some(ref data) = package_data {
                    debug!("🔌 Package size: {} bytes", data.len());
                }
            }
            "trust_level" => {
                let trust_str = field.text().await.map_err(|e| {
                    error!("Error reading trust_level field: {}", e);
                    ApiError::bad_request(format!("Invalid trust level: {}", e))
                })?;
                debug!("🔌 Trust level string: {}", trust_str);
                user_trust_level =
                    serde_json::from_str(&format!("\"{}\"", trust_str)).map_err(|e| {
                        error!("Error parsing trust level '{}': {}", trust_str, e);
                        ApiError::bad_request(format!("Invalid trust level format: {}", e))
                    })?;
                debug!("🔌 Trust level: {:?}", user_trust_level);
            }
            "capabilities" => {
                let caps_str = field.text().await.map_err(|e| {
                    error!("Error reading capabilities field: {}", e);
                    ApiError::bad_request(format!("Invalid capabilities: {}", e))
                })?;
                debug!("🔌 Capabilities string: {}", caps_str);

                // Parse as full PluginCapability objects
                user_capabilities = serde_json::from_str(&caps_str)
                    .map_err(|e| {
                        error!("Error parsing capabilities '{}': {}", caps_str, e);
                        ApiError::bad_request(format!(
                            "Invalid capabilities format: {}. Expected an array of capability objects with proper structure.",
                            e
                        ))
                    })?;
                debug!("🔌 Capabilities: {:?}", user_capabilities);
            }
            _ => {
                debug!("🔌 Skipping unknown field: {}", field_name);
                // Skip unknown fields - we need to consume the field to avoid errors
                let _ = field.bytes().await;
            }
        }
    }

    let package_data = package_data
        .ok_or_else(|| ApiError::bad_request("Missing plugin_package field".to_string()))?;

    // Extract and validate the plugin package
    debug!("🔌 Extracting plugin package");
    let package = extract_plugin_package(&package_data)?;
    let _extraction_cleanup = TempExtractionGuard::new(package.extraction_path.clone());
    let plugin_name = package.manifest.plugin.name.clone();
    let plugin_version = package.manifest.plugin.version.clone();

    // Verify digital signature if present
    let signature_valid = verify_plugin_signature(&package).await?;
    if let Err(error) = enforce_plugin_signature_policy(
        &plugin_name,
        plugin_manager.requires_code_signing(),
        signature_valid,
    ) {
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_signature_rejected",
            "failure",
            serde_json::json!({
                "source": "zip_package",
                "version": plugin_version,
                "code_signing_required": plugin_manager.requires_code_signing(),
                "signature_verified": signature_valid,
                "reason": error.to_string(),
            }),
        )
        .await;
        return Err(error);
    }

    if signature_valid {
        info!("✅ Plugin signature verified: {}", plugin_name);
    } else {
        warn!(
            "⚠️  Plugin signature not verified (may be unsigned): {}",
            plugin_name
        );
    }

    // Parse declared capabilities from manifest
    let declared_capabilities: Vec<PluginCapability> =
        parse_capability_strings(&package.manifest.security.required_capabilities)?;

    // Parse recommended trust level from manifest (for future use)
    let _recommended_trust_level =
        parse_trust_level_string(&package.manifest.security.recommended_trust_level)
            .unwrap_or(PluginTrustLevel::Untrusted);

    // If user didn't specify capabilities, use declared ones (but still require explicit trust)
    let final_capabilities = if user_capabilities.is_empty() {
        declared_capabilities.clone()
    } else {
        user_capabilities
    };

    record_plugin_audit_event(
        &state,
        &plugin_name,
        "plugin_install_attempted",
        "started",
        serde_json::json!({
            "source": "zip_package",
            "version": plugin_version,
            "trust_level": user_trust_level,
            "requested_capabilities": final_capabilities,
            "declared_capabilities": declared_capabilities,
            "signature_verified": signature_valid,
            "package_hash": package.package_hash,
            "package_size": package.package_size,
        }),
    )
    .await;

    // Validate that user-granted capabilities include all required ones
    let missing_capabilities: Vec<&PluginCapability> = declared_capabilities
        .iter()
        .filter(|req_cap| {
            !final_capabilities
                .iter()
                .any(|granted| capability_satisfies(granted, req_cap))
        })
        .collect();

    if !missing_capabilities.is_empty() {
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_install_failed",
            "failure",
            serde_json::json!({
                "source": "zip_package",
                "version": plugin_version,
                "reason": "missing_required_capabilities",
                "missing_capabilities": missing_capabilities,
            }),
        )
        .await;
        return Err(ApiError::bad_request(format!(
            "Plugin '{}' requires capabilities that were not granted: {:?}. Please grant these capabilities or contact the plugin author.",
            plugin_name, missing_capabilities
        )));
    }

    let disallowed_capabilities: Vec<&PluginCapability> = final_capabilities
        .iter()
        .filter(|capability| !is_capability_allowed_for_trust_level(&user_trust_level, capability))
        .collect();

    if !disallowed_capabilities.is_empty() {
        let minimum_trust_level = minimum_trust_level_for_capabilities(&final_capabilities);
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_install_failed",
            "failure",
            serde_json::json!({
                "source": "zip_package",
                "version": plugin_version,
                "reason": "capability_not_allowed_for_trust_level",
                "trust_level": user_trust_level,
                "minimum_trust_level": minimum_trust_level,
                "disallowed_capabilities": disallowed_capabilities,
            }),
        )
        .await;
        return Err(ApiError::bad_request(format!(
            "Trust level {:?} does not allow capabilities {:?}. Use at least {:?} for plugin '{}'.",
            user_trust_level, disallowed_capabilities, minimum_trust_level, plugin_name
        )));
    }

    // Check for potentially dangerous capabilities being granted
    let sensitive_capabilities = [
        "DeleteRecords",
        "ModifyEventData",
        "BlockOperations",
        "AccessVfs",
        "HttpRequest",
    ];
    let granted_sensitive: Vec<String> = final_capabilities
        .iter()
        .map(|cap| format!("{:?}", cap))
        .filter(|cap_str| {
            sensitive_capabilities
                .iter()
                .any(|sensitive| cap_str.contains(sensitive))
        })
        .collect();

    if !granted_sensitive.is_empty() {
        info!(
            "⚠️  Plugin '{}' granted sensitive capabilities: {:?}",
            plugin_name, granted_sensitive
        );
    }

    // Install plugin using directory-based approach
    let _record_id = match state
        .plugin_config_service
        .install_plugin_from_directory(
            plugin_name.clone(),
            plugin_version.clone(),
            package.manifest.plugin.description.clone(),
            package.manifest.plugin.author.clone(),
            user_trust_level.clone(),
            final_capabilities.clone(),
            ResourceLimits::default(),
            package.extraction_path.clone(),
            package.manifest.plugin.wasm_file.clone(),
            Some(serde_json::json!({
                "package_hash": package.package_hash,
                "package_size": package.package_size,
                "signature_verified": signature_valid,
                "manifest": package.manifest,
                "source": "zip_package",
                "extraction_path": package.extraction_path.display().to_string()
            })),
        )
        .await
    {
        Ok(record_id) => record_id,
        Err(error) => {
            record_plugin_audit_event(
                &state,
                &plugin_name,
                "plugin_install_failed",
                "failure",
                serde_json::json!({
                    "source": "zip_package",
                    "version": plugin_version,
                    "reason": error.to_string(),
                }),
            )
            .await;
            return Err(ApiError::internal(format!(
                "Failed to install plugin: {}",
                error
            )));
        }
    };

    let persisted_config = match state
        .plugin_config_service
        .get_plugin_config(&plugin_name)
        .await
    {
        Ok(config) => config,
        Err(error) => {
            record_plugin_audit_event(
                &state,
                &plugin_name,
                "plugin_load_failed",
                "failure",
                serde_json::json!({
                    "source": "zip_package",
                    "version": plugin_version,
                    "reason": error.to_string(),
                }),
            )
            .await;
            if let Err(rollback_error) = state
                .plugin_config_service
                .uninstall_plugin(&plugin_name)
                .await
            {
                error!(
                    "Failed to roll back plugin '{}' after persisted config lookup failure: {}",
                    plugin_name, rollback_error
                );
            }

            return Err(ApiError::internal(format!(
                "Failed to load persisted plugin configuration: {}",
                error
            )));
        }
    };

    // Load the plugin into runtime and register event handlers as one lifecycle step.
    let load_result = plugin_manager
        .load_and_register_plugin(&state.event_bus, &persisted_config, &package.wasm_data)
        .await;

    if let Err(e) = load_result {
        record_plugin_audit_event(
            &state,
            &plugin_name,
            "plugin_load_failed",
            "failure",
            serde_json::json!({
                "source": "zip_package",
                "version": plugin_version,
                "reason": e.to_string(),
            }),
        )
        .await;
        if let Err(rollback_error) = state
            .plugin_config_service
            .uninstall_plugin(&plugin_name)
            .await
        {
            error!(
                "Failed to roll back plugin '{}' after runtime load failure: {}",
                plugin_name, rollback_error
            );
        }

        return Err(ApiError::internal(format!("Failed to load plugin: {}", e)));
    }

    info!(
        "✅ Plugin '{}' v{} registered successfully from ZIP package",
        plugin_name, plugin_version
    );

    record_plugin_audit_event(
        &state,
        &plugin_name,
        "plugin_installed",
        "success",
        serde_json::json!({
            "source": "zip_package",
            "version": plugin_version,
            "trust_level": user_trust_level,
            "capabilities": final_capabilities,
            "signature_verified": signature_valid,
            "package_hash": package.package_hash,
            "package_size": package.package_size,
        }),
    )
    .await;

    // Return plugin info based on manifest data
    let plugin_info = PluginInfo {
        name: plugin_name.clone(),
        status: PluginStatus::Enabled,
        version: plugin_version,
        description: package.manifest.plugin.description.clone(),
        author: package.manifest.plugin.author.clone(),
        capabilities: final_capabilities,
        trust_level: user_trust_level,
        routes: vec![], // Routes will be populated by the runtime
        executions: 0,
        errors: 0,
        last_execution: None,
        resource_usage: ResourceUsageInfo::default(),
    };

    Ok(Json(ApiResponse::success(plugin_info)))
}

/// Extract and validate a plugin package from ZIP data
pub fn extract_plugin_package(package_data: &[u8]) -> Result<PluginPackage, ApiError> {
    debug!(
        "🔍 Extracting plugin package (size: {} bytes)",
        package_data.len()
    );

    let package_size = u64::try_from(package_data.len())
        .map_err(|_| ApiError::bad_request("Plugin package is too large".to_string()))?;
    if package_size > MAX_PLUGIN_PACKAGE_SIZE_BYTES {
        return Err(ApiError::bad_request(format!(
            "Plugin package is too large: {} bytes exceeds {} bytes",
            package_size, MAX_PLUGIN_PACKAGE_SIZE_BYTES
        )));
    }

    // Calculate package hash for integrity
    let package_hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(package_data);
        format!("{:x}", hasher.finalize())
    };

    // Create temporary directory for extraction
    let mut temp_guard = create_temp_extraction_dir(&package_hash)?;
    let temp_dir = temp_guard.path().to_path_buf();

    debug!(
        "📁 Extracting to temporary directory: {}",
        temp_dir.display()
    );

    // Open ZIP archive
    let cursor = Cursor::new(package_data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ApiError::bad_request(format!("Invalid ZIP archive: {}", e)))?;
    if archive.len() > MAX_PLUGIN_FILE_COUNT {
        return Err(ApiError::bad_request(format!(
            "Plugin package contains too many files: {} exceeds {}",
            archive.len(),
            MAX_PLUGIN_FILE_COUNT
        )));
    }

    // Track extracted files
    let mut manifest_data: Option<Vec<u8>> = None;
    let mut wasm_files: HashMap<String, Vec<u8>> = HashMap::new();
    let mut signature_data: Option<Vec<u8>> = None;
    let mut total_extracted_size = 0_u64;

    // Extract all files from ZIP to directory
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ApiError::bad_request(format!("Failed to read ZIP entry {}: {}", i, e)))?;

        let file_name = file.name().to_string();
        debug!("📄 Extracting file: {}", file_name);

        let relative_path = match sanitize_zip_entry_path(&file_name)? {
            Some(path) => path,
            None => {
                debug!("📄 Skipping directory or hidden file: {}", file_name);
                continue;
            }
        };
        let normalized_name = normalized_relative_path(&relative_path);

        // Skip directories and hidden files
        if normalized_name.is_empty() {
            debug!("📄 Skipping directory or hidden file: {}", file_name);
            continue;
        }

        let file_size = file.size();
        let file_size_limit = max_file_size_for_zip_entry(&normalized_name);
        if file_size > file_size_limit {
            return Err(ApiError::bad_request(format!(
                "Plugin package file '{}' is too large: {} bytes exceeds {} bytes",
                file_name, file_size, file_size_limit
            )));
        }

        total_extracted_size = total_extracted_size.checked_add(file_size).ok_or_else(|| {
            ApiError::bad_request("Plugin package expanded size overflow".to_string())
        })?;
        if total_extracted_size > MAX_PLUGIN_EXTRACTED_SIZE_BYTES {
            return Err(ApiError::bad_request(format!(
                "Plugin package expanded size is too large: {} bytes exceeds {} bytes",
                total_extracted_size, MAX_PLUGIN_EXTRACTED_SIZE_BYTES
            )));
        }

        // Create directory structure if needed
        let file_path = temp_dir.join(&relative_path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ApiError::internal(format!("Failed to create directory structure: {}", e))
            })?;
        }

        // Extract file
        let mut buffer = Vec::new();
        file.by_ref()
            .take(file_size_limit.saturating_add(1))
            .read_to_end(&mut buffer)
            .map_err(|e| {
                ApiError::bad_request(format!("Failed to read file '{}': {}", file_name, e))
            })?;
        let actual_size = u64::try_from(buffer.len())
            .map_err(|_| ApiError::bad_request("Plugin package file is too large".to_string()))?;
        if actual_size > file_size_limit {
            return Err(ApiError::bad_request(format!(
                "Plugin package file '{}' is too large: {} bytes exceeds {} bytes",
                file_name, actual_size, file_size_limit
            )));
        }

        // Write file to extraction directory
        std::fs::write(&file_path, &buffer).map_err(|e| {
            ApiError::internal(format!("Failed to write file '{}': {}", file_name, e))
        })?;

        // Process specific files for validation
        match normalized_name.as_str() {
            "plugin.toml" => {
                debug!("📋 Found plugin manifest");
                manifest_data = Some(buffer);
            }
            name if name.ends_with(".wasm") => {
                debug!("🔧 Found WASM file: {}", name);

                // Basic WASM validation
                if buffer.len() < 4 || &buffer[0..4] != b"\x00asm" {
                    return Err(ApiError::bad_request(format!(
                        "Invalid WASM file: {}",
                        name
                    )));
                }

                wasm_files.insert(normalized_name, buffer);
            }
            "signature" | "plugin.sig" => {
                debug!("🔐 Found signature file");
                signature_data = Some(buffer);
            }
            _ => {
                debug!("📄 Extracted supplemental file: {}", file_name);
            }
        }
    }

    // Validate required files
    let manifest_data = manifest_data
        .ok_or_else(|| ApiError::bad_request("Missing plugin.toml manifest file".to_string()))?;

    // Parse manifest
    let manifest_str = String::from_utf8(manifest_data.clone())
        .map_err(|e| ApiError::bad_request(format!("Invalid UTF-8 in plugin.toml: {}", e)))?;

    let mut manifest: PluginManifest = toml::from_str(&manifest_str)
        .map_err(|e| ApiError::bad_request(format!("Invalid TOML manifest: {}", e)))?;

    let declared_wasm_path =
        sanitize_zip_entry_path(&manifest.plugin.wasm_file)?.ok_or_else(|| {
            ApiError::bad_request("Manifest wasm_file must point to a file".to_string())
        })?;
    let declared_wasm_name = normalized_relative_path(&declared_wasm_path);
    let wasm_data = wasm_files.remove(&declared_wasm_name).ok_or_else(|| {
        let available_wasm_files: Vec<String> = wasm_files.keys().cloned().collect();
        ApiError::bad_request(format!(
            "Manifest declares WASM file '{}' but package contains {:?}",
            manifest.plugin.wasm_file, available_wasm_files
        ))
    })?;
    let signature_payload =
        build_signature_payload(&manifest_data, declared_wasm_name.as_bytes(), &wasm_data);
    manifest.plugin.wasm_file = declared_wasm_name;

    // Validate plugin name format
    if !is_valid_plugin_name(&manifest.plugin.name) {
        return Err(ApiError::bad_request(format!(
            "Invalid plugin name '{}': must be alphanumeric with hyphens/underscores",
            manifest.plugin.name
        )));
    }

    // Validate version format (basic semver check)
    if !is_valid_version(&manifest.plugin.version) {
        return Err(ApiError::bad_request(format!(
            "Invalid version '{}': must follow semantic versioning",
            manifest.plugin.version
        )));
    }

    info!(
        "✅ Successfully extracted plugin package: {} v{} to {}",
        manifest.plugin.name,
        manifest.plugin.version,
        temp_dir.display()
    );

    temp_guard.disarm();

    Ok(PluginPackage {
        manifest,
        wasm_data,
        signature_data,
        signature_payload,
        package_hash,
        package_size,
        extraction_path: temp_dir,
    })
}

/// Removes an extracted plugin package directory when validation or analysis exits early.
pub(super) struct TempExtractionGuard {
    path: PathBuf,
    active: bool,
}

impl TempExtractionGuard {
    /// Create a cleanup guard for an already-created extraction path.
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, active: true }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for TempExtractionGuard {
    fn drop(&mut self) {
        if !self.active || !self.path.exists() {
            return;
        }

        if let Err(error) = std::fs::remove_dir_all(&self.path) {
            warn!(
                "Failed to clean up plugin extraction directory '{}': {}",
                self.path.display(),
                error
            );
        }
    }
}

fn create_temp_extraction_dir(package_hash: &str) -> Result<TempExtractionGuard, ApiError> {
    let hash_prefix: String = package_hash.chars().take(16).collect();
    let counter = TEMP_EXTRACTION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let temp_dir = std::env::temp_dir().join(format!(
        "oxide_plugin_{}_{}_{}_{}",
        hash_prefix,
        std::process::id(),
        timestamp_nanos,
        counter
    ));

    std::fs::create_dir(&temp_dir).map_err(|e| {
        ApiError::internal(format!(
            "Failed to create plugin extraction directory '{}': {}",
            temp_dir.display(),
            e
        ))
    })?;

    Ok(TempExtractionGuard::new(temp_dir))
}

fn max_file_size_for_zip_entry(normalized_name: &str) -> u64 {
    match normalized_name {
        "plugin.toml" => MAX_PLUGIN_MANIFEST_SIZE_BYTES,
        "signature" | "plugin.sig" => MAX_PLUGIN_SIGNATURE_SIZE_BYTES,
        name if name.ends_with(".wasm") => MAX_PLUGIN_WASM_SIZE_BYTES,
        _ => MAX_PLUGIN_SUPPLEMENTAL_FILE_SIZE_BYTES,
    }
}

fn build_signature_payload(manifest_data: &[u8], wasm_path: &[u8], wasm_data: &[u8]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(
        SIGNATURE_PAYLOAD_MAGIC.len()
            + std::mem::size_of::<u64>() * 3
            + manifest_data.len()
            + wasm_path.len()
            + wasm_data.len(),
    );

    payload.extend_from_slice(SIGNATURE_PAYLOAD_MAGIC);
    append_len_prefixed_bytes(&mut payload, manifest_data);
    append_len_prefixed_bytes(&mut payload, wasm_path);
    append_len_prefixed_bytes(&mut payload, wasm_data);
    payload
}

fn append_len_prefixed_bytes(payload: &mut Vec<u8>, bytes: &[u8]) {
    payload.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    payload.extend_from_slice(bytes);
}

fn sanitize_zip_entry_path(file_name: &str) -> Result<Option<PathBuf>, ApiError> {
    if file_name.ends_with('/') || file_name.ends_with('\\') {
        return Ok(None);
    }

    let mut relative_path = PathBuf::new();
    let mut has_components = false;

    for component in Path::new(file_name).components() {
        match component {
            Component::Normal(part) => {
                if part.to_string_lossy().starts_with('.') {
                    return Ok(None);
                }
                relative_path.push(part);
                has_components = true;
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ApiError::bad_request(format!(
                    "Unsafe path in plugin ZIP entry: {}",
                    file_name
                )));
            }
        }
    }

    if has_components {
        Ok(Some(relative_path))
    } else {
        Ok(None)
    }
}

fn normalized_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// Verify digital signature of the plugin package
pub async fn verify_plugin_signature(package: &PluginPackage) -> Result<bool, ApiError> {
    // If no signature present, return false (unsigned)
    let signature_data = match &package.signature_data {
        Some(data) => data,
        None => {
            debug!(
                "No signature found for plugin: {}",
                package.manifest.plugin.name
            );
            return Ok(false);
        }
    };

    let trusted_keys = load_trusted_plugin_keys()?;
    if trusted_keys.is_empty() {
        warn!(
            "Digital signature present but no trusted Ed25519 public keys are configured for plugin: {}",
            package.manifest.plugin.name
        );
        return Ok(false);
    }

    let signature = decode_plugin_signature(signature_data)?;
    let verified = verify_detached_ed25519_signature(package, &signature, &trusted_keys);

    if verified {
        info!(
            "Verified Ed25519 package signature for plugin '{}'",
            package.manifest.plugin.name
        );
    } else {
        warn!(
            "Plugin package signature did not match any configured trusted key: {}",
            package.manifest.plugin.name
        );
    }

    Ok(verified)
}

struct TrustedPluginKey {
    verifying_key: VerifyingKey,
}

impl TrustedPluginKey {
    fn new(verifying_key: VerifyingKey) -> Self {
        Self { verifying_key }
    }
}

fn load_trusted_plugin_keys() -> Result<Vec<TrustedPluginKey>, ApiError> {
    let mut key_material = Vec::new();

    if let Ok(value) = std::env::var(TRUSTED_PLUGIN_KEYS_ENV) {
        key_material.extend(split_trusted_key_material(&value));
    }

    if let Ok(path) = std::env::var(TRUSTED_PLUGIN_KEYS_FILE_ENV) {
        let contents = std::fs::read_to_string(&path).map_err(|e| {
            ApiError::internal(format!(
                "Failed to read trusted plugin key file '{}': {}",
                path, e
            ))
        })?;
        key_material.extend(split_trusted_key_material(&contents));
    }

    let mut trusted_keys = Vec::new();
    for material in key_material {
        let key_bytes = decode_key_material(&material)
            .map_err(|e| ApiError::internal(format!("Invalid trusted plugin public key: {}", e)))?;
        let key_bytes: [u8; 32] = key_bytes.try_into().map_err(|bytes: Vec<u8>| {
            ApiError::internal(format!(
                "Invalid trusted plugin public key length: expected 32 bytes, got {}",
                bytes.len()
            ))
        })?;
        let verifying_key = VerifyingKey::from_bytes(&key_bytes)
            .map_err(|e| ApiError::internal(format!("Invalid trusted plugin public key: {}", e)))?;
        trusted_keys.push(TrustedPluginKey::new(verifying_key));
    }

    Ok(trusted_keys)
}

fn split_trusted_key_material(value: &str) -> Vec<String> {
    value
        .split(['\n', '\r', ',', ';'])
        .map(str::trim)
        .filter(|part| !part.is_empty() && !part.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect()
}

fn verify_detached_ed25519_signature(
    package: &PluginPackage,
    signature_bytes: &[u8; 64],
    trusted_keys: &[TrustedPluginKey],
) -> bool {
    let signature = Signature::from_bytes(signature_bytes);

    trusted_keys.iter().any(|key| {
        key.verifying_key
            .verify(&package.signature_payload, &signature)
            .is_ok()
    })
}

fn decode_plugin_signature(signature_data: &[u8]) -> Result<[u8; 64], ApiError> {
    if signature_data.len() == 64 {
        return signature_data
            .try_into()
            .map_err(|_| ApiError::bad_request("Invalid Ed25519 signature length".to_string()));
    }

    let signature_text = std::str::from_utf8(signature_data).map_err(|_| {
        ApiError::bad_request(
            "Plugin signature must be raw Ed25519 bytes or UTF-8 encoded key material".to_string(),
        )
    })?;
    let signature_material = extract_signature_material(signature_text)?;
    let signature_bytes = decode_key_material(&signature_material)
        .map_err(|e| ApiError::bad_request(format!("Invalid plugin signature encoding: {}", e)))?;

    signature_bytes.try_into().map_err(|bytes: Vec<u8>| {
        ApiError::bad_request(format!(
            "Invalid Ed25519 signature length: expected 64 bytes, got {}",
            bytes.len()
        ))
    })
}

fn extract_signature_material(signature_text: &str) -> Result<String, ApiError> {
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(signature_text) {
        if let Some(signature) = value.get("signature").and_then(|value| value.as_str()) {
            return Ok(signature.trim().to_string());
        }
    }

    for line in signature_text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((name, value)) = trimmed.split_once('=') {
            let name = name.trim().to_ascii_lowercase();
            if matches!(name.as_str(), "signature" | "sig" | "ed25519") {
                return Ok(value.trim().to_string());
            }
        } else {
            return Ok(trimmed.to_string());
        }
    }

    Err(ApiError::bad_request(
        "Plugin signature file does not contain signature material".to_string(),
    ))
}

fn decode_key_material(material: &str) -> Result<Vec<u8>, String> {
    let mut material = material.trim().trim_matches('"').trim_matches('\'');

    for prefix in ["ed25519:", "base64:", "hex:"] {
        if let Some(stripped) = material.strip_prefix(prefix) {
            material = stripped.trim();
            break;
        }
    }

    if material.len().is_multiple_of(2) && material.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return decode_hex_material(material);
    }

    STANDARD
        .decode(material)
        .or_else(|_| URL_SAFE.decode(material))
        .or_else(|_| URL_SAFE_NO_PAD.decode(material))
        .map_err(|e| e.to_string())
}

fn decode_hex_material(material: &str) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::with_capacity(material.len() / 2);
    let mut chars = material.as_bytes().chunks_exact(2);

    for pair in &mut chars {
        let high = hex_value(pair[0])?;
        let low = hex_value(pair[1])?;
        bytes.push((high << 4) | low);
    }

    if !chars.remainder().is_empty() {
        return Err("hex input must contain an even number of digits".to_string());
    }

    Ok(bytes)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err("hex input contains a non-hex digit".to_string()),
    }
}

fn enforce_plugin_signature_policy(
    plugin_name: &str,
    code_signing_required: bool,
    signature_valid: bool,
) -> Result<(), ApiError> {
    if !code_signing_required || signature_valid {
        return Ok(());
    }

    Err(ApiError::forbidden(format!(
        "Plugin '{}' requires a verified digital signature before installation",
        plugin_name
    )))
}

/// Validate plugin name format
fn is_valid_plugin_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }

    let Some(first_char) = name.chars().next() else {
        return false;
    };

    first_char.is_alphabetic()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// Validate version format (basic semver validation)
fn is_valid_version(version: &str) -> bool {
    if version.is_empty() {
        return false;
    }

    // Basic check for semver pattern (major.minor.patch)
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() < 2 || parts.len() > 4 {
        return false;
    }

    parts.iter().all(|part| {
        if part.is_empty() {
            return false;
        }
        // Allow numeric parts and pre-release identifiers
        part.chars().all(|c| c.is_alphanumeric() || c == '-')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use ed25519_dalek::{Signer, SigningKey};
    use std::io::{Cursor, Write};

    fn test_manifest(wasm_file: &str) -> String {
        format!(
            r#"
[plugin]
name = "signed_plugin"
version = "1.0.0"
description = "Test plugin"
author = "Test Author"
build_timestamp = "2026-01-01T00:00:00Z"
wasm_file = "{}"

[security]
required_capabilities = ["LogInfo"]
recommended_trust_level = "Untrusted"
"#,
            wasm_file
        )
    }

    fn test_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut archive = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();

        for (name, contents) in entries {
            archive.start_file(name, options).unwrap();
            archive.write_all(contents).unwrap();
        }

        archive.finish().unwrap().into_inner()
    }

    #[test]
    fn signature_policy_fails_closed_when_required() {
        let error = enforce_plugin_signature_policy("plugin", true, false).unwrap_err();

        assert_eq!(error.status_code(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn signature_policy_allows_verified_or_optional_signatures() {
        assert!(enforce_plugin_signature_policy("plugin", true, true).is_ok());
        assert!(enforce_plugin_signature_policy("plugin", false, false).is_ok());
    }

    #[test]
    fn extraction_normalizes_manifest_wasm_path_and_uses_unique_temp_dirs() {
        let manifest = test_manifest("./nested/./plugin.wasm");
        let wasm_data = b"\0asmtest plugin bytes";
        let package_data = test_zip(&[
            ("plugin.toml", manifest.as_bytes()),
            ("nested/plugin.wasm", wasm_data),
        ]);

        let package_a = extract_plugin_package(&package_data).unwrap();
        let _cleanup_a = TempExtractionGuard::new(package_a.extraction_path.clone());
        let package_b = extract_plugin_package(&package_data).unwrap();
        let _cleanup_b = TempExtractionGuard::new(package_b.extraction_path.clone());

        assert_eq!(package_a.manifest.plugin.wasm_file, "nested/plugin.wasm");
        assert_eq!(package_a.wasm_data, wasm_data);
        assert_ne!(package_a.extraction_path, package_b.extraction_path);
    }

    #[test]
    fn extraction_rejects_oversized_manifest() {
        let oversized_manifest = vec![b'a'; MAX_PLUGIN_MANIFEST_SIZE_BYTES as usize + 1];
        let package_data = test_zip(&[
            ("plugin.toml", oversized_manifest.as_slice()),
            ("plugin.wasm", b"\0asmtest plugin bytes"),
        ]);

        let error = extract_plugin_package(&package_data).unwrap_err();

        assert_eq!(error.status_code(), StatusCode::BAD_REQUEST);
    }

    fn test_plugin_package() -> PluginPackage {
        let manifest_data = br#"
[plugin]
name = "signed_plugin"
version = "1.0.0"
description = "Test plugin"
author = "Test Author"
build_timestamp = "2026-01-01T00:00:00Z"
wasm_file = "plugin.wasm"

[security]
required_capabilities = ["LogInfo"]
recommended_trust_level = "Untrusted"
"#;
        let wasm_data = b"\0asmtest plugin bytes".to_vec();

        PluginPackage {
            manifest: PluginManifest {
                plugin: PluginMetadata {
                    name: "signed_plugin".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Test plugin".to_string(),
                    author: "Test Author".to_string(),
                    homepage: None,
                    license: None,
                    min_oxide_version: None,
                    keywords: None,
                    categories: None,
                    changelog: None,
                    build_timestamp: "2026-01-01T00:00:00Z".to_string(),
                    wasm_file: "plugin.wasm".to_string(),
                },
                security: PluginSecurity {
                    required_capabilities: vec!["LogInfo".to_string()],
                    recommended_trust_level: "Untrusted".to_string(),
                    signature: None,
                    certificate_chain: None,
                    security_contact: None,
                    security_advisories: None,
                    audit_info: None,
                },
                dependencies: None,
                config: None,
            },
            wasm_data: wasm_data.clone(),
            signature_data: None,
            signature_payload: build_signature_payload(manifest_data, b"plugin.wasm", &wasm_data),
            package_hash: "hash".to_string(),
            package_size: 128,
            extraction_path: PathBuf::from("test"),
        }
    }

    #[test]
    fn ed25519_signature_verifies_against_trusted_key() {
        let package = test_plugin_package();
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let signature = signing_key.sign(&package.signature_payload);
        let trusted_key = TrustedPluginKey::new(signing_key.verifying_key());

        assert!(verify_detached_ed25519_signature(
            &package,
            &signature.to_bytes(),
            &[trusted_key]
        ));
    }

    #[test]
    fn ed25519_signature_rejects_tampered_payload() {
        let mut package = test_plugin_package();
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let signature = signing_key.sign(&package.signature_payload);
        package.signature_payload.push(b'!');
        let trusted_key = TrustedPluginKey::new(signing_key.verifying_key());

        assert!(!verify_detached_ed25519_signature(
            &package,
            &signature.to_bytes(),
            &[trusted_key]
        ));
    }
}
