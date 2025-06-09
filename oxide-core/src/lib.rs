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
pub mod plugin_api;
pub mod hooks;

// Re-export commonly used types for convenience
pub use auth::{AuthService, UserRole};
pub use collection::{CollectionSchema, CollectionType, FieldDefinition, FieldType};
pub use error::AppError;
pub use event::{
    // New event system
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType,
    BeforeEventHandler, AfterEventHandler, EventBus, InMemoryEventBus,
    // Legacy compatibility
    Event, EventHandler,
};
pub use plugin_api::{PluginError, PluginResult, EventPayload, PluginResponse};

// Re-export the main hook registration function
pub use hooks::register_system_hooks;
