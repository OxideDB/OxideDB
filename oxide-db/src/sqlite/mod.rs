//! SQLite implementation module
//!
//! This module contains the SQLite implementation of the database interface,
//! organized into focused submodules for better maintainability.

mod auth;
mod collections;
mod connection;
mod operations;

pub use connection::SqliteDb; 