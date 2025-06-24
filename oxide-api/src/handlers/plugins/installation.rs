//! Plugin installation and package management

use axum::{
    extract::{State, Multipart},
    Json,
};
use std::io::{Read, Cursor};
use tracing::{debug, info, warn, error};
use zip::ZipArchive;

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
};
use super::{
    types::*,
    capabilities::{parse_capability_string, parse_trust_level_string},
};
use oxide_core::plugin_security::{PluginCapability, PluginTrustLevel, ResourceLimits};

/// Register/Install a new plugin from a ZIP package
pub async fn register_plugin(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<PluginInfo>>, ApiError> {
    debug!("🔌 Starting plugin registration from ZIP package");
    
    let plugin_manager = state.plugin_manager.as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    let mut package_data: Option<Vec<u8>> = None;
    let mut user_trust_level = PluginTrustLevel::Untrusted;
    let mut user_capabilities: Vec<PluginCapability> = Vec::new();

    // Parse multipart form data
    debug!("🔌 Parsing multipart form data");
    while let Some(field) = multipart.next_field().await
        .map_err(|e| {
            error!("Multipart field parsing error: {}", e);
            ApiError::bad_request(format!("Error parsing multipart field: {}", e))
        })? 
    {
        let field_name = field.name().unwrap_or("unknown");
        debug!("🔌 Processing field: {}", field_name);
        
        match field_name {
            "plugin_package" => {
                debug!("🔌 Reading plugin package");
                let file_name = field.file_name().unwrap_or("plugin.zip");
                debug!("🔌 Package file name: {}", file_name);
                
                package_data = Some(field.bytes().await
                    .map_err(|e| {
                        error!("Error reading plugin package: {}", e);
                        ApiError::bad_request(format!("Error reading plugin package: {}", e))
                    })?
                    .to_vec());
                
                if let Some(ref data) = package_data {
                    debug!("🔌 Package size: {} bytes", data.len());
                }
            }
            "trust_level" => {
                let trust_str = field.text().await
                    .map_err(|e| {
                        error!("Error reading trust_level field: {}", e);
                        ApiError::bad_request(format!("Invalid trust level: {}", e))
                    })?;
                debug!("🔌 Trust level string: {}", trust_str);
                user_trust_level = serde_json::from_str(&format!("\"{}\"", trust_str))
                    .map_err(|e| {
                        error!("Error parsing trust level '{}': {}", trust_str, e);
                        ApiError::bad_request(format!("Invalid trust level format: {}", e))
                    })?;
                debug!("🔌 Trust level: {:?}", user_trust_level);
            }
            "capabilities" => {
                let caps_str = field.text().await
                    .map_err(|e| {
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
    
    // Verify digital signature if present
    let signature_valid = verify_plugin_signature(&package).await?;
    if signature_valid {
        info!("✅ Plugin signature verified: {}", package.manifest.plugin.name);
    } else {
        warn!("⚠️  Plugin signature not verified (may be unsigned): {}", package.manifest.plugin.name);
    }

    let plugin_name = package.manifest.plugin.name.clone();
    let plugin_version = package.manifest.plugin.version.clone();

    // Parse declared capabilities from manifest
    let declared_capabilities: Vec<PluginCapability> = package.manifest.security.required_capabilities
        .iter()
        .filter_map(|cap_str| parse_capability_string(cap_str).ok())
        .collect();

    // Parse recommended trust level from manifest (for future use)
    let _recommended_trust_level = parse_trust_level_string(&package.manifest.security.recommended_trust_level)
        .unwrap_or(PluginTrustLevel::Untrusted);

    // If user didn't specify capabilities, use declared ones (but still require explicit trust)
    let final_capabilities = if user_capabilities.is_empty() {
        declared_capabilities.clone()
    } else {
        user_capabilities
    };

    // Validate that user-granted capabilities include all required ones
    let missing_capabilities: Vec<&PluginCapability> = declared_capabilities.iter()
        .filter(|req_cap| !final_capabilities.iter().any(|granted| granted == *req_cap))
        .collect();

    if !missing_capabilities.is_empty() {
        return Err(ApiError::bad_request(format!(
            "Plugin '{}' requires capabilities that were not granted: {:?}. Please grant these capabilities or contact the plugin author.",
            plugin_name, missing_capabilities
        )));
    }

    // Check for potentially dangerous capabilities being granted
    let sensitive_capabilities = [
        "DeleteRecords", "ModifyEventData", "BlockOperations", "HttpRequest"
    ];
    let granted_sensitive: Vec<String> = final_capabilities.iter()
        .map(|cap| format!("{:?}", cap))
        .filter(|cap_str| {
            sensitive_capabilities.iter().any(|sensitive| cap_str.contains(sensitive))
        })
        .collect();

    if !granted_sensitive.is_empty() {
        info!("⚠️  Plugin '{}' granted sensitive capabilities: {:?}", plugin_name, granted_sensitive);
    }

    // Install plugin using directory-based approach
    let _record_id = state.plugin_config_service.install_plugin_from_directory(
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
    ).await
        .map_err(|e| ApiError::internal(format!("Failed to install plugin: {}", e)))?;

    // Load the plugin into runtime (NO METADATA EXTRACTION FROM WASM)
    {
        let mut runtime_guard = plugin_manager.runtime.lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;
        
        runtime_guard.load_plugin_with_trust(
            &plugin_name,
            &package.wasm_data,
            user_trust_level.clone(),
            final_capabilities.clone(),
            ResourceLimits::default(),
        ).map_err(|e| ApiError::internal(format!("Failed to load plugin: {}", e)))?;
    }

    info!("✅ Plugin '{}' v{} registered successfully from ZIP package", plugin_name, plugin_version);

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
    debug!("🔍 Extracting plugin package (size: {} bytes)", package_data.len());

    // Calculate package hash for integrity
    let package_hash = {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(package_data);
        format!("{:x}", hasher.finalize())
    };

    // Create temporary directory for extraction
    let temp_dir = std::env::temp_dir().join(format!("oxide_plugin_{}", package_hash));
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| ApiError::internal(format!("Failed to create extraction directory: {}", e)))?;

    debug!("📁 Extracting to temporary directory: {}", temp_dir.display());

    // Open ZIP archive
    let cursor = Cursor::new(package_data);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| ApiError::bad_request(format!("Invalid ZIP archive: {}", e)))?;

    // Track extracted files
    let mut manifest_data: Option<Vec<u8>> = None;
    let mut wasm_data: Option<Vec<u8>> = None;
    let mut signature_data: Option<Vec<u8>> = None;
    let mut wasm_filename: Option<String> = None;

    // Extract all files from ZIP to directory
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)
            .map_err(|e| ApiError::bad_request(format!("Failed to read ZIP entry {}: {}", i, e)))?;
        
        let file_name = file.name().to_string();
        debug!("📄 Extracting file: {}", file_name);

        // Skip directories and hidden files
        if file_name.ends_with('/') || file_name.starts_with('.') {
            debug!("📄 Skipping directory or hidden file: {}", file_name);
            continue;
        }

        // Create directory structure if needed
        let file_path = temp_dir.join(&file_name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| ApiError::internal(format!("Failed to create directory structure: {}", e)))?;
        }

        // Extract file
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .map_err(|e| ApiError::bad_request(format!("Failed to read file '{}': {}", file_name, e)))?;

        // Write file to extraction directory
        std::fs::write(&file_path, &buffer)
            .map_err(|e| ApiError::internal(format!("Failed to write file '{}': {}", file_name, e)))?;

        // Process specific files for validation
        match file_name.as_str() {
            "plugin.toml" => {
                debug!("📋 Found plugin manifest");
                manifest_data = Some(buffer);
            }
            name if name.ends_with(".wasm") => {
                debug!("🔧 Found WASM file: {}", name);
                
                // Basic WASM validation
                if buffer.len() < 4 || &buffer[0..4] != b"\x00asm" {
                    return Err(ApiError::bad_request(format!("Invalid WASM file: {}", name)));
                }
                
                wasm_data = Some(buffer);
                wasm_filename = Some(name.to_string());
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
    
    let wasm_data = wasm_data
        .ok_or_else(|| ApiError::bad_request("Missing WASM file in package".to_string()))?;

    // Parse manifest
    let manifest_str = String::from_utf8(manifest_data)
        .map_err(|e| ApiError::bad_request(format!("Invalid UTF-8 in plugin.toml: {}", e)))?;
    
    let manifest: PluginManifest = toml::from_str(&manifest_str)
        .map_err(|e| ApiError::bad_request(format!("Invalid TOML manifest: {}", e)))?;

    // Validate manifest consistency
    if let Some(ref declared_wasm) = wasm_filename {
        if manifest.plugin.wasm_file != *declared_wasm {
            warn!("WASM filename mismatch: manifest declares '{}' but found '{}'", 
                  manifest.plugin.wasm_file, declared_wasm);
        }
    }

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

    info!("✅ Successfully extracted plugin package: {} v{} to {}", 
          manifest.plugin.name, manifest.plugin.version, temp_dir.display());

    Ok(PluginPackage {
        manifest,
        wasm_data,
        signature_data,
        package_hash,
        package_size: package_data.len() as u64,
        extraction_path: temp_dir,
    })
}

/// Verify digital signature of the plugin package
pub async fn verify_plugin_signature(package: &PluginPackage) -> Result<bool, ApiError> {
    // If no signature present, return false (unsigned)
    let _signature_data = match &package.signature_data {
        Some(data) => data,
        None => {
            debug!("No signature found for plugin: {}", package.manifest.plugin.name);
            return Ok(false);
        }
    };

    // TODO: Implement actual signature verification
    // This would involve:
    // 1. Loading trusted public keys/certificates
    // 2. Verifying the signature against the package hash
    // 3. Checking certificate chain validity
    // 4. Ensuring the signer is trusted
    
    warn!("🚧 Digital signature verification not yet implemented for plugin: {}", 
          package.manifest.plugin.name);
    
    // For now, return false to indicate signature verification is not working
    Ok(false)
}

/// Validate plugin name format
fn is_valid_plugin_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    
    name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') 
        && name.chars().next().unwrap().is_alphabetic()
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