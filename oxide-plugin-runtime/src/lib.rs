//! OxideDB Plugin Runtime
//!
//! This crate provides the runtime environment for executing WebAssembly plugins
//! within the OxideDB system. It includes the plugin runtime implementation,
//! host functions for plugin-host communication, and security management.

pub mod factory;
pub mod host_functions;
pub mod host_state;
pub mod manager;
pub mod runtime;
pub mod utils;

pub use factory::*;
pub use manager::*;
pub use runtime::*;

/// Plugin runtime version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
