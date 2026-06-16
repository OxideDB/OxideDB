//! Shared async-to-sync bridge for WASM host functions.
//!
//! Wasmtime host functions are synchronous, but the services they call (`Db`,
//! `Vfs`) expose async traits. This module provides the single bridge used by
//! both the database and VFS host-function modules to run an async operation
//! from within a sync host call without parking the tokio worker thread when
//! a multi-thread runtime is available, and with a dedicated fallback runtime
//! otherwise.
//!
//! Consolidating the previously duplicated `run_database_operation` /
//! `run_vfs_operation` helpers here keeps the bridging logic in one place.

use std::future::Future;
use std::sync::OnceLock;

use tokio::runtime::RuntimeFlavor;

/// Dedicated fallback runtime used when there is no multi-thread tokio
/// runtime in the current thread (e.g. tests on a current-thread runtime).
/// Lazily initialized on first use.
static PLUGIN_HOST_RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

fn plugin_host_runtime() -> Result<&'static tokio::runtime::Runtime, String> {
    PLUGIN_HOST_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_name("oxide-plugin-host")
                .build()
                .map_err(|e| format!("failed to initialize plugin host runtime: {}", e))
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// Run an async operation from within a synchronous WASM host function.
///
/// On a multi-thread tokio runtime this uses `block_in_place` so the blocking
/// work is scheduled off the worker without spawning a new OS thread. When no
/// multi-thread runtime is present, it falls back to the shared dedicated
/// runtime on a spawned thread.
///
/// Returns the operation's result, or an error string if the runtime could not
/// be acquired or the operation panicked.
pub fn run_host_operation<F, T>(operation: F) -> Result<T, String>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) if handle.runtime_flavor() == RuntimeFlavor::MultiThread => {
            Ok(tokio::task::block_in_place(|| handle.block_on(operation)))
        }
        _ => {
            let runtime = plugin_host_runtime()?;
            std::thread::spawn(move || runtime.block_on(operation))
                .join()
                .map_err(|_| "Plugin host operation thread panicked".to_string())
        }
    }
}
