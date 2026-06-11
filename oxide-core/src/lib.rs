//! OxideDB Core Library
//!
//! This crate contains the core data structures, traits, and interfaces
//! that define the OxideDB architecture. It provides:
//!
//! - Event system for hook-first architecture
//! - Plugin API contracts
//! - Authentication services
//! - Collection schemas and data validation
//! - Standardized error handling
//! - System hooks for common functionality
//!
//! This crate is designed to be database-agnostic and runtime-agnostic,
//! containing only shared abstractions and contracts.

pub mod auth;
pub mod collection;
pub mod dashboard;
pub mod error;
pub mod event;
pub mod field_types;
pub mod hooks;
pub mod logging;
pub mod plugin_api;
pub mod plugin_config;
pub mod plugin_security;
pub mod site_settings;
pub mod user_preferences;
pub mod vfs;

// Re-export commonly used types for convenience
pub use auth::{
    AuthService, Claims, CollectionPermissions, CrudOperation, OperationRule, PermissionContext,
    PermissionLevel, UserRole,
};
pub use collection::{CollectionSchema, CollectionType, FieldDefinition};
pub use dashboard::{
    ActivityEntry, ActivityType, ApiStats, CollectionStatsEntry, DashboardStats,
    DashboardStatsService, EndpointStats, GrowthTrends, HealthStatus, StorageUsage, SystemHealth,
    SystemStats, UserActivity, UserStats,
};
pub use error::AppError;
pub use event::{
    AfterEventContext,
    AfterEventHandler,
    AfterEventType,
    // Event system core types
    BeforeEventContext,
    BeforeEventHandler,
    BeforeEventType,
    BusConfig as EventBusConfig,
    CircuitBreakerMiddleware,
    CompositeAfterMiddleware,
    CompositeBeforeMiddleware,
    EventBus,
    EventBusHealth,
    EventFilter,
    EventMetrics,
    // Configuration and utilities
    EventSystemConfig,
    HandlerExecutionResult,
    HandlerMetadata,
    InMemoryEventBus,
    RetryMiddleware,
    // Middleware
    TimeoutMiddleware,
};
pub use field_types::FieldType;
pub use logging::{
    ApplicationLogger, AuditEventType, CorrelationIdTrait, LogContext, LogLevel, LoggingMetrics,
    LoggingResult, LoggingService, NoOpLogger, SecurityAuditor,
};
pub use plugin_api::{EventPayload, PluginError, PluginResponse, PluginResult};
pub use plugin_security::{
    ExecutionStats, PluginCapability, PluginSecurityContext, PluginSecurityManager,
    PluginTrustLevel, ResourceLimits, SecurityPolicies, SecurityViolation,
};
pub use site_settings::{
    settings_sections, BackupSettings, BrandingSettings, DeploymentEnvironment, EmailSettings,
    EmailTemplateSettings, GeneralSettings, MaintenanceSettings, OxideDbEdition, SecuritySettings,
    SettingsHealthStatus, SiteSettings, SiteSettingsResponse, SiteSettingsService,
    SystemInfoSettings, SystemInfoUpdateRequest, UpdateSiteSettingsRequest,
};
pub use vfs::{
    FileId, FileIdentifier, FileListRequest, FileListResponse, FileMetadata, FileReadRequest,
    FileReadResponse, FileWriteRequest, NoOpVfs, VfsBackupConfig, VfsError, VfsNamespace,
    VfsNamespaceConfig, VfsPath, VfsResult, VfsServiceBridge, VfsUsageStats, VirtualFileSystem,
};

// Re-export the main hook registration function
pub use hooks::register_system_hooks;
