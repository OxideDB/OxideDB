//! Hook Registry
//!
//! This module provides a centralized registration system for all system hooks.
//! It allows for organized configuration and registration of hooks with the event bus.

use crate::{
    auth::PermissionService,
    hooks::{
        audit::{ActivityLoggerHook, SecurityAuditHook},
        auth::{AuthorizationHook, PasswordHashingHook, UserValidationHook},
        validation::{AuthSchemaValidatorHook, DataSanitizerHook, SchemaValidatorHook},
    },
    AppError, AuthService, BeforeEventType, EventBus,
};
use std::sync::Arc;
use tracing::info;

/// Configuration for system hook registration
#[derive(Debug, Clone)]
pub struct HookRegistryConfig {
    /// Enable password hashing hooks
    pub enable_password_hooks: bool,
    /// Enable user validation hooks
    pub enable_user_validation: bool,
    /// Enable authorization hooks
    pub enable_authorization: bool,
    /// Enable activity logging hooks
    pub enable_activity_logging: bool,
    /// Enable security audit hooks
    pub enable_security_audit: bool,
    /// Enable schema validation hooks
    pub enable_schema_validation: bool,
    /// Enable data sanitization hooks
    pub enable_data_sanitization: bool,
    /// Enable auth schema validation hooks
    pub enable_auth_schema_validation: bool,
}

impl Default for HookRegistryConfig {
    fn default() -> Self {
        Self {
            enable_password_hooks: true,
            enable_user_validation: true,
            enable_authorization: true,
            enable_activity_logging: true,
            enable_security_audit: true,
            enable_schema_validation: false, // Disabled by default as it requires schema setup
            enable_data_sanitization: true,
            enable_auth_schema_validation: true, // Enabled by default for auth collections
        }
    }
}

/// Hook registry for centralized hook management
pub struct HookRegistry {
    config: HookRegistryConfig,
    auth_service: Option<Arc<AuthService>>,
    permission_service: Option<Arc<dyn PermissionService>>,
}

impl Default for HookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl HookRegistry {
    /// Create a new hook registry with default configuration
    pub fn new() -> Self {
        Self {
            config: HookRegistryConfig::default(),
            auth_service: None,
            permission_service: None,
        }
    }

    /// Create a new hook registry with custom configuration
    pub fn with_config(config: HookRegistryConfig) -> Self {
        Self {
            config,
            auth_service: None,
            permission_service: None,
        }
    }

    /// Set the auth service for hooks that require it
    pub fn with_auth_service(mut self, auth_service: Arc<AuthService>) -> Self {
        self.auth_service = Some(auth_service);
        self
    }

    /// Set the permission service for authorization hooks
    pub fn with_permission_service(
        mut self,
        permission_service: Arc<dyn PermissionService>,
    ) -> Self {
        self.permission_service = Some(permission_service);
        self
    }

    /// Register all enabled system hooks with the event bus
    pub async fn register_all_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        info!("🎣 Registering system hooks...");

        let mut registered_count = 0;

        // Register authentication hooks
        if self.config.enable_password_hooks {
            self.register_password_hooks(event_bus).await?;
            registered_count += 1;
        }

        if self.config.enable_user_validation {
            self.register_user_validation_hooks(event_bus).await?;
            registered_count += 1;
        }

        if self.config.enable_authorization {
            self.register_authorization_hooks(event_bus).await?;
            registered_count += 1;
        }

        // Register audit hooks
        if self.config.enable_activity_logging {
            self.register_activity_logging_hooks(event_bus).await?;
            registered_count += 1;
        }

        if self.config.enable_security_audit {
            self.register_security_audit_hooks(event_bus).await?;
            registered_count += 1;
        }

        // Register validation hooks
        if self.config.enable_schema_validation {
            self.register_schema_validation_hooks(event_bus).await?;
            registered_count += 1;
        }

        if self.config.enable_auth_schema_validation {
            self.register_auth_schema_validation_hooks(event_bus)
                .await?;
            registered_count += 1;
        }

        if self.config.enable_data_sanitization {
            self.register_data_sanitization_hooks(event_bus).await?;
            registered_count += 1;
        }

