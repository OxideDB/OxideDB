//! Plugin capabilities and trust level management

use axum::{
    extract::{Path, State},
    Json,
};
use tracing::{info, warn};

use super::types::*;
use crate::{
    errors::ApiError, extractors::AuthenticatedUser, responses::ApiResponse, server::AppState,
};
use oxide_core::{
    auth::CrudOperation,
    plugin_security::{PluginCapability, PluginTrustLevel, VfsOperation},
};
use serde_json::Value;
use std::collections::HashMap;

/// Grant a capability to a plugin
pub async fn grant_plugin_capability(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Path((plugin_name, capability_name)): Path<(String, String)>,
    Json(capability_config): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<Vec<PluginCapability>>>, ApiError> {
    super::ensure_plugin_superuser(&authenticated_user, "grant plugin capabilities")?;

    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Parse capability from name and config
    let capability = parse_capability_from_request(&capability_name, capability_config)?;

    // Update database configuration
    state
        .plugin_config_service
        .add_plugin_capability(&plugin_name, capability.clone())
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    // Grant capability in runtime when the plugin is currently loaded. If it is
    // disabled, the persisted configuration will be applied on the next load.
    if is_plugin_loaded(plugin_manager, &plugin_name)? {
        let mut runtime_guard = plugin_manager
            .runtime
            .lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;

        runtime_guard
            .grant_plugin_capability(&plugin_name, capability.clone())
            .map_err(|e| ApiError::internal(format!("Failed to grant capability: {}", e)))?;
    } else {
        info!(
            "Saved capability {:?} for unloaded plugin '{}'",
            capability, plugin_name
        );
    }

    // Get updated capabilities from database
    let config = state
        .plugin_config_service
        .get_plugin_config(&plugin_name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;

    info!(
        "✅ Granted capability {:?} to plugin '{}' and saved to database",
        capability, plugin_name
    );

    Ok(Json(ApiResponse::success(config.capabilities)))
}

/// Revoke a capability from a plugin
pub async fn revoke_plugin_capability(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Path((plugin_name, capability_name)): Path<(String, String)>,
    Json(capability_config): Json<serde_json::Value>,
) -> Result<Json<ApiResponse<Vec<PluginCapability>>>, ApiError> {
    super::ensure_plugin_superuser(&authenticated_user, "revoke plugin capabilities")?;

    let plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Parse capability from name and config
    let capability = parse_capability_from_request(&capability_name, capability_config)?;

    // Update database configuration
    state
        .plugin_config_service
        .remove_plugin_capability(&plugin_name, &capability)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to update plugin configuration: {}", e)))?;

    // Revoke capability in runtime when the plugin is currently loaded. If it is
    // disabled, the persisted configuration is already the source of truth.
    if is_plugin_loaded(plugin_manager, &plugin_name)? {
        let mut runtime_guard = plugin_manager
            .runtime
            .lock()
            .map_err(|_| ApiError::internal("Failed to acquire plugin runtime lock".to_string()))?;

        runtime_guard
            .revoke_plugin_capability(&plugin_name, &capability)
            .map_err(|e| ApiError::internal(format!("Failed to revoke capability: {}", e)))?;
    } else {
        info!(
            "Removed capability {:?} for unloaded plugin '{}'",
            capability, plugin_name
        );
    }

    // Get updated capabilities from database
    let config = state
        .plugin_config_service
        .get_plugin_config(&plugin_name)
        .await
        .map_err(|e| ApiError::internal(format!("Failed to get plugin configuration: {}", e)))?;

    info!(
        "✅ Revoked capability {:?} from plugin '{}' and saved to database",
        capability, plugin_name
    );

    Ok(Json(ApiResponse::success(config.capabilities)))
}

/// Update plugin trust level
pub async fn update_plugin_trust_level(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Path(plugin_name): Path<String>,
    Json(_request): Json<UpdateTrustLevelRequest>,
) -> Result<Json<ApiResponse<PluginTrustLevel>>, ApiError> {
    super::ensure_plugin_superuser(&authenticated_user, "update plugin trust levels")?;

    let _plugin_manager = state
        .plugin_manager
        .as_ref()
        .ok_or_else(|| ApiError::internal("Plugin system not available".to_string()))?;

    // Note: Currently the plugin runtime doesn't support changing trust levels after loading
    // This would require reloading the plugin with new trust level
    warn!("Trust level update requested for plugin '{}', but runtime doesn't support dynamic trust level changes", plugin_name);

    Err(ApiError::bad_request("Dynamic trust level updates not yet supported. Please unregister and re-register the plugin with the new trust level.".to_string()))
}

/// Parse a capability from request data
fn parse_capability_from_request(
    capability_name: &str,
    config: serde_json::Value,
) -> Result<PluginCapability, ApiError> {
    match capability_name {
        "LogInfo" => Ok(PluginCapability::LogInfo),
        "LogError" => Ok(PluginCapability::LogError),
        "ReadEventData" => Ok(PluginCapability::ReadEventData),
        "ModifyEventData" => Ok(PluginCapability::ModifyEventData),
        "BlockOperations" => Ok(PluginCapability::BlockOperations),
        "ScheduleTasks" => Ok(PluginCapability::ScheduleTasks),
        "ReadConfig" => Ok(PluginCapability::ReadConfig {
            keys: json_string_array(&config, "keys", vec!["*".to_string()])?,
        }),
        "AccessCollection" => {
            let collection = config
                .get("collection")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    ApiError::bad_request(
                        "Missing collection field for AccessCollection capability".to_string(),
                    )
                })?;
            let operations = config
                .get("operations")
                .and_then(value_to_string_vec)
                .map(|operations| {
                    operations
                        .iter()
                        .map(|operation| parse_crud_operation(operation))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_else(|| vec![CrudOperation::Read]);
            Ok(PluginCapability::AccessCollection {
                collection: collection.to_string(),
                operations,
            })
        }
        "HttpRequest" => Ok(PluginCapability::HttpRequest {
            allowed_urls: json_string_array(&config, "allowed_urls", vec!["*".to_string()])?,
            rate_limit: json_u32(&config, "rate_limit", 60)?,
        }),
        "PersistentStorage" => Ok(PluginCapability::PersistentStorage {
            max_size: json_u64(&config, "max_size", 1024 * 1024)?,
            key_prefixes: json_string_array(&config, "key_prefixes", vec!["plugin_*".to_string()])?,
        }),
        "EmitEvents" => Ok(PluginCapability::EmitEvents {
            event_types: json_string_array(&config, "event_types", vec!["custom.*".to_string()])?,
        }),
        "AccessVfs" => Ok(PluginCapability::AccessVfs {
            namespaces: json_string_array(&config, "namespaces", vec!["*".to_string()])?,
            operations: vfs_operations_from_strings(json_string_array(
                &config,
                "operations",
                vec!["Read".to_string(), "List".to_string(), "Usage".to_string()],
            )?)?,
        }),
        "RegisterHttpRoutes" => {
            let path_patterns = config
                .get("path_patterns")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_else(|| vec!["*".to_string()]);
            let methods = config
                .get("methods")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_else(|| vec!["GET".to_string(), "POST".to_string()]);
            Ok(PluginCapability::RegisterHttpRoutes {
                path_patterns,
                methods: normalize_methods(methods),
            })
        }
        "HandleHttpRequests" => Ok(PluginCapability::HandleHttpRequests),
        "CreateRecords" => Ok(PluginCapability::CreateRecords {
            collections: json_string_array(&config, "collections", vec!["*".to_string()])?,
        }),
        "ReadRecords" => Ok(PluginCapability::ReadRecords {
            collections: json_string_array(&config, "collections", vec!["*".to_string()])?,
        }),
        "UpdateRecords" => Ok(PluginCapability::UpdateRecords {
            collections: json_string_array(&config, "collections", vec!["*".to_string()])?,
        }),
        "DeleteRecords" => Ok(PluginCapability::DeleteRecords {
            collections: json_string_array(&config, "collections", vec!["*".to_string()])?,
        }),
        _ => Err(ApiError::bad_request(format!(
            "Unknown capability: {}",
            capability_name
        ))),
    }
}

/// Parse a capability string into a PluginCapability enum
pub fn parse_capability_string(cap_str: &str) -> Result<PluginCapability, ApiError> {
    let capability_string = cap_str.trim();

    if capability_string.is_empty() {
        return Err(ApiError::bad_request(
            "Capability string cannot be empty".to_string(),
        ));
    }

    if let Ok(capability) = serde_json::from_str::<PluginCapability>(capability_string) {
        return Ok(capability);
    }

    let (capability_name, args) = parse_capability_invocation(capability_string)?;

    match capability_name.as_str() {
        // Simple unit variants
        "LogInfo" => Ok(PluginCapability::LogInfo),
        "LogError" => Ok(PluginCapability::LogError),
        "ReadEventData" => Ok(PluginCapability::ReadEventData),
        "ModifyEventData" => Ok(PluginCapability::ModifyEventData),
        "BlockOperations" => Ok(PluginCapability::BlockOperations),
        "HandleHttpRequests" => Ok(PluginCapability::HandleHttpRequests),
        "ScheduleTasks" => Ok(PluginCapability::ScheduleTasks),

        // Struct variants with default values
        "AccessCollection" => Ok(PluginCapability::AccessCollection {
            collection: string_arg(&args, "collection", "*".to_string())?,
            operations: crud_operations_arg(&args, "operations", vec![CrudOperation::Read])?,
        }),
        "RegisterHttpRoutes" => Ok(PluginCapability::RegisterHttpRoutes {
            path_patterns: string_vec_arg(&args, "path_patterns", vec!["*".to_string()])?,
            methods: normalize_methods(string_vec_arg(
                &args,
                "methods",
                vec!["GET".to_string(), "POST".to_string()],
            )?),
        }),
        "CreateRecords" => Ok(PluginCapability::CreateRecords {
            collections: string_vec_arg(&args, "collections", vec!["*".to_string()])?,
        }),
        "ReadRecords" => Ok(PluginCapability::ReadRecords {
            collections: string_vec_arg(&args, "collections", vec!["*".to_string()])?,
        }),
        "UpdateRecords" => Ok(PluginCapability::UpdateRecords {
            collections: string_vec_arg(&args, "collections", vec!["*".to_string()])?,
        }),
        "DeleteRecords" => Ok(PluginCapability::DeleteRecords {
            collections: string_vec_arg(&args, "collections", vec!["*".to_string()])?,
        }),
        "ReadConfig" => Ok(PluginCapability::ReadConfig {
            keys: string_vec_arg(&args, "keys", vec!["*".to_string()])?,
        }),
        "HttpRequest" => Ok(PluginCapability::HttpRequest {
            allowed_urls: string_vec_arg(&args, "allowed_urls", vec!["*".to_string()])?,
            rate_limit: u32_arg(&args, "rate_limit", 60)?,
        }),
        "PersistentStorage" => Ok(PluginCapability::PersistentStorage {
            max_size: u64_arg(&args, "max_size", 1024 * 1024)?,
            key_prefixes: string_vec_arg(&args, "key_prefixes", vec!["plugin_*".to_string()])?,
        }),
        "EmitEvents" => Ok(PluginCapability::EmitEvents {
            event_types: string_vec_arg(&args, "event_types", vec!["custom.*".to_string()])?,
        }),
        "AccessVfs" => Ok(PluginCapability::AccessVfs {
            namespaces: string_vec_arg(&args, "namespaces", vec!["*".to_string()])?,
            operations: vfs_operations_arg(
                &args,
                "operations",
                vec![VfsOperation::Read, VfsOperation::List, VfsOperation::Usage],
            )?,
        }),
        _ => Err(ApiError::bad_request(format!(
            "Unknown capability: {}",
            cap_str
        ))),
    }
}

pub fn parse_capability_strings(
    capability_strings: &[String],
) -> Result<Vec<PluginCapability>, ApiError> {
    capability_strings
        .iter()
        .map(|capability| parse_capability_string(capability))
        .collect()
}

pub fn capability_satisfies(granted: &PluginCapability, required: &PluginCapability) -> bool {
    granted.grants(required)
}

type CapabilityArgs = HashMap<String, Value>;

fn parse_capability_invocation(input: &str) -> Result<(String, CapabilityArgs), ApiError> {
    let Some(open_paren) = input.find('(') else {
        return Ok((input.trim().to_string(), CapabilityArgs::new()));
    };

    if !input.ends_with(')') {
        return Err(ApiError::bad_request(format!(
            "Invalid capability syntax '{}': missing closing ')'",
            input
        )));
    }

    let capability_name = input[..open_paren].trim();
    if capability_name.is_empty() {
        return Err(ApiError::bad_request(
            "Capability name cannot be empty".to_string(),
        ));
    }

    let args = parse_capability_args(&input[open_paren + 1..input.len() - 1])?;
    Ok((capability_name.to_string(), args))
}

fn parse_capability_args(input: &str) -> Result<CapabilityArgs, ApiError> {
    let mut args = CapabilityArgs::new();

    if input.trim().is_empty() {
        return Ok(args);
    }

    for segment in split_top_level(input, ',') {
        let Some((key, value)) = split_once_top_level(&segment, '=') else {
            return Err(ApiError::bad_request(format!(
                "Invalid capability argument '{}': expected key=value",
                segment
            )));
        };

        let key = key.trim();
        if key.is_empty() {
            return Err(ApiError::bad_request(
                "Capability argument key cannot be empty".to_string(),
            ));
        }

        args.insert(key.to_string(), parse_capability_arg_value(value.trim())?);
    }

    Ok(args)
}

fn parse_capability_arg_value(value: &str) -> Result<Value, ApiError> {
    if let Ok(json_value) = serde_json::from_str::<Value>(value) {
        return Ok(json_value);
    }

    if value.starts_with('[') && value.ends_with(']') {
        let inner = &value[1..value.len() - 1];
        if inner.trim().is_empty() {
            return Ok(Value::Array(Vec::new()));
        }

        return split_top_level(inner, ',')
            .into_iter()
            .map(|item| parse_capability_arg_value(item.trim()))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array);
    }

    if let Ok(number) = value.parse::<u64>() {
        return Ok(Value::Number(number.into()));
    }

    if let Ok(boolean) = value.parse::<bool>() {
        return Ok(Value::Bool(boolean));
    }

    Ok(Value::String(trim_quotes(value).to_string()))
}

fn split_top_level(input: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut paren_depth = 0i32;
    let mut quote_char: Option<char> = None;
    let mut escaped = false;

    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }

        if character == '\\' {
            current.push(character);
            escaped = true;
            continue;
        }

        if let Some(active_quote) = quote_char {
            if character == active_quote {
                quote_char = None;
            }
            current.push(character);
            continue;
        }

        match character {
            '"' | '\'' => {
                quote_char = Some(character);
                current.push(character);
            }
            '[' => {
                bracket_depth += 1;
                current.push(character);
            }
            ']' => {
                bracket_depth -= 1;
                current.push(character);
            }
            '{' => {
                brace_depth += 1;
                current.push(character);
            }
            '}' => {
                brace_depth -= 1;
                current.push(character);
            }
            '(' => {
                paren_depth += 1;
                current.push(character);
            }
            ')' => {
                paren_depth -= 1;
                current.push(character);
            }
            c if c == delimiter && bracket_depth == 0 && brace_depth == 0 && paren_depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

fn split_once_top_level(input: &str, delimiter: char) -> Option<(&str, &str)> {
    let mut bracket_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut paren_depth = 0i32;
    let mut quote_char: Option<char> = None;
    let mut escaped = false;

    for (index, character) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if let Some(active_quote) = quote_char {
            if character == active_quote {
                quote_char = None;
            }
            continue;
        }

        match character {
            '"' | '\'' => quote_char = Some(character),
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            c if c == delimiter && bracket_depth == 0 && brace_depth == 0 && paren_depth == 0 => {
                return Some((&input[..index], &input[index + character.len_utf8()..]));
            }
            _ => {}
        }
    }

    None
}

fn trim_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(value)
}

