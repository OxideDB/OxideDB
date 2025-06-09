//! # OxideDB Core
//!
//! This crate contains the fundamental data structures, traits, and contracts
//! that define the core architecture of OxideDB. It provides the event system
//! that enables the hook-first architecture, standardized error handling,
//! and the plugin API contracts.

pub mod error;
pub mod event;
pub mod plugin_api;
pub mod collection;

pub use error::AppError;
pub use event::{Event, EventBus, InMemoryEventBus};
pub use plugin_api::{
    host_functions, plugin_exports, EventPayload, PluginError, PluginResponse, PluginResult,
    PluginRuntime,
};
pub use collection::{CollectionSchema, CollectionType, FieldDefinition, FieldType};
