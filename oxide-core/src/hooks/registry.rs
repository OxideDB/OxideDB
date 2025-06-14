//! Hook Registry
//!
//! This module provides a centralized registration system for all system hooks.
//! It allows for organized configuration and registration of hooks with the event bus.

use crate::{
    EventBus, AuthService, AppError, BeforeEventType,
    hooks::{
        auth::{PasswordHashingHook, UserValidationHook},
        audit::{ActivityLoggerHook, SecurityAuditHook},
        validation::{SchemaValidatorHook, DataSanitizerHook},
    }
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
    /// Enable activity logging hooks
    pub enable_activity_logging: bool,
    /// Enable security audit hooks
    pub enable_security_audit: bool,
    /// Enable schema validation hooks
    pub enable_schema_validation: bool,
    /// Enable data sanitization hooks
    pub enable_data_sanitization: bool,
}

impl Default for HookRegistryConfig {
    fn default() -> Self {
        Self {
            enable_password_hooks: true,
            enable_user_validation: true,
            enable_activity_logging: true,
            enable_security_audit: true,
            enable_schema_validation: false, // Disabled by default as it requires schema setup
            enable_data_sanitization: true,
        }
    }
}

/// Hook registry for centralized hook management
pub struct HookRegistry {
    config: HookRegistryConfig,
    auth_service: Option<Arc<AuthService>>,
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
        }
    }

    /// Create a new hook registry with custom configuration
    pub fn with_config(config: HookRegistryConfig) -> Self {
        Self {
            config,
            auth_service: None,
        }
    }

    /// Set the auth service for hooks that require it
    pub fn with_auth_service(mut self, auth_service: Arc<AuthService>) -> Self {
        self.auth_service = Some(auth_service);
        self
    }

    /// Register all enabled system hooks with the event bus
    pub fn register_all_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        info!("🎣 Registering system hooks...");

        let mut registered_count = 0;

        // Register authentication hooks
        if self.config.enable_password_hooks {
            self.register_password_hooks(event_bus)?;
            registered_count += 1;
        }

        if self.config.enable_user_validation {
            self.register_user_validation_hooks(event_bus)?;
            registered_count += 1;
        }

        // Register audit hooks
        if self.config.enable_activity_logging {
            self.register_activity_logging_hooks(event_bus)?;
            registered_count += 1;
        }

        if self.config.enable_security_audit {
            self.register_security_audit_hooks(event_bus)?;
            registered_count += 1;
        }

        // Register validation hooks
        if self.config.enable_schema_validation {
            self.register_schema_validation_hooks(event_bus)?;
            registered_count += 1;
        }

        if self.config.enable_data_sanitization {
            self.register_data_sanitization_hooks(event_bus)?;
            registered_count += 1;
        }

        info!("✅ Successfully registered {} hook categories", registered_count);
        Ok(())
    }

    /// Register password hashing hooks
    fn register_password_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let auth_service = self.auth_service.as_ref()
            .ok_or_else(|| AppError::internal("AuthService is required for password hooks"))?;

        let hook = Arc::new(PasswordHashingHook::new(Arc::clone(auth_service)));

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordCreate.name(),
            Box::new(move |context| hook_create.handle_before_record_create(context)),
        )?;

        // Register for record updates
        let hook_update = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordUpdate.name(),
            Box::new(move |context| hook_update.handle_before_record_update(context)),
        )?;

        info!("🔒 Password hashing hooks registered");
        Ok(())
    }

    /// Register user validation hooks
    fn register_user_validation_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let hook = Arc::new(UserValidationHook::new()
            .map_err(|e| AppError::internal(format!("Failed to create user validation hook: {}", e)))?);

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordCreate.name(),
            Box::new(move |context| hook_create.handle_before_record_create(context)),
        )?;

        // Register for record updates
        let hook_update = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordUpdate.name(),
            Box::new(move |context| hook_update.handle_before_record_update(context)),
        )?;

        info!("✅ User validation hooks registered");
        Ok(())
    }

    /// Register activity logging hooks
    fn register_activity_logging_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let hook = Arc::new(ActivityLoggerHook::new());

        // Register for before events
        let hook_before = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordCreate.name(),
            Box::new(move |context| {
                hook_before.handle_before_event(BeforeEventType::RecordCreate.name(), context)
            }),
        )?;

        info!("📋 Activity logging hooks registered");
        Ok(())
    }

    /// Register security audit hooks
    fn register_security_audit_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let hook = Arc::new(SecurityAuditHook::new());

        // Register for before events to monitor security
        let hook_before = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordCreate.name(),
            Box::new(move |context| {
                hook_before.handle_before_event(BeforeEventType::RecordCreate.name(), context)
            }),
        )?;

        info!("🔐 Security audit hooks registered");
        Ok(())
    }

    /// Register schema validation hooks
    fn register_schema_validation_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let hook = Arc::new(SchemaValidatorHook::new());

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordCreate.name(),
            Box::new(move |context| hook_create.handle_before_record_create(context)),
        )?;

        info!("📝 Schema validation hooks registered");
        Ok(())
    }

    /// Register data sanitization hooks
    fn register_data_sanitization_hooks(&self, event_bus: &dyn EventBus) -> Result<(), AppError> {
        let hook = Arc::new(DataSanitizerHook::new()
            .map_err(|e| AppError::internal(format!("Failed to create data sanitizer hook: {}", e)))?);

        // Register for record creation
        let hook_create = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordCreate.name(),
            Box::new(move |context| hook_create.handle_before_record_create(context)),
        )?;

        // Register for record updates
        let hook_update = Arc::clone(&hook);
        event_bus.subscribe_before(
            BeforeEventType::RecordUpdate.name(),
            Box::new(move |context| hook_update.handle_before_record_update(context)),
        )?;

        info!("🧹 Data sanitization hooks registered");
        Ok(())
    }
}

/// Convenience function to register all system hooks with default configuration
pub fn register_system_hooks(
    event_bus: &dyn EventBus,
    auth_service: Arc<AuthService>,
) -> Result<(), AppError> {
    let registry = HookRegistry::new().with_auth_service(auth_service);
    registry.register_all_hooks(event_bus)
} 