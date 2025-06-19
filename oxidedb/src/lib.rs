//! OxideDB Main Binary Crate
//!
//! This crate contains the main application binary and the Wasmtime-based
//! plugin runtime implementation.

pub mod config;
pub mod startup;
pub mod commands;
pub mod plugin_integration;
pub mod plugin_runtime;
pub mod sample_data;

// Re-export key types for external use
pub use config::{ServerConfig, SecurityPolicy, AdminMode, LogLevel, OxideDbConfig};
pub use startup::ApplicationBootstrap;
pub use commands::{StartCommand, RegisterSuperuserCommand, CommandHandler};
pub use plugin_integration::{PluginEventBridge, PluginManager};
pub use plugin_runtime::WasmtimePluginRuntime;

use oxide_core::AppError;

/// Application result type for consistent error handling
pub type Result<T> = std::result::Result<T, AppError>;

/// Application version info
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