fn string_arg(args: &CapabilityArgs, key: &str, default_value: String) -> Result<String, ApiError> {
    match args.get(key) {
        Some(Value::String(value)) => Ok(value.clone()),
        Some(other) => value_to_string(other).ok_or_else(|| {
            ApiError::bad_request(format!(
                "Capability argument '{}' must be a string, got {}",
                key, other
            ))
        }),
        None => Ok(default_value),
    }
}

fn string_vec_arg(
    args: &CapabilityArgs,
    key: &str,
    default_value: Vec<String>,
) -> Result<Vec<String>, ApiError> {
    match args.get(key) {
        Some(value) => value_to_string_vec(value).ok_or_else(|| {
            ApiError::bad_request(format!(
                "Capability argument '{}' must be a string or array of strings",
                key
            ))
        }),
        None => Ok(default_value),
    }
}

fn u64_arg(args: &CapabilityArgs, key: &str, default_value: u64) -> Result<u64, ApiError> {
    match args.get(key) {
        Some(Value::Number(number)) => number.as_u64().ok_or_else(|| {
            ApiError::bad_request(format!(
                "Capability argument '{}' must be a positive integer",
                key
            ))
        }),
        Some(Value::String(value)) => value.parse::<u64>().map_err(|_| {
            ApiError::bad_request(format!(
                "Capability argument '{}' must be a positive integer",
                key
            ))
        }),
        Some(_) => Err(ApiError::bad_request(format!(
            "Capability argument '{}' must be a positive integer",
            key
        ))),
        None => Ok(default_value),
    }
}

