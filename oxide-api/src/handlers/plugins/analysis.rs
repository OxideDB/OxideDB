//! Plugin package analysis and validation

use axum::{
    extract::{State, Multipart},
    Json,
};
use tracing::{debug, info};

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
};
use super::{
    types::*,
    installation::{extract_plugin_package, verify_plugin_signature},
    capabilities::{parse_capability_string, parse_trust_level_string},
};
use oxide_core::plugin_security::PluginTrustLevel;

/// Analyze a plugin package to extract metadata and capabilities before installation
pub async fn analyze_plugin(
    State(_state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ApiResponse<PluginAnalysisResult>>, ApiError> {
    debug!("🔍 Analyzing plugin package for capability information");

    let mut package_data: Option<Vec<u8>> = None;

    // Parse multipart form data for plugin package
    while let Some(field) = multipart.next_field().await
        .map_err(|e| ApiError::bad_request(format!("Error parsing multipart field: {}", e)))?
    {
        let field_name = field.name().unwrap_or("unknown");
        
        if field_name == "plugin_package" {
            package_data = Some(field.bytes().await
                .map_err(|e| ApiError::bad_request(format!("Error reading plugin package: {}", e)))?
                .to_vec());
            break;
        }
    }
    
    let package_data = package_data
        .ok_or_else(|| ApiError::bad_request("Missing plugin_package field".to_string()))?;

    // Extract and validate the plugin package WITHOUT loading WASM
    debug!("🔍 Extracting plugin package for analysis");
    let package = extract_plugin_package(&package_data)?;

    // Verify digital signature if present
    let signature_valid = verify_plugin_signature(&package).await?;

    // Parse declared capabilities from manifest
    let declared_capabilities: Vec<String> = package.manifest.security.required_capabilities.clone();

    // Parse recommended trust level from manifest
    let recommended_trust_level = package.manifest.security.recommended_trust_level.clone();

    let mut warnings = Vec::new();
    let mut errors = Vec::new();

    // Validate capabilities format
    for cap_str in &declared_capabilities {
        if parse_capability_string(cap_str).is_err() {
            warnings.push(format!("Unknown or invalid capability: {}", cap_str));
        }
    }

    // Validate trust level format
    if parse_trust_level_string(&recommended_trust_level).is_err() {
        warnings.push(format!("Invalid recommended trust level: {}", recommended_trust_level));
    }

    // Check for potential security concerns
    let sensitive_capabilities = [
        "DeleteRecords", "ModifyEventData", "BlockOperations", "HandleHttpRequests"
    ];
    let has_sensitive_caps: Vec<&String> = declared_capabilities.iter()
        .filter(|cap| sensitive_capabilities.iter().any(|sensitive| cap.contains(sensitive)))
        .collect();

    if !has_sensitive_caps.is_empty() {
        warnings.push(format!("Plugin requests sensitive capabilities: {:?}", has_sensitive_caps));
    }

    // Check signature status
    if !signature_valid && package.signature_data.is_some() {
        warnings.push("Plugin signature verification failed".to_string());
    } else if package.signature_data.is_none() {
        warnings.push("Plugin is not digitally signed".to_string());
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
            capabilities: declared_capabilities.iter()
                .filter_map(|cap_str| parse_capability_string(cap_str).ok())
                .collect(),
            trust_level: parse_trust_level_string(&recommended_trust_level)
                .unwrap_or(PluginTrustLevel::Untrusted),
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
            security_advisories: package.manifest.security.security_advisories.clone().unwrap_or_default(),
            audit_info: package.manifest.security.audit_info.clone().map(|audit| 
                oxide_plugin_sdk::PluginAuditInfo {
                    audit_date: audit.audit_date,
                    auditor: audit.auditor,
                    report_url: audit.report_url,
                    status: audit.status,
                }
            ),
        },
        size_bytes: package.package_size,
        warnings,
        errors,
    };

    info!("✅ Plugin package analysis completed: {} v{}", 
          package.manifest.plugin.name, package.manifest.plugin.version);

    Ok(Json(ApiResponse::success(analysis)))
} 