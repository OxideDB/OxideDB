//! Plugin package analysis and validation

use axum::{
    extract::{Multipart, State},
    Json,
};
use tracing::{debug, info};

use super::{
    capabilities::{
        minimum_trust_level_for_capabilities, parse_capability_string, parse_trust_level_string,
        trust_level_rank,
    },
    installation::{extract_plugin_package, verify_plugin_signature, TempExtractionGuard},
    types::*,
};
use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};

/// Analyze a plugin package to extract metadata and capabilities before installation
pub async fn analyze_plugin(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<PluginAnalysisResult>>, ApiError> {
    super::ensure_plugin_superuser(&authenticated_user, "analyze plugin packages")?;

    debug!("🔍 Analyzing plugin package for capability information");

    let mut package_data: Option<Vec<u8>> = None;

    // Parse multipart form data for plugin package
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(format!("Error parsing multipart field: {}", e)))?
    {
        let field_name = field.name().unwrap_or("unknown");

        if field_name == "plugin_package" {
            package_data = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| {
                        ApiError::bad_request(format!("Error reading plugin package: {}", e))
                    })?
                    .to_vec(),
            );
            break;
        }
    }

    let package_data = package_data
        .ok_or_else(|| ApiError::bad_request("Missing plugin_package field".to_string()))?;

    // Extract and validate the plugin package WITHOUT loading WASM
    debug!("🔍 Extracting plugin package for analysis");
    let package = extract_plugin_package(&package_data)?;
    let _extraction_cleanup = TempExtractionGuard::new(package.extraction_path.clone());

    // Verify digital signature if present
    let signature_valid = verify_plugin_signature(&package).await?;
    let code_signing_required = state
        .plugin_manager
        .as_ref()
        .is_some_and(|manager| manager.requires_code_signing());

    // Parse declared capabilities from manifest
    let declared_capabilities: Vec<String> =
        package.manifest.security.required_capabilities.clone();

    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Validate capabilities format
    let mut parsed_capabilities = Vec::new();
    for cap_str in &declared_capabilities {
        match parse_capability_string(cap_str) {
            Ok(capability) => parsed_capabilities.push(capability),
            Err(e) => errors.push(format!("Invalid capability '{}': {}", cap_str, e)),
        }
    }

    // Validate trust level format
    let manifest_recommended_trust_level =
        package.manifest.security.recommended_trust_level.clone();
    let required_trust_level = minimum_trust_level_for_capabilities(&parsed_capabilities);
    let effective_recommended_trust_level =
        match parse_trust_level_string(&manifest_recommended_trust_level) {
            Ok(manifest_trust_level) => {
                if trust_level_rank(&manifest_trust_level) < trust_level_rank(&required_trust_level)
                {
                    warnings.push(format!(
                        "Manifest recommends {:?}, but declared capabilities require at least {:?}",
                        manifest_trust_level, required_trust_level
                    ));
                    required_trust_level.clone()
                } else {
                    manifest_trust_level
                }
            }
            Err(_) => {
                warnings.push(format!(
                    "Invalid recommended trust level: {}",
                    manifest_recommended_trust_level
                ));
                required_trust_level.clone()
            }
        };
    let recommended_trust_level = format!("{:?}", effective_recommended_trust_level);

    // Check for potential security concerns
    let sensitive_capabilities = [
        "DeleteRecords",
        "ModifyEventData",
        "BlockOperations",
        "AccessVfs",
        "HandleHttpRequests",
        "ManageCollections",
    ];
    let has_sensitive_caps: Vec<&String> = declared_capabilities
        .iter()
        .filter(|cap| {
            sensitive_capabilities
                .iter()
                .any(|sensitive| cap.contains(sensitive))
        })
        .collect();

    if !has_sensitive_caps.is_empty() {
        warnings.push(format!(
            "Plugin requests sensitive capabilities: {:?}",
            has_sensitive_caps
        ));
    }

    // Check signature status
    if !signature_valid && package.signature_data.is_some() {
        warnings.push("Plugin signature verification failed".to_string());
    } else if package.signature_data.is_none() {
        warnings.push("Plugin is not digitally signed".to_string());
    }

    if code_signing_required && !signature_valid {
        errors.push(
            "Server policy requires a verified plugin signature before installation".to_string(),
        );
    }

    // Build analysis result
    let analysis = PluginAnalysisResult {
        is_valid: errors.is_empty(),
        plugin_info: Some(PluginInfo {
            name: package.manifest.plugin.name.clone(),
            status: PluginStatus::Loading,
            version: package.manifest.plugin.version.clone(),
            description: package.manifest.plugin.description.clone(),
            author: package.manifest.plugin.author.clone(),
            capabilities: parsed_capabilities,
            trust_level: effective_recommended_trust_level,
            routes: vec![], // Routes not available until actual installation
            executions: 0,
            errors: 0,
            last_execution: None,
            resource_usage: ResourceUsageInfo::default(),
        }),
        declared_capabilities,
        recommended_trust_level: Some(recommended_trust_level),
        security_info: PluginSecurityInfo {
            binary_hash: package.package_hash,
            hash_algorithm: "SHA-256".to_string(),
            signature_valid,
            security_advisories: package
                .manifest
                .security
                .security_advisories
                .clone()
                .unwrap_or_default(),
            audit_info: package.manifest.security.audit_info.clone().map(|audit| {
                oxide_plugin_sdk::PluginAuditInfo {
                    audit_date: audit.audit_date,
                    auditor: audit.auditor,
                    report_url: audit.report_url,
                    status: audit.status,
                }
            }),
        },
        size_bytes: package.package_size,
        warnings,
        errors,
    };

    info!(
        "✅ Plugin package analysis completed: {} v{}",
        package.manifest.plugin.name, package.manifest.plugin.version
    );

    Ok(Json(ApiResponse::success(analysis)))
}
