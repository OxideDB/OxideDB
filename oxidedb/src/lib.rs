//! OxideDB Main Binary Crate
//!
//! This crate contains the main application binary with plugin management.

pub mod commands;
pub mod config;
pub mod sample_data;
pub mod startup;

// Re-export key types for external use
pub use commands::{CommandHandler, RegisterSuperuserCommand, StartCommand};
pub use config::{AdminMode, LogLevel, OxideDbConfig, SecurityPolicy, ServerConfig};
pub use startup::ApplicationBootstrap;

// Re-export plugin runtime from the separate crate
pub use oxide_plugin_runtime::WasmtimePluginRuntime;

use oxide_core::AppError;

/// Application result type for consistent error handling
pub type Result<T> = std::result::Result<T, AppError>;

/// Application version info
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
