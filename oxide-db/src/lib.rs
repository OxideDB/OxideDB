//! # OxideDB Database Layer
//!
//! This crate provides database abstractions and implementations for OxideDB.
//! It defines the `Db` trait that specifies the interface for all database
//! operations and includes a SQLite implementation that integrates with the
//! event system.

pub mod db;
pub mod record;
pub mod sqlite;

pub use db::Db;
pub use record::Record;
pub use sqlite::SqliteDb;
