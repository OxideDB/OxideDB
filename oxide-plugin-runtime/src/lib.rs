//! Wasmtime-based Plugin Runtime for OxideDB
//!
//! This crate provides a Wasmtime-based implementation of the PluginRuntime trait
//! defined in oxide-core. It handles:
//! - Plugin loading and execution using Wasmtime
//! - Host function implementations for plugin communication
//! - Memory management for host-plugin data exchange
//! - Security and capability management
//! - Factory pattern for runtime creation
//! - High-level plugin management and event system integration

pub mod runtime;
pub mod host_state;
pub mod factory;
pub mod manager;

pub use runtime::WasmtimePluginRuntime;
pub use host_state::HostState;
pub use factory::{WasmtimePluginRuntimeFactory, WasmtimePluginRuntimeFactoryWithPolicies};
pub use manager::{PluginManager, PluginEventBridge, PluginStatistics, PluginStatus}; 