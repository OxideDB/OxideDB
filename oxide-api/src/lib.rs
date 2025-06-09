//! # OxideDB API Layer
//!
//! This crate provides the HTTP API server for OxideDB. It handles REST API
//! requests and translates them into database operations through the database
//! abstraction layer. All operations are routed through the event system to
//! maintain the hook-first architecture.

pub mod handlers;
pub mod middleware;
pub mod server;

pub use server::ApiServer;