        info!(
            "✅ Successfully registered {} hook categories",
            registered_count
        );
        Ok(())
    }

    /// Register password hashing hooks
    async fn register_password_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let auth_service = self
            .auth_service
            .as_ref()
            .ok_or_else(|| AppError::internal("AuthService is required for password hooks"))?;

        let hook = Arc::new(PasswordHashingHook::new(Arc::clone(auth_service)));

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "password_hash_create".to_string(),
            "Password Hashing (Create)".to_string(),
        )
        .with_description("Hash passwords before creating user records".to_string())
        .with_priority(100); // High priority for security

        event_bus
            .subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_create);
                    Box::pin(async move { hook.handle_before_record_create(context).await })
                }),
                metadata,
            )
            .await?;

        // Register for record updates
        let hook_update = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "password_hash_update".to_string(),
            "Password Hashing (Update)".to_string(),
        )
        .with_description("Hash passwords before updating user records".to_string())
        .with_priority(100); // High priority for security

        event_bus
            .subscribe_before(
                BeforeEventType::RecordUpdate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_update);
                    Box::pin(async move { hook.handle_before_record_update(context).await })
                }),
                metadata,
            )
            .await?;

        info!("🔒 Password hashing hooks registered");
        Ok(())
    }

    /// Register user validation hooks
    async fn register_user_validation_hooks(
        &self,
        event_bus: &dyn EventBus,
    ) -> Result<(), AppError> {
        let hook = Arc::new(UserValidationHook::new().map_err(|e| {
            AppError::internal(format!("Failed to create user validation hook: {}", e))
        })?);

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "user_validation_create".to_string(),
            "User Validation (Create)".to_string(),
        )
        .with_description("Validate user data before creating records".to_string())
        .with_priority(50);

        event_bus
            .subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_create);
                    Box::pin(async move { hook.handle_before_record_create(context) })
                }),
                metadata,
            )
            .await?;

        // Register for record updates
        let hook_update = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "user_validation_update".to_string(),
            "User Validation (Update)".to_string(),
        )
        .with_description("Validate user data before updating records".to_string())
        .with_priority(50);

        event_bus
            .subscribe_before(
                BeforeEventType::RecordUpdate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_update);
                    Box::pin(async move { hook.handle_before_record_update(context) })
                }),
                metadata,
            )
            .await?;

        info!("✅ User validation hooks registered");
        Ok(())
    }

    /// Register authorization hooks
    async fn register_authorization_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let auth_service = self
            .auth_service
            .as_ref()
            .ok_or_else(|| AppError::internal("AuthService is required for authorization hooks"))?;

        let permission_service = self.permission_service.as_ref().ok_or_else(|| {
            AppError::internal("PermissionService is required for authorization hooks")
        })?;

        let hook = Arc::new(AuthorizationHook::new(
            Arc::clone(auth_service),
            Arc::clone(permission_service),
        ));

        // Initialize default permissions for system collections asynchronously
        hook.initialize_default_permissions().await?;

        // Register for API request authorization
        let hook_api = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "authorization_api".to_string(),
            "Authorization (API Request)".to_string(),
        )
        .with_description("Authorize API requests based on user permissions".to_string())
        .with_priority(200); // Very high priority for security

        event_bus
            .subscribe_before(
                BeforeEventType::ApiRequest.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_api);
                    Box::pin(async move { hook.handle_before_api_request(context).await })
                }),
                metadata,
            )
            .await?;

        info!("🔐 Authorization hooks registered");
        Ok(())
    }

    /// Register activity logging hooks
    async fn register_activity_logging_hooks(
        &self,
        event_bus: &dyn EventBus,
    ) -> Result<(), AppError> {
        let hook = Arc::new(ActivityLoggerHook::new());

        // Register for before events
        let hook_before = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "activity_logging".to_string(),
            "Activity Logging".to_string(),
        )
        .with_description("Log activity events for audit trail".to_string())
        .with_priority(10); // Low priority - runs after main logic

        event_bus
            .subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_before);
                    Box::pin(async move {
                        hook.handle_before_event(BeforeEventType::RecordCreate.name(), context)
                    })
                }),
                metadata,
            )
            .await?;

        info!("📋 Activity logging hooks registered");
        Ok(())
    }

    /// Register security audit hooks
    async fn register_security_audit_hooks(
        &self,
        event_bus: &dyn EventBus,
    ) -> Result<(), AppError> {
        let hook = Arc::new(SecurityAuditHook::new());

        // Register for before events to monitor security
        let hook_before = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "security_audit".to_string(),
            "Security Audit".to_string(),
        )
        .with_description("Monitor and audit security-related events".to_string())
        .with_priority(150); // High priority for security monitoring

        event_bus
            .subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_before);
                    Box::pin(async move {
                        hook.handle_before_event(BeforeEventType::RecordCreate.name(), context)
                    })
                }),
                metadata,
            )
            .await?;

        info!("🔐 Security audit hooks registered");
        Ok(())
    }

    /// Register schema validation hooks
    async fn register_schema_validation_hooks(
        &self,
        event_bus: &dyn EventBus,
    ) -> Result<(), AppError> {
        let hook = Arc::new(SchemaValidatorHook::new());

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "schema_validation".to_string(),
            "Schema Validation".to_string(),
        )
        .with_description("Validate data against collection schema".to_string())
        .with_priority(80); // High priority for data integrity

        event_bus
            .subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_create);
                    Box::pin(async move { hook.handle_before_record_create(context) })
                }),
                metadata,
            )
            .await?;

        info!("📝 Schema validation hooks registered");
        Ok(())
    }

    /// Register data sanitization hooks
    async fn register_data_sanitization_hooks(
        &self,
        event_bus: &dyn EventBus,
    ) -> Result<(), AppError> {
        let hook = Arc::new(DataSanitizerHook::new().map_err(|e| {
            AppError::internal(format!("Failed to create data sanitizer hook: {}", e))
        })?);

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "data_sanitizer_create".to_string(),
            "Data Sanitizer (Create)".to_string(),
        )
        .with_description("Sanitize input data before record creation".to_string())
        .with_priority(60);

        event_bus
            .subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_create);
                    Box::pin(async move { hook.handle_before_record_create(context) })
                }),
                metadata,
            )
            .await?;

        // Register for record updates
        let hook_update = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "data_sanitizer_update".to_string(),
            "Data Sanitizer (Update)".to_string(),
        )
        .with_description("Sanitize input data before record updates".to_string())
        .with_priority(60);

        event_bus
            .subscribe_before(
                BeforeEventType::RecordUpdate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_update);
                    Box::pin(async move { hook.handle_before_record_update(context) })
                }),
                metadata,
            )
            .await?;

        info!("🧹 Data sanitization hooks registered");
        Ok(())
    }

    /// Register auth schema validation hooks
    async fn register_auth_schema_validation_hooks(
        &self,
        event_bus: &dyn EventBus,
    ) -> Result<(), AppError> {
        let hook = Arc::new(AuthSchemaValidatorHook::new());

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        let metadata = crate::event::HandlerMetadata::new(
            "auth_schema_validation".to_string(),
            "Auth Schema Validation".to_string(),
        )
        .with_description("Validate data against auth schema".to_string())
        .with_priority(70); // High priority for data integrity

        event_bus
            .subscribe_before(
                BeforeEventType::RecordCreate.name(),
                Arc::new(move |context| {
                    let hook = Arc::clone(&hook_create);
                    Box::pin(async move { hook.handle_before_record_create(context) })
                }),
                metadata,
            )
            .await?;

        info!("📝 Auth schema validation hooks registered");
        Ok(())
    }
}

/// Convenience function to register all system hooks with default configuration
pub async fn register_system_hooks(
    event_bus: &dyn EventBus,
    auth_service: Arc<AuthService>,
    permission_service: Arc<dyn PermissionService>,
) -> Result<(), AppError> {
    let registry = HookRegistry::new()
        .with_auth_service(auth_service)
        .with_permission_service(permission_service);
    registry.register_all_hooks(event_bus).await
}
