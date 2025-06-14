//! # OxideDB API Layer
//!
//! This crate provides the HTTP API server for OxideDB. It handles REST API
//! requests and translates them into database operations through the database
//! abstraction layer. All operations are routed through the event system to
//! maintain the hook-first architecture.
//!
//! ## Architecture
//!
//! The API layer is organized into several key modules:
//!
//! - [`server`] - Core HTTP server implementation and configuration
//! - [`routes`] - Route definitions and HTTP endpoint mapping
//! - [`handlers`] - Request handlers organized by domain
//! - [`middleware`] - HTTP middleware for cross-cutting concerns
//! - [`responses`] - Standardized API response types
//! - [`errors`] - API-specific error handling and HTTP status mapping
//!
//! ## Usage
//!
//! ```rust,no_run
//! use oxide_api::ApiServer;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let server = ApiServer::new(
//!     db_instance,
//!     event_bus,
//!     "127.0.0.1".to_string(),
//!     8080
//! );
//!
//! server.start().await?;
//! # Ok(())
//! # }
//! ```

pub mod errors;
pub mod handlers;
pub mod middleware;
pub mod responses;
pub mod routes;
pub mod server;

// Re-export the main server type for convenience
pub use server::ApiServer;

// Re-export commonly used types
pub use errors::ApiError;
pub use responses::{ApiResponse, PaginatedResponse};