fn u32_arg(args: &CapabilityArgs, key: &str, default_value: u32) -> Result<u32, ApiError> {
    u64_arg(args, key, u64::from(default_value)).and_then(|value| {
        u32::try_from(value).map_err(|_| {
            ApiError::bad_request(format!("Capability argument '{}' is too large", key))
        })
    })
}

fn crud_operations_arg(
    args: &CapabilityArgs,
    key: &str,
    default_value: Vec<CrudOperation>,
) -> Result<Vec<CrudOperation>, ApiError> {
    let operation_names = match args.get(key) {
        Some(value) => value_to_string_vec(value).ok_or_else(|| {
            ApiError::bad_request(format!(
                "Capability argument '{}' must be an operation string or array",
                key
            ))
        })?,
        None => return Ok(default_value),
    };

    operation_names
        .into_iter()
        .map(|operation| parse_crud_operation(&operation))
        .collect()
}

fn vfs_operations_arg(
    args: &CapabilityArgs,
    key: &str,
    default_value: Vec<VfsOperation>,
) -> Result<Vec<VfsOperation>, ApiError> {
    let operation_names = match args.get(key) {
        Some(value) => value_to_string_vec(value).ok_or_else(|| {
            ApiError::bad_request(format!(
                "Capability argument '{}' must be a VFS operation string or array",
                key
            ))
        })?,
        None => return Ok(default_value),
    };

    vfs_operations_from_strings(operation_names)
}

