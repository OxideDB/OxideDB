//! HTTP request handlers organized by domain
//!
//! This module contains handler functions for various API endpoints,
//! organized by functional domain. Each handler translates HTTP requests
//! into database operations and returns appropriate HTTP responses.
//!
//! ## Handler Organization
//!
//! - [`health`] - Health check and system status handlers
//! - [`collections`] - Collection management handlers
//! - [`records`] - Record CRUD operation handlers
//! - [`admin`] - Admin UI and static file handlers
//!
//! All handlers follow the hook-first architecture principle, ensuring
//! that operations are properly dispatched through the event system.

pub mod admin;
pub mod collections;
pub mod health;
pub mod records;

// Re-export commonly used types for convenience
pub use collections::{CollectionStats, CollectionHandlers};
pub use health::{HealthHandlers, HealthStatus};
pub use records::RecordHandlers; 