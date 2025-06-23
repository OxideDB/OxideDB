//! Metadata Host Functions
//!
//! NOTE: These functions have been removed as OxideDB now relies solely on 
//! TOML-based metadata from plugin.toml files instead of embedded runtime metadata.

use std::sync::{Arc, Mutex};
use wasmtime::{Caller, Linker};
use tracing::debug;

use crate::host_state::HostState;

/// Define metadata-related host functions for the plugin linker
/// 
/// NOTE: Previously included set_plugin_metadata and get_plugin_metadata functions,
/// but these have been removed in favor of TOML-only metadata approach.
pub fn define_metadata_functions(linker: &mut Linker<Arc<Mutex<HostState>>>) -> anyhow::Result<()> {
    debug!("Metadata host functions disabled - using TOML-only metadata approach");
    
    // No metadata host functions are registered anymore
    // All plugin metadata comes from the plugin.toml file during plugin loading
    
    Ok(())
}