fn parse_crud_operation(operation: &str) -> Result<CrudOperation, ApiError> {
    match operation.trim().to_lowercase().as_str() {
        "create" => Ok(CrudOperation::Create),
        "read" => Ok(CrudOperation::Read),
        "update" => Ok(CrudOperation::Update),
        "delete" => Ok(CrudOperation::Delete),
        "list" => Ok(CrudOperation::List),
        _ => Err(ApiError::bad_request(format!(
            "Unknown CRUD operation in capability: {}",
            operation
        ))),
    }
}

fn vfs_operations_from_strings(operations: Vec<String>) -> Result<Vec<VfsOperation>, ApiError> {
    operations
        .into_iter()
        .map(|operation| parse_vfs_operation(&operation))
        .collect()
}

fn parse_vfs_operation(operation: &str) -> Result<VfsOperation, ApiError> {
    match operation.trim().to_lowercase().as_str() {
        "write" => Ok(VfsOperation::Write),
        "read" => Ok(VfsOperation::Read),
        "move" | "rename" => Ok(VfsOperation::Move),
        "delete" => Ok(VfsOperation::Delete),
        "list" => Ok(VfsOperation::List),
        "usage" | "stats" | "usage_stats" => Ok(VfsOperation::Usage),
        _ => Err(ApiError::bad_request(format!(
            "Unknown VFS operation in capability: {}",
            operation
        ))),
    }
}

