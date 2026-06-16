//! API key rule management handlers
//!
//! OxideDB already supports header-aware permission rules. These handlers expose
//! a focused admin API for managing `x-api-key` access rules without requiring
//! the frontend to manually edit raw collection permissions.

use axum::{extract::State, Json};
use oxide_core::{
    auth::types::AuthOperation, CollectionPermissions, CollectionType, CrudOperation,
    PermissionLevel,
};
use oxide_db::Db;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, info};
use uuid::Uuid;

use crate::{
    errors::ApiError,
    extractors::AuthenticatedUser,
    handlers::permissions::{CollectionPermissionsInfo, PermissionHandlers},
    responses::ApiResponse,
    server::AppState,
};

const API_KEY_HEADER_VARIABLE: &str = "@req.headers.x-api-key";
const API_KEY_HEADER_HASH_VARIABLE: &str = "@req.headers.x-api-key.sha256";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeyOperationType {
    Crud,
    Auth,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyCollectionOption {
    pub name: String,
    pub collection_type: CollectionType,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyRuleInfo {
    pub collection: String,
    pub collection_type: CollectionType,
    pub operation_type: ApiKeyOperationType,
    pub operation: String,
    pub rule: String,
    pub key_hash: Option<String>,
    pub key_preview: Option<String>,
    pub hashed: bool,
    pub legacy_plaintext: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiKeyRulesResponse {
    pub rules: Vec<ApiKeyRuleInfo>,
    pub collections: Vec<ApiKeyCollectionOption>,
    pub total_rules: usize,
    pub protected_collections: usize,
    pub exact_key_rules: usize,
    pub hashed_key_rules: usize,
    pub wildcard_key_rules: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertApiKeyRuleRequest {
    pub collection: String,
    pub operation_type: ApiKeyOperationType,
    pub operation: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpsertApiKeyRuleResponse {
    pub rule: ApiKeyRuleInfo,
    pub api_key: String,
    pub summary: ApiKeyRulesResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RevokeApiKeyRuleRequest {
    pub collection: String,
    pub operation_type: ApiKeyOperationType,
    pub operation: String,
    pub fallback_permission: Option<PermissionLevel>,
}

pub struct ApiKeyHandlers;

impl ApiKeyHandlers {
    pub async fn list_rules(db: Arc<dyn Db>) -> Result<ApiKeyRulesResponse, ApiError> {
        debug!("Listing API key rules");

        let permissions = PermissionHandlers::list_collection_permissions(db).await?;
        Ok(Self::build_response(permissions))
    }

    pub async fn upsert_rule(
        db: Arc<dyn Db>,
        request: UpsertApiKeyRuleRequest,
    ) -> Result<UpsertApiKeyRuleResponse, ApiError> {
        let collection = normalize_required_string(&request.collection, "Collection")?;
        let operation = normalize_required_string(&request.operation, "Operation")?;
        let api_key = validate_or_generate_key(request.key)?;
        let key_hash = hash_api_key(&api_key);
        let rule_expr = format!("{} = '{}'", API_KEY_HEADER_HASH_VARIABLE, key_hash);

        debug!(
            "Upserting API key rule for collection '{}' operation '{}' ({:?})",
            collection, operation, request.operation_type
        );

        let schema = db.get_collection_schema(&collection).await?;
        let mut permissions =
            PermissionHandlers::get_permissions(Arc::clone(&db), collection.clone()).await?;

        Self::set_permission_rule(
            &mut permissions,
            schema.collection_type.clone(),
            request.operation_type.clone(),
            &operation,
            PermissionLevel::Rule(rule_expr),
        )?;

        touch_permissions(&mut permissions);
        db.store_permissions(&permissions).await?;

        let rule = Self::find_rule_info(
            &permissions,
            schema.collection_type,
            request.operation_type,
            &operation,
        )
        .ok_or_else(|| ApiError::internal("Failed to read back stored API key rule"))?;

        let summary = Self::list_rules(db).await?;

        info!(
            "API key rule saved for collection '{}' operation '{}'",
            collection, operation
        );

        Ok(UpsertApiKeyRuleResponse {
            rule,
            api_key,
            summary,
        })
    }

    pub async fn revoke_rule(
        db: Arc<dyn Db>,
        request: RevokeApiKeyRuleRequest,
    ) -> Result<ApiKeyRulesResponse, ApiError> {
        let collection = normalize_required_string(&request.collection, "Collection")?;
        let operation = normalize_required_string(&request.operation, "Operation")?;

        debug!(
            "Revoking API key rule for collection '{}' operation '{}' ({:?})",
            collection, operation, request.operation_type
        );

        let schema = db.get_collection_schema(&collection).await?;
        let mut permissions =
            PermissionHandlers::get_permissions(Arc::clone(&db), collection.clone()).await?;

        let fallback = request
            .fallback_permission
            .unwrap_or(PermissionLevel::AuthenticatedOnly);

        Self::set_permission_rule(
            &mut permissions,
            schema.collection_type,
            request.operation_type,
            &operation,
            fallback,
        )?;

        touch_permissions(&mut permissions);
        db.store_permissions(&permissions).await?;

        info!(
            "API key rule revoked for collection '{}' operation '{}'",
            collection, operation
        );

        Self::list_rules(db).await
    }

    fn build_response(permissions: Vec<CollectionPermissionsInfo>) -> ApiKeyRulesResponse {
        let mut rules = Vec::new();
        let mut collections = Vec::new();

        for info in permissions {
            collections.push(ApiKeyCollectionOption {
                name: info.collection_name.clone(),
                collection_type: info.collection_type.clone(),
            });

            rules.extend(Self::extract_rules(&info));
        }

        let protected_collections = rules
            .iter()
            .map(|rule| rule.collection.clone())
            .collect::<HashSet<_>>()
            .len();
        let exact_key_rules = rules.iter().filter(|rule| rule.legacy_plaintext).count();
        let hashed_key_rules = rules.iter().filter(|rule| rule.hashed).count();
        let wildcard_key_rules = rules
            .len()
            .saturating_sub(exact_key_rules + hashed_key_rules);

        ApiKeyRulesResponse {
            total_rules: rules.len(),
            rules,
            collections,
            protected_collections,
            exact_key_rules,
            hashed_key_rules,
            wildcard_key_rules,
        }
    }

    fn extract_rules(info: &CollectionPermissionsInfo) -> Vec<ApiKeyRuleInfo> {
        let mut rules = Vec::new();

        for operation in crud_operations() {
            if let Some(rule) = info.permissions.crud_rules.get(&operation) {
                if let PermissionLevel::Rule(rule_expr) = &rule.permission {
                    if is_api_key_rule(rule_expr) {
                        rules.push(build_rule_info(
                            &info.collection_name,
                            info.collection_type.clone(),
                            ApiKeyOperationType::Crud,
                            operation.to_string(),
                            rule_expr.clone(),
                        ));
                    }
                }
            }
        }

        if info.collection_type == CollectionType::Auth {
            for operation in auth_operations() {
                if let Some(rule) = info.permissions.auth_rules.get(&operation) {
                    if let PermissionLevel::Rule(rule_expr) = &rule.permission {
                        if is_api_key_rule(rule_expr) {
                            rules.push(build_rule_info(
                                &info.collection_name,
                                info.collection_type.clone(),
                                ApiKeyOperationType::Auth,
                                operation.to_string(),
                                rule_expr.clone(),
                            ));
                        }
                    }
                }
            }
        }

        rules
    }

    fn set_permission_rule(
        permissions: &mut CollectionPermissions,
        collection_type: CollectionType,
        operation_type: ApiKeyOperationType,
        operation: &str,
        permission: PermissionLevel,
    ) -> Result<(), ApiError> {
        match operation_type {
            ApiKeyOperationType::Crud => {
                let operation = parse_crud_operation(operation)?;
                permissions.set_crud_permission(operation, permission);
                Ok(())
            }
            ApiKeyOperationType::Auth => {
                if collection_type != CollectionType::Auth {
                    return Err(ApiError::bad_request(
                        "Auth operations can only be configured for auth collections",
                    ));
                }

                let operation = parse_auth_operation(operation)?;
                permissions.set_auth_permission(operation, permission);
                Ok(())
            }
        }
    }

    fn find_rule_info(
        permissions: &CollectionPermissions,
        collection_type: CollectionType,
        operation_type: ApiKeyOperationType,
        operation: &str,
    ) -> Option<ApiKeyRuleInfo> {
        let permission = match operation_type {
            ApiKeyOperationType::Crud => permissions
                .crud_rules
                .get(&parse_crud_operation(operation).ok()?)?
                .permission
                .clone(),
            ApiKeyOperationType::Auth => permissions
                .auth_rules
                .get(&parse_auth_operation(operation).ok()?)?
                .permission
                .clone(),
        };

        match permission {
            PermissionLevel::Rule(rule_expr) if is_api_key_rule(&rule_expr) => {
                Some(build_rule_info(
                    &permissions.collection,
                    collection_type,
                    operation_type,
                    operation.to_string(),
                    rule_expr,
                ))
            }
            _ => None,
        }
    }
}

pub async fn list_api_key_rules(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<ApiKeyRulesResponse>>, ApiError> {
    ensure_superuser(&authenticated_user)?;

    let response = ApiKeyHandlers::list_rules(state.db).await?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn upsert_api_key_rule(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(request): Json<UpsertApiKeyRuleRequest>,
) -> Result<Json<ApiResponse<UpsertApiKeyRuleResponse>>, ApiError> {
    ensure_superuser(&authenticated_user)?;

    let response = ApiKeyHandlers::upsert_rule(state.db, request).await?;
    Ok(Json(ApiResponse::success(response)))
}

pub async fn revoke_api_key_rule(
    authenticated_user: AuthenticatedUser,
    State(state): State<AppState>,
    Json(request): Json<RevokeApiKeyRuleRequest>,
) -> Result<Json<ApiResponse<ApiKeyRulesResponse>>, ApiError> {
    ensure_superuser(&authenticated_user)?;

    let response = ApiKeyHandlers::revoke_rule(state.db, request).await?;
    Ok(Json(ApiResponse::success(response)))
}

fn ensure_superuser(authenticated_user: &AuthenticatedUser) -> Result<(), ApiError> {
    if authenticated_user.is_superuser() {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "Superuser privileges are required to manage API keys",
        ))
    }
}

fn crud_operations() -> [CrudOperation; 5] {
    [
        CrudOperation::Create,
        CrudOperation::Read,
        CrudOperation::Update,
        CrudOperation::Delete,
        CrudOperation::List,
    ]
}

fn auth_operations() -> [AuthOperation; 7] {
    [
        AuthOperation::Login,
        AuthOperation::Register,
        AuthOperation::TokenValidation,
        AuthOperation::TokenRefresh,
        AuthOperation::Logout,
        AuthOperation::GetCurrentUser,
        AuthOperation::ListAuthCollections,
    ]
}

fn parse_crud_operation(operation: &str) -> Result<CrudOperation, ApiError> {
    match operation {
        "create" => Ok(CrudOperation::Create),
        "read" => Ok(CrudOperation::Read),
        "update" => Ok(CrudOperation::Update),
        "delete" => Ok(CrudOperation::Delete),
        "list" => Ok(CrudOperation::List),
        _ => Err(ApiError::bad_request(format!(
            "Unsupported CRUD operation '{}'",
            operation
        ))),
    }
}

fn parse_auth_operation(operation: &str) -> Result<AuthOperation, ApiError> {
    match operation {
        "login" => Ok(AuthOperation::Login),
        "register" => Ok(AuthOperation::Register),
        "token_validation" => Ok(AuthOperation::TokenValidation),
        "token_refresh" => Ok(AuthOperation::TokenRefresh),
        "logout" => Ok(AuthOperation::Logout),
        "get_current_user" => Ok(AuthOperation::GetCurrentUser),
        "list_auth_collections" => Ok(AuthOperation::ListAuthCollections),
        _ => Err(ApiError::bad_request(format!(
            "Unsupported auth operation '{}'",
            operation
        ))),
    }
}

fn build_rule_info(
    collection: &str,
    collection_type: CollectionType,
    operation_type: ApiKeyOperationType,
    operation: String,
    rule: String,
) -> ApiKeyRuleInfo {
    let legacy_plaintext_key = extract_exact_key(&rule);
    let key_hash = extract_key_hash(&rule);
    let hashed = key_hash.is_some();
    let legacy_plaintext = legacy_plaintext_key.is_some();
    let key_preview = legacy_plaintext_key
        .as_deref()
        .map(mask_key)
        .or_else(|| key_hash.as_deref().map(mask_hash));

    ApiKeyRuleInfo {
        collection: collection.to_string(),
        collection_type,
        operation_type,
        operation,
        rule,
        key_hash,
        key_preview,
        hashed,
        legacy_plaintext,
    }
}

fn is_api_key_rule(rule: &str) -> bool {
    rule.contains(API_KEY_HEADER_VARIABLE) || rule.contains(API_KEY_HEADER_HASH_VARIABLE)
}

fn extract_exact_key(rule: &str) -> Option<String> {
    if !is_api_key_rule(rule) || is_hashed_api_key_rule(rule) {
        return None;
    }

    extract_rhs_quoted_value(rule)
}

fn extract_key_hash(rule: &str) -> Option<String> {
    if !is_hashed_api_key_rule(rule) {
        return None;
    }

    extract_rhs_quoted_value(rule)
}

fn is_hashed_api_key_rule(rule: &str) -> bool {
    rule.contains(API_KEY_HEADER_HASH_VARIABLE)
}

fn extract_rhs_quoted_value(rule: &str) -> Option<String> {
    let (_, rhs) = rule.split_once('=')?;
    let trimmed = rhs.trim();
    let quote = trimmed.chars().next()?;

    if quote != '\'' && quote != '"' {
        return None;
    }

    let value_start = quote.len_utf8();
    let closing = trimmed[value_start..].find(quote)?;
    Some(trimmed[value_start..value_start + closing].to_string())
}

fn mask_key(key: &str) -> String {
    if key.len() <= 10 {
        "****".to_string()
    } else {
        format!("{}...{}", &key[..6], &key[key.len() - 4..])
    }
}

fn mask_hash(hash: &str) -> String {
    let preview_len = hash.len().min(12);
    format!("sha256:{}...", &hash[..preview_len])
}

fn validate_or_generate_key(key: Option<String>) -> Result<String, ApiError> {
    let key = key.unwrap_or_else(generate_api_key);
    let key = key.trim().to_string();

    if key.len() < 16 {
        return Err(ApiError::bad_request(
            "API keys must be at least 16 characters long",
        ));
    }

    if key.len() > 256 {
        return Err(ApiError::bad_request(
            "API keys cannot be longer than 256 characters",
        ));
    }

    if !key
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    {
        return Err(ApiError::bad_request(
            "API keys may only contain letters, numbers, dots, underscores, and dashes",
        ));
    }

    Ok(key)
}

fn generate_api_key() -> String {
    format!("ox_{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

fn hash_api_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

fn normalize_required_string(value: &str, label: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(ApiError::bad_request(format!("{} is required", label)))
    } else {
        Ok(trimmed.to_string())
    }
}

fn touch_permissions(permissions: &mut CollectionPermissions) {
    permissions.updated_at = chrono::Utc::now().timestamp();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_api_key_rule_does_not_expose_plaintext_key() {
        let key = "ox_test_secret_key_123456789";
        let key_hash = hash_api_key(key);
        let rule = format!("{} = '{}'", API_KEY_HEADER_HASH_VARIABLE, key_hash);

        let info = build_rule_info(
            "articles",
            CollectionType::Base,
            ApiKeyOperationType::Crud,
            "list".to_string(),
            rule,
        );

        assert!(info.hashed);
        assert!(!info.legacy_plaintext);
        assert_eq!(info.key_hash.as_deref(), Some(key_hash.as_str()));
        assert!(!info.rule.contains(key));
    }

    #[test]
    fn legacy_plaintext_rule_is_detected_but_not_confused_with_hash() {
        let rule = format!("{} = '{}'", API_KEY_HEADER_VARIABLE, "legacy-secret");

        let info = build_rule_info(
            "articles",
            CollectionType::Base,
            ApiKeyOperationType::Crud,
            "read".to_string(),
            rule,
        );

        assert!(!info.hashed);
        assert!(info.legacy_plaintext);
        assert!(info.key_hash.is_none());
        assert_eq!(info.key_preview.as_deref(), Some("legacy...cret"));
    }
}
