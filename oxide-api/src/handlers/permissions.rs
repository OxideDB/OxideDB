//! Permission management handlers
//!
//! This module provides HTTP handlers for managing collection permissions
//! and access control rules. It allows administrators to configure
//! CRUD-based permissions similar to PocketBase.

use axum::{
    extract::{Path, State},
    Json,
};
use oxide_core::{
    CollectionPermissions, CrudOperation, PermissionLevel, UserRole,
    Claims,
};
use oxide_db::Db;
use std::sync::Arc;
use tracing::{debug, info};

use crate::{
    errors::ApiError,
    responses::ApiResponse,
    server::AppState,
};

/// Handlers for permission management operations
pub struct PermissionHandlers;

impl PermissionHandlers {
    /// Get permissions for a collection
    pub async fn get_permissions(
        db: Arc<dyn Db>,
        collection: String,
    ) -> Result<CollectionPermissions, ApiError> {
        debug!("Getting permissions for collection: {}", collection);

        // Check if collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        // Try to get custom permissions from database
        match db.get_permissions(&collection).await? {
            Some(permissions) => {
                debug!("Found custom permissions for collection: {}", collection);
                Ok(permissions)
            }
            None => {
                debug!("No custom permissions found, returning defaults for collection: {}", collection);
                // Return default permissions if no custom ones exist
                Ok(CollectionPermissions::new(collection))
            }
        }
    }

    /// Update permissions for a collection
    pub async fn update_permissions(
        db: Arc<dyn Db>,
        collection: String,
        permissions: CollectionPermissions,
        user_claims: Claims,
    ) -> Result<CollectionPermissions, ApiError> {
        debug!("Updating permissions for collection: {}", collection);

        // Only superusers can modify permissions
        let user_role: UserRole = user_claims.role.parse()
            .map_err(|_| ApiError::forbidden("Invalid user role".to_string()))?;
        
        if user_role != UserRole::Superuser {
            return Err(ApiError::forbidden("Only superusers can modify permissions".to_string()));
        }

        // Check if collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        // Validate permissions
        Self::validate_permissions(&permissions)?;

        // Store permissions in database
        let mut updated_permissions = permissions;
        updated_permissions.collection = collection;
        updated_permissions.update_timestamp();

        db.store_permissions(&updated_permissions).await?;

        info!("Updated permissions for collection: {}", updated_permissions.collection);
        Ok(updated_permissions)
    }

    /// List all collections with their permission status
    pub async fn list_collection_permissions(
        db: Arc<dyn Db>,
    ) -> Result<Vec<CollectionPermissionsInfo>, ApiError> {
        debug!("Listing all collection permissions");

        let collections = db.list_collections().await?;
        let collections_with_custom_permissions = db.list_collections_with_permissions().await?;
        let mut permissions_info = Vec::new();

        for collection_schema in collections {
            // Try to get custom permissions, fall back to defaults
            let permissions = match db.get_permissions(&collection_schema.name).await? {
                Some(custom_permissions) => custom_permissions,
                None => CollectionPermissions::new(collection_schema.name.clone()),
            };

            let has_custom_rules = collections_with_custom_permissions.contains(&collection_schema.name);
            
            let info = CollectionPermissionsInfo {
                collection_name: collection_schema.name,
                collection_type: collection_schema.collection_type,
                permissions,
                has_custom_rules,
            };
            
            permissions_info.push(info);
        }

        Ok(permissions_info)
    }

    /// Reset permissions for a collection to defaults
    pub async fn reset_permissions(
        db: Arc<dyn Db>,
        collection: String,
        user_claims: Claims,
    ) -> Result<CollectionPermissions, ApiError> {
        debug!("Resetting permissions for collection: {}", collection);

        // Only superusers can reset permissions
        let user_role: UserRole = user_claims.role.parse()
            .map_err(|_| ApiError::forbidden("Invalid user role".to_string()))?;
        
        if user_role != UserRole::Superuser {
            return Err(ApiError::forbidden("Only superusers can reset permissions".to_string()));
        }

        // Check if collection exists
        let exists = db.collection_exists(&collection).await?;
        if !exists {
            return Err(ApiError::not_found(format!("Collection '{}'", collection)));
        }

        // Delete custom permissions (revert to defaults)
        db.delete_permissions(&collection).await?;

        // Return default permissions
        let permissions = CollectionPermissions::new(collection);

        info!("Reset permissions to defaults for collection: {}", permissions.collection);
        Ok(permissions)
    }

