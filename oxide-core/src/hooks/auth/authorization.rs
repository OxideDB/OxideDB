//! Authorization Hook
//!
//! This hook enforces API access control rules based on collection permissions
//! and user authentication status. It follows the hook-first architecture by
//! intercepting BeforeApiRequest events and validating permissions.

use crate::auth::types::{AuthOperation, Operation};
use crate::auth::PermissionService;
use crate::{
    AppError, AuthService, BeforeEventContext, Claims, CollectionPermissions, CrudOperation,
    PermissionContext,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Configuration for authorization behavior
#[derive(Debug, Clone)]
pub struct AuthorizationConfig {
    /// Default permission level for collections without explicit rules
    pub default_auth_required: bool,
    /// Collections that bypass authorization entirely (be very careful!)
    pub bypass_collections: Vec<String>,
    /// Whether to log authorization decisions
    pub log_decisions: bool,
}

impl Default for AuthorizationConfig {
    fn default() -> Self {
        Self {
            default_auth_required: true,
            bypass_collections: vec![
                "health".to_string(), // Health checks bypass auth
                "admin".to_string(),  // Admin UI should be publicly accessible
                "plugin".to_string(), // Plugin routes perform plugin-specific authorization later
                                      // Note: Auth endpoints are controlled via collection-specific rules
            ],
            log_decisions: true,
        }
    }
}

/// Authorization hook that enforces API access control
pub struct AuthorizationHook {
    auth_service: Arc<AuthService>,
    config: AuthorizationConfig,
    permission_service: Arc<dyn PermissionService>,
}

impl AuthorizationHook {
    /// Create a new authorization hook
    pub fn new(
        auth_service: Arc<AuthService>,
        permission_service: Arc<dyn PermissionService>,
    ) -> Self {
        Self {
            auth_service,
            config: AuthorizationConfig::default(),
            permission_service,
        }
    }

    /// Create a new authorization hook with custom configuration
    pub fn with_config(
        auth_service: Arc<AuthService>,
        permission_service: Arc<dyn PermissionService>,
        config: AuthorizationConfig,
    ) -> Self {
        Self {
            auth_service,
            config,
            permission_service,
        }
    }

    /// Handle BeforeApiRequest events for authorization
    pub async fn handle_before_api_request(
        &self,
        context: &mut BeforeEventContext,
    ) -> Result<(), AppError> {
        // Extract request information from the context
        let method = context
            .data
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("GET");

        let path = context
            .data
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("/");

        let empty_headers = serde_json::json!({});
        let headers = context.data.get("headers").unwrap_or(&empty_headers);

        debug!("🔐 Authorizing API request: {} {}", method, path);

        // Parse the collection and operation from the path
        let (collection, operation, _record_id) = self.parse_request_info(method, path)?;

        // Check if this collection bypasses authorization
        if self.config.bypass_collections.contains(&collection) {
            debug!("🔓 Bypassing authorization for collection: {}", collection);
            return Ok(());
        }

        // Extract authentication information from headers
        let user_claims = self.extract_user_claims(headers)?;

        // Get permission rules for the collection from permission service
        let permissions = match self.permission_service.get_permissions(&collection).await? {
            Some(perms) => perms,
            None => {
                // For collections without explicit permissions, we need to handle auth collections
                // specially to allow custom rules to work properly
                if self.is_auth_collection(&collection) {
                    // For auth collections, default to public permissions to allow login/register, etc.
                    debug!("No explicit permissions found for auth collection '{}', using public defaults", collection);
                    CollectionPermissions::public(collection.clone())
                } else if self.is_system_collection(&collection) {
                    // System collections get restrictive defaults
                    if self.config.default_auth_required {
                        CollectionPermissions::new(collection.clone())
                    } else {
                        CollectionPermissions::public(collection.clone())
                    }
                } else {
                    // User collections get default permissions that allow custom rules to work
                    // Use restrictive defaults but don't store them, so custom rules can override
                    debug!("No explicit permissions found for collection '{}', using runtime-only defaults", collection);
                    if self.config.default_auth_required {
                        CollectionPermissions::new(collection.clone())
                    } else {
                        CollectionPermissions::public(collection.clone())
                    }
                }
            }
        };

        // Create permission context with request metadata
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("headers".to_string(), headers.clone());
        metadata.insert(
            "method".to_string(),
            serde_json::Value::String(method.to_string()),
        );
        metadata.insert(
            "path".to_string(),
            serde_json::Value::String(path.to_string()),
        );

        let permission_context = PermissionContext::new(
            user_claims,
            operation.clone(),
            collection.clone(),
            None, // Record ID - could be extracted from path if needed
        )
        .with_metadata(metadata);
        warn!("🔍 Permission context: {:?}", permission_context);
        warn!("🔍 Permissions: {:?}", permissions);

        // Check permission using the permission service
        debug!("🔍 Authorization: About to call permission_service.check_permission");
        let allowed = self
            .permission_service
            .check_permission(&permissions, &permission_context)?;
        debug!(
            "🔍 Authorization: permission_service.check_permission returned: {}",
            allowed
        );

        if !allowed {
            let user_info = permission_context
                .user_claims
                .map(|claims| format!("user {} ({})", claims.sub, claims.role))
                .unwrap_or_else(|| "anonymous".to_string());

            let error_msg = format!(
                "Access denied: {} cannot perform {} operation on collection '{}'",
                user_info, operation, collection
            );

            if self.config.log_decisions {
                warn!("🚫 {}", error_msg);
            }

            return Err(AppError::auth(error_msg));
        }

        if self.config.log_decisions {
            let user_info = permission_context
                .user_claims
                .map(|claims| format!("user {} ({})", claims.sub, claims.role))
                .unwrap_or_else(|| "anonymous".to_string());

            info!(
                "✅ Access granted: {} can perform {} operation on collection '{}'",
                user_info, operation, collection
            );
        }

        Ok(())
    }

    /// Parse request information to extract collection, operation, and record ID
    fn parse_request_info(
        &self,
        method: &str,
        path: &str,
    ) -> Result<(String, Operation, Option<String>), AppError> {
        // Handle health checks
        if path.starts_with("/health") {
            return Ok((
                "health".to_string(),
                Operation::Crud(CrudOperation::Read),
                None,
            ));
        }

        // Handle auth endpoints
        if path.starts_with("/auth") {
            let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

            // Handle collection-specific auth endpoints: /auth/{collection}/{operation}
            if path_parts.len() >= 3 && path_parts[0] == "auth" {
                let collection = path_parts[1].to_string();
                let auth_operation = path_parts[2];

                let operation = match auth_operation {
                    "login" => Operation::Auth(AuthOperation::Login),
                    "register" => Operation::Auth(AuthOperation::Register),
                    _ => Operation::Crud(CrudOperation::Read),
                };

                return Ok((collection, operation, None));
            }

            // Handle global auth endpoints (not collection-specific)
            if path_parts.len() >= 2 && path_parts[0] == "auth" {
                let operation = match path_parts[1] {
                    "validate" => Operation::Auth(AuthOperation::TokenValidation),
                    "refresh" => Operation::Auth(AuthOperation::TokenRefresh),
                    "logout" => Operation::Auth(AuthOperation::Logout),
                    "me" => Operation::Auth(AuthOperation::GetCurrentUser),
                    "collections" => Operation::Auth(AuthOperation::ListAuthCollections),
                    _ => Operation::Crud(CrudOperation::Read),
                };
                return Ok(("auth".to_string(), operation, None));
            }

            return Ok((
                "auth".to_string(),
                Operation::Crud(CrudOperation::Read),
                None,
            ));
        }

        // Handle admin UI endpoints - these should be publicly accessible
        if path.starts_with("/admin") {
            return Ok((
                "admin".to_string(),
                Operation::Crud(CrudOperation::Read),
                None,
            ));
        }

        // Plugin HTTP routes are authorized after route matching, using the
        // owning plugin's synthetic collection (`plugin:{name}`).
        if path.starts_with("/plugin") {
            let operation = match method {
                "GET" => Operation::Crud(CrudOperation::Read),
                "POST" => Operation::Crud(CrudOperation::Create),
                "PUT" | "PATCH" => Operation::Crud(CrudOperation::Update),
                "DELETE" => Operation::Crud(CrudOperation::Delete),
                _ => Operation::Crud(CrudOperation::Read),
            };
            return Ok(("plugin".to_string(), operation, None));
        }

        // Handle permission endpoints
        if path.starts_with("/permissions") {
            return Ok((
                "permissions".to_string(),
                Operation::Crud(CrudOperation::Read),
                None,
            ));
        }

        // Parse collection endpoints: /collections/{collection} or /collections/{collection}/records/{id}
        let path_parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();

        // Handle the base /collections endpoint (list all collections)
        if path_parts.len() == 1 && path_parts[0] == "collections" {
            let operation = match method {
                "GET" => Operation::Crud(CrudOperation::List),
                "POST" => Operation::Crud(CrudOperation::Create),
                _ => Operation::Crud(CrudOperation::List),
            };
            return Ok(("collections".to_string(), operation, None));
        }

        if path_parts.len() >= 2 && path_parts[0] == "collections" {
            let collection = path_parts[1].to_string();

            // Handle collection-specific permission endpoints
            if path_parts.len() >= 3 && path_parts[2] == "permissions" {
                return Ok((
                    "permissions".to_string(),
                    Operation::Crud(CrudOperation::Update),
                    None,
                ));
            }

            if path_parts.len() >= 4 && path_parts[2] == "records" {
                // Record-specific operations: /collections/{collection}/records/{id}
                let record_id = Some(path_parts[3].to_string());
                let operation = match method {
                    "GET" => Operation::Crud(CrudOperation::Read),
                    "PUT" | "PATCH" => Operation::Crud(CrudOperation::Update),
                    "DELETE" => Operation::Crud(CrudOperation::Delete),
                    _ => Operation::Crud(CrudOperation::Read),
                };
                return Ok((collection, operation, record_id));
            } else if path_parts.len() == 3 && path_parts[2] == "records" {
                // Collection-level operations: /collections/{collection}/records
                let operation = match method {
                    "GET" => Operation::Crud(CrudOperation::List),
                    "POST" => Operation::Crud(CrudOperation::Create),
                    _ => Operation::Crud(CrudOperation::List),
                };
                return Ok((collection, operation, None));
            }
        }

        // Other collection-related endpoints (schema, stats, etc.)
        let operation = match method {
            "GET" => Operation::Crud(CrudOperation::Read),
            "POST" => Operation::Crud(CrudOperation::Create),
            "PUT" | "PATCH" => Operation::Crud(CrudOperation::Update),
            "DELETE" => Operation::Crud(CrudOperation::Delete),
            _ => Operation::Crud(CrudOperation::Read),
        };

        Ok(("unknown".to_string(), operation, None))
    }

    /// Extract user claims from request headers
    fn extract_user_claims(&self, headers: &serde_json::Value) -> Result<Option<Claims>, AppError> {
        let authorization = headers
            .get("authorization")
            .or_else(|| headers.get("Authorization"))
            .and_then(|v| v.as_str());

        let token = match authorization {
            Some(auth_header) => {
                if let Some(stripped) = auth_header.strip_prefix("Bearer ") {
                    stripped // Remove "Bearer " prefix
                } else {
                    auth_header
                }
            }
            None => return Ok(None), // No authentication provided
        };

        match self.auth_service.verify_token(token) {
            Ok(claims) => Ok(Some(claims)),
            Err(_) => {
                debug!("🔑 Invalid or expired token provided");
                Ok(None) // Invalid token treated as no authentication
            }
        }
    }

    /// Initialize default permissions for system collections
    pub async fn initialize_default_permissions(&self) -> Result<(), AppError> {
        // Only initialize permissions for truly internal system collections
        // Auth collections like "_users" should NOT get default permissions
        // so that custom rules can be applied to them

        // Collections endpoint permissions - require authentication for all operations
        let collections_permissions = CollectionPermissions::new("collections".to_string());
        // All collection operations require at least authenticated user by default
        // Specific permissions can be configured per collection as needed
        self.permission_service
            .store_permissions(&collections_permissions)
            .await?;

        // Note: Admin UI routes are handled via bypass_collections in AuthorizationConfig
        // They remain publicly accessible for authentication purposes only

        // Note: We no longer initialize default permissions for "_users" and "_superusers"
        // collections to allow custom rules to work properly

        info!("🔐 Default authorization permissions initialized (auth collections left for custom rules)");
        Ok(())
    }

    /// Check if a collection is a system collection that should have restrictive defaults
    fn is_system_collection(&self, collection: &str) -> bool {
        // System collections that should have restrictive defaults
        // Note: "_users" is NOT included here to allow custom rules to work on auth collections
        matches!(collection, "_internal" | "_system" | "_meta")
    }

    /// Check if a collection is an auth collection
    fn is_auth_collection(&self, collection: &str) -> bool {
        // Use the auth service to check if this is a configured auth collection
        self.auth_service.config().is_auth_collection(collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        auth::{AuthServiceConfig, PermissionLevel},
        UserRole,
    };
    use std::collections::HashMap;
    use std::sync::Arc;

    // Mock permission service for testing
    struct MockPermissionService {
        permissions: std::sync::Mutex<HashMap<String, CollectionPermissions>>,
    }

    impl MockPermissionService {
        fn new() -> Self {
            Self {
                permissions: std::sync::Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl PermissionService for MockPermissionService {
        fn check_authentication(&self, context: &PermissionContext) -> Result<bool, AppError> {
            Ok(context.user_claims.is_some())
        }

        fn check_permission(
            &self,
            permissions: &CollectionPermissions,
            context: &PermissionContext,
        ) -> Result<bool, AppError> {
            let rule = match permissions.get_operation_rule(&context.operation) {
                Some(rule) => rule,
                None => return Ok(false),
            };

            match &rule.permission {
                PermissionLevel::None => Ok(false),
                PermissionLevel::Public => Ok(true),
                PermissionLevel::AuthenticatedOnly => Ok(context.user_claims.is_some()),
                PermissionLevel::SuperuserOnly => {
                    Ok(matches!(context.user_role(), Some(UserRole::Superuser)))
                }
                PermissionLevel::Rule(_) => Ok(context.user_claims.is_some()),
            }
        }

        async fn store_permissions(
            &self,
            permissions: &CollectionPermissions,
        ) -> Result<(), AppError> {
            let mut perms = self.permissions.lock().unwrap();
            perms.insert(permissions.collection.clone(), permissions.clone());
            Ok(())
        }

        async fn get_permissions(
            &self,
            collection: &str,
        ) -> Result<Option<CollectionPermissions>, AppError> {
            let perms = self.permissions.lock().unwrap();
            Ok(perms.get(collection).cloned())
        }

        async fn delete_permissions(&self, collection: &str) -> Result<(), AppError> {
            let mut perms = self.permissions.lock().unwrap();
            perms.remove(collection);
            Ok(())
        }

        async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError> {
            let perms = self.permissions.lock().unwrap();
            Ok(perms.keys().cloned().collect())
        }
    }

    #[tokio::test]
    async fn test_authorization_hook() {
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let permission_service = Arc::new(MockPermissionService::new());
        let hook = AuthorizationHook::new(auth_service, permission_service.clone());

        // Set up a restrictive collection permission (superuser only)
        let permissions = CollectionPermissions::new("sensitive_data".to_string());
        permission_service
            .store_permissions(&permissions)
            .await
            .unwrap();

        // Create a request context with user token
        let mut context = BeforeEventContext::new_create(
            "api".to_string(),
            serde_json::json!({
                "method": "GET",
                "path": "/collections/sensitive_data/records",
                "headers": {}
            }),
        );

        // Should fail for unauthenticated request
        let result = hook.handle_before_api_request(&mut context).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_request_info() {
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let permission_service = Arc::new(MockPermissionService::new());
        let hook = AuthorizationHook::new(auth_service, permission_service);

        // Test collections list endpoint
        let (collection, operation, record_id) =
            hook.parse_request_info("GET", "/collections").unwrap();
        assert_eq!(collection, "collections");
        assert_eq!(operation, Operation::Crud(CrudOperation::List));
        assert_eq!(record_id, None);

        // Test collections create endpoint
        let (collection, operation, record_id) =
            hook.parse_request_info("POST", "/collections").unwrap();
        assert_eq!(collection, "collections");
        assert_eq!(operation, Operation::Crud(CrudOperation::Create));
        assert_eq!(record_id, None);

        // Test collection list endpoint
        let (collection, operation, record_id) = hook
            .parse_request_info("GET", "/collections/users/records")
            .unwrap();
        assert_eq!(collection, "users");
        assert_eq!(operation, Operation::Crud(CrudOperation::List));
        assert_eq!(record_id, None);

        // Test record read endpoint
        let (collection, operation, record_id) = hook
            .parse_request_info("GET", "/collections/users/records/123")
            .unwrap();
        assert_eq!(collection, "users");
        assert_eq!(operation, Operation::Crud(CrudOperation::Read));
        assert_eq!(record_id, Some("123".to_string()));
    }

    #[test]
    fn test_superuser_bypass() {
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let permission_service = Arc::new(MockPermissionService::new());
        let _hook = AuthorizationHook::new(auth_service.clone(), permission_service.clone());

        // TODO: Implement actual test logic for superuser bypass behavior
    }

    #[test]
    fn test_rule_based_permission() {
        let auth_config = AuthServiceConfig::new("test_secret".to_string());
        let auth_service = Arc::new(AuthService::new(auth_config));
        let permission_service = Arc::new(MockPermissionService::new());
        let _hook = AuthorizationHook::new(auth_service.clone(), permission_service.clone());

        // TODO: Implement actual test logic for rule-based permission behavior
    }
}
