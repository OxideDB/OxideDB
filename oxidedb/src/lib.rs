//! OxideDB Main Binary Crate
//!
//! This crate contains the main application binary and the Wasmtime-based
//! plugin runtime implementation.

pub mod plugin_runtime;

pub use plugin_runtime::WasmtimePluginRuntime;