    /// Create a permission preset (e.g., public, authenticated-only, etc.)
    pub async fn create_permission_preset(
        collection: String,
        preset_type: PermissionPresetType,
    ) -> Result<CollectionPermissions, ApiError> {
        debug!("Creating permission preset '{}' for collection: {}", preset_type, collection);

        let permissions = match preset_type {
            PermissionPresetType::Public => CollectionPermissions::new_public(collection),
            PermissionPresetType::AuthenticatedOnly => {
                let mut perms = CollectionPermissions::new(collection);
                for operation in [CrudOperation::Create, CrudOperation::Read, CrudOperation::Update, CrudOperation::Delete, CrudOperation::List] {
                    perms.set_operation_permission(operation, PermissionLevel::AuthenticatedOnly, None);
                }
                perms
            }
            PermissionPresetType::SuperuserOnly => CollectionPermissions::new(collection),
            PermissionPresetType::ReadOnly => {
                let mut perms = CollectionPermissions::new(collection);
                // Allow public read and list, but restrict create/update/delete to superuser
                perms.set_operation_permission(CrudOperation::Read, PermissionLevel::Public, None);
                perms.set_operation_permission(CrudOperation::List, PermissionLevel::Public, None);
                perms
            }
        };

        Ok(permissions)
    }

    /// Validate permission rules
    fn validate_permissions(permissions: &CollectionPermissions) -> Result<(), ApiError> {
        // Basic validation
        if permissions.collection.is_empty() {
            return Err(ApiError::bad_request("Collection name cannot be empty".to_string()));
        }

        // Validate that all CRUD operations have rules
        let required_operations = [
            CrudOperation::Create,
            CrudOperation::Read,
            CrudOperation::Update,
            CrudOperation::Delete,
            CrudOperation::List,
        ];

        for operation in &required_operations {
            if permissions.get_operation_rule(operation).is_none() {
                return Err(ApiError::bad_request(format!(
                    "Missing permission rule for {} operation",
                    operation
                )));
            }
        }

        // Validate custom rules syntax (basic check)
        for rule in permissions.rules.values() {
            if let PermissionLevel::Rule(rule_expr) = &rule.permission {
                if rule_expr.is_empty() {
                    return Err(ApiError::bad_request("Custom rule expression cannot be empty".to_string()));
                }
                // TODO: Add more sophisticated rule validation
            }
        }

        Ok(())
    }
}

/// Information about a collection's permissions
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CollectionPermissionsInfo {
    pub collection_name: String,
    pub collection_type: oxide_core::CollectionType,
    pub permissions: CollectionPermissions,
    pub has_custom_rules: bool,
}

/// Predefined permission preset types
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionPresetType {
    /// All operations are public (no authentication required)
    Public,
    /// All operations require authentication but any user can perform them
    AuthenticatedOnly,
    /// All operations require superuser privileges
    SuperuserOnly,
    /// Read and List operations are public, others require superuser
    ReadOnly,
}

impl std::fmt::Display for PermissionPresetType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionPresetType::Public => write!(f, "public"),
            PermissionPresetType::AuthenticatedOnly => write!(f, "authenticated_only"),
            PermissionPresetType::SuperuserOnly => write!(f, "superuser_only"),
            PermissionPresetType::ReadOnly => write!(f, "read_only"),
        }
    }
}

// HTTP Handler Functions

/// Get permissions for a specific collection
///
/// GET /collections/{collection}/permissions
pub async fn get_collection_permissions(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    let permissions = PermissionHandlers::get_permissions(state.db, collection).await?;
    Ok(Json(ApiResponse::success(permissions)))
}

/// Update permissions for a specific collection
///
/// PUT /collections/{collection}/permissions
pub async fn update_collection_permissions(
    authenticated_user: crate::extractors::AuthenticatedUser,
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(permissions): Json<CollectionPermissions>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    let updated_permissions = PermissionHandlers::update_permissions(
        state.db,
        collection,
        permissions,
        authenticated_user.claims,
    ).await?;
    
    Ok(Json(ApiResponse::success(updated_permissions)))
}

/// List all collections with their permissions
///
/// GET /permissions
pub async fn list_all_permissions(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<CollectionPermissionsInfo>>>, ApiError> {
    let permissions_list = PermissionHandlers::list_collection_permissions(state.db).await?;
    Ok(Json(ApiResponse::success(permissions_list)))
}

/// Reset permissions for a collection to defaults
///
/// POST /collections/{collection}/permissions/reset
pub async fn reset_collection_permissions(
    authenticated_user: crate::extractors::AuthenticatedUser,
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    let permissions = PermissionHandlers::reset_permissions(state.db, collection, authenticated_user.claims).await?;
    Ok(Json(ApiResponse::success(permissions)))
}

/// Create permissions from a preset
///
/// POST /collections/{collection}/permissions/preset
pub async fn create_permissions_from_preset(
    authenticated_user: crate::extractors::AuthenticatedUser,
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(preset): Json<PermissionPresetType>,
) -> Result<Json<ApiResponse<CollectionPermissions>>, ApiError> {
    // Get the permissions from the preset
    let permissions = PermissionHandlers::create_permission_preset(collection.clone(), preset).await?;
    
    // Store the permissions in the database
    let updated_permissions = PermissionHandlers::update_permissions(
        state.db,
        collection,
        permissions,
        authenticated_user.claims,
    ).await?;
    
    Ok(Json(ApiResponse::success(updated_permissions)))
}