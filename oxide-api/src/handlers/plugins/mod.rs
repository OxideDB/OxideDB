//! Plugin HTTP Route Handlers
//!
//! This module provides HTTP handlers for plugin-registered routes,
//! including authorization and security validation.

pub mod analysis;
pub mod audit;
pub mod capabilities;
pub mod installation;
pub mod management;
pub mod permissions;
pub mod routes;
pub mod types;

// Re-export commonly used types and functions
pub use analysis::*;
pub use audit::*;
pub use capabilities::*;
pub use installation::*;
pub use management::*;
pub use permissions::*;
pub use routes::*;
pub use types::*; 