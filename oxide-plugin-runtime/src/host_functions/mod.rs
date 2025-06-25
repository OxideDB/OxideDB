//! Host Functions Module
//!
//! This module contains all the host functions that plugins can call,
//! organized by functionality for better maintainability.

pub mod database;
pub mod event;
pub mod http;
pub mod logging;
pub mod vfs;

pub use database::*;
pub use event::*;
pub use http::*;
pub use logging::*;
pub use vfs::*; 