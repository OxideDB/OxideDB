//! # OxideDB API Server
//!
//! This crate provides the HTTP REST API server for OxideDB, built with Axum.
//! It offers CRUD operations, authentication, and a web-based admin interface.
//!
//! ## Features
//!
//! - RESTful API endpoints for collections and records
//! - JWT-based authentication system  
//! - Permission-based access control
//! - Web-based admin interface
//! - Event-driven hook system integration
//! - Comprehensive error handling and validation

pub mod errors;
pub mod extractors;
pub mod handlers;
pub mod middleware;
pub mod responses;
pub mod routes;
pub mod server;
pub mod services;

pub use server::{create_app, AppState};
pub use errors::ApiError;
pub use responses::ApiResponse;
