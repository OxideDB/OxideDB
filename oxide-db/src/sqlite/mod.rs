//! SQLite implementation module
//!
//! This module contains the SQLite implementation of the database interface,
//! organized into focused submodules for better maintainability.

mod auth;
mod collections;
mod connection;
mod operations;
mod permissions;
mod relationships;
mod schema_adapter;
mod site_settings;
mod user_preferences;

pub use connection::SqliteDb;
pub use schema_adapter::SqliteSchemaAdapter;