fn value_to_string_vec(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::Array(items) => items
            .iter()
            .map(value_to_string)
            .collect::<Option<Vec<String>>>(),
        _ => value_to_string(value).map(|value| vec![value]),
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn normalize_methods(methods: Vec<String>) -> Vec<String> {
    methods
        .into_iter()
        .map(|method| method.trim().to_uppercase())
        .collect()
}

fn json_string_array(
    config: &serde_json::Value,
    key: &str,
    default_value: Vec<String>,
) -> Result<Vec<String>, ApiError> {
    match config.get(key) {
        Some(value) => value_to_string_vec(value).ok_or_else(|| {
            ApiError::bad_request(format!(
                "Capability field '{}' must be a string or array of strings",
                key
            ))
        }),
        None => Ok(default_value),
    }
}

fn json_u64(config: &serde_json::Value, key: &str, default_value: u64) -> Result<u64, ApiError> {
    match config.get(key) {
        Some(Value::Number(value)) => value.as_u64().ok_or_else(|| {
            ApiError::bad_request(format!(
                "Capability field '{}' must be a positive integer",
                key
            ))
        }),
        Some(Value::String(value)) => value.parse::<u64>().map_err(|_| {
            ApiError::bad_request(format!(
                "Capability field '{}' must be a positive integer",
                key
            ))
        }),
        Some(_) => Err(ApiError::bad_request(format!(
            "Capability field '{}' must be a positive integer",
            key
        ))),
        None => Ok(default_value),
    }
}

fn json_u32(config: &serde_json::Value, key: &str, default_value: u32) -> Result<u32, ApiError> {
    json_u64(config, key, u64::from(default_value)).and_then(|value| {
        u32::try_from(value)
            .map_err(|_| ApiError::bad_request(format!("Capability field '{}' is too large", key)))
    })
}

/// Parse a trust level string into a PluginTrustLevel enum
pub fn parse_trust_level_string(trust_str: &str) -> Result<PluginTrustLevel, ApiError> {
    match trust_str.to_lowercase().as_str() {
        "untrusted" => Ok(PluginTrustLevel::Untrusted),
        "partiallytrusted" | "partially_trusted" => Ok(PluginTrustLevel::PartiallyTrusted),
        "fullytrusted" | "fully_trusted" => Ok(PluginTrustLevel::FullyTrusted),
        "system" => Ok(PluginTrustLevel::System),
        _ => Err(ApiError::bad_request(format!(
            "Unknown trust level: {}",
            trust_str
        ))),
    }
}

pub fn is_capability_allowed_for_trust_level(
    trust_level: &PluginTrustLevel,
    capability: &PluginCapability,
) -> bool {
    match trust_level {
        PluginTrustLevel::Untrusted => matches!(
            capability,
            PluginCapability::LogInfo
                | PluginCapability::LogError
                | PluginCapability::ReadEventData
        ),
        PluginTrustLevel::PartiallyTrusted => match capability {
            PluginCapability::HttpRequest { .. }
            | PluginCapability::ScheduleTasks
            | PluginCapability::RegisterHttpRoutes { .. }
            | PluginCapability::DeleteRecords { .. } => false,
            PluginCapability::AccessVfs { operations, .. } => operations.iter().all(|operation| {
                matches!(
                    operation,
                    VfsOperation::Read | VfsOperation::List | VfsOperation::Usage
                )
            }),
            _ => true,
        },
        PluginTrustLevel::FullyTrusted | PluginTrustLevel::System => true,
    }
}

pub fn minimum_trust_level_for_capabilities(capabilities: &[PluginCapability]) -> PluginTrustLevel {
    if capabilities.iter().all(|capability| {
        is_capability_allowed_for_trust_level(&PluginTrustLevel::Untrusted, capability)
    }) {
        PluginTrustLevel::Untrusted
    } else if capabilities.iter().all(|capability| {
        is_capability_allowed_for_trust_level(&PluginTrustLevel::PartiallyTrusted, capability)
    }) {
        PluginTrustLevel::PartiallyTrusted
    } else {
        PluginTrustLevel::FullyTrusted
    }
}

pub fn trust_level_rank(trust_level: &PluginTrustLevel) -> u8 {
    match trust_level {
        PluginTrustLevel::Untrusted => 0,
        PluginTrustLevel::PartiallyTrusted => 1,
        PluginTrustLevel::FullyTrusted => 2,
        PluginTrustLevel::System => 3,
    }
}

fn is_plugin_loaded(
    plugin_manager: &oxide_plugin_runtime::PluginManager,
    plugin_name: &str,
) -> Result<bool, ApiError> {
    let stats = plugin_manager
        .get_plugin_statistics()
        .map_err(|e| ApiError::internal(format!("Failed to get plugin statistics: {}", e)))?;

    Ok(stats.iter().any(|stat| stat.name == plugin_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scoped_record_capability() {
        let capability =
            parse_capability_string(r#"ReadRecords(collections=["posts", "comments"])"#).unwrap();

        assert!(matches!(
            capability,
            PluginCapability::ReadRecords { collections }
                if collections == vec!["posts".to_string(), "comments".to_string()]
        ));
    }

    #[test]
    fn parses_scoped_route_capability() {
        let capability = parse_capability_string(
            r#"RegisterHttpRoutes(path_patterns=["/api/hello/*"], methods=["get", "post"])"#,
        )
        .unwrap();

        assert!(matches!(
            capability,
            PluginCapability::RegisterHttpRoutes {
                path_patterns,
                methods,
            } if path_patterns == vec!["/api/hello/*".to_string()]
                && methods == vec!["GET".to_string(), "POST".to_string()]
        ));
    }

    #[test]
    fn parses_json_capability_object() {
        let capability =
            parse_capability_string(r#"{"CreateRecords":{"collections":["items"]}}"#).unwrap();

        assert!(matches!(
            capability,
            PluginCapability::CreateRecords { collections }
                if collections == vec!["items".to_string()]
        ));
    }

    #[test]
    fn parses_scoped_vfs_capability() {
        let capability = parse_capability_string(
            r#"AccessVfs(namespaces=["media/*"], operations=["read", "move", "list"])"#,
        )
        .unwrap();

        assert!(matches!(
            capability,
            PluginCapability::AccessVfs {
                namespaces,
                operations,
            } if namespaces == vec!["media/*".to_string()]
                && operations == vec![VfsOperation::Read, VfsOperation::Move, VfsOperation::List]
        ));
    }

    #[test]
    fn broader_grant_satisfies_scoped_requirement() {
        let granted = parse_capability_string(r#"ReadRecords(collections=["*"])"#).unwrap();
        let required = parse_capability_string(r#"ReadRecords(collections=["posts"])"#).unwrap();

        assert!(capability_satisfies(&granted, &required));
        assert!(!capability_satisfies(&required, &granted));
    }

    #[test]
    fn invalid_capability_string_is_rejected() {
        assert!(parse_capability_string("NotACapability").is_err());
        assert!(parse_capability_string("ReadRecords(collections=[").is_err());
    }
}
