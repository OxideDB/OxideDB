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
pub mod error;
pub mod event;
pub mod field_types;
pub mod logging;
pub mod vfs;
pub mod plugin_api;
pub mod plugin_security;
pub mod plugin_config;
pub mod hooks;

// Re-export commonly used types for convenience
pub use auth::{
    AuthService, UserRole, Claims, CrudOperation, PermissionLevel,
    OperationRule, CollectionPermissions, PermissionContext
};
pub use collection::{CollectionSchema, CollectionType, FieldDefinition};
pub use error::AppError;
pub use field_types::FieldType;
pub use event::{
    // Event system core types
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType,
    BeforeEventHandler, AfterEventHandler, EventBus, InMemoryEventBus,
    // Configuration and utilities
    EventSystemConfig, BusConfig as EventBusConfig, EventBusHealth, EventMetrics,
    HandlerMetadata, HandlerExecutionResult, EventFilter,
    // Middleware
    TimeoutMiddleware, RetryMiddleware, CircuitBreakerMiddleware,
    CompositeBeforeMiddleware, CompositeAfterMiddleware,
};
pub use logging::{
    ApplicationLogger, SecurityAuditor, LoggingService, LogLevel, AuditEventType,
    LogContext, LoggingMetrics, CorrelationIdTrait, NoOpLogger, LoggingResult
};
pub use vfs::{
    VirtualFileSystem, VfsServiceBridge, FileMetadata, VfsNamespaceConfig, VfsBackupConfig,
    FileWriteRequest, FileReadRequest, FileReadResponse, FileListRequest, FileListResponse,
    FileIdentifier, VfsUsageStats, VfsResult, VfsError, FileId, VfsPath, VfsNamespace, NoOpVfs
};
pub use plugin_api::{PluginError, PluginResult, EventPayload, PluginResponse};
pub use plugin_security::{
    PluginCapability, PluginSecurityContext, PluginTrustLevel, ResourceLimits,
    ExecutionStats, SecurityViolation, PluginSecurityManager, SecurityPolicies
};

// Re-export the main hook registration function
pub use hooks::register_system_hooks;
