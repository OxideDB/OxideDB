//! # OxideDB Database Layer
//!
//! This crate provides database abstractions and implementations for OxideDB.
//! It defines the `Db` trait that specifies the interface for all database
//! operations and includes a SQLite implementation that integrates with the
//! event system.

pub mod dashboard_activity_listener;
pub mod dashboard_stats_service;
pub mod db;
pub mod record;
pub mod sqlite;

pub use dashboard_activity_listener::{
    register_dashboard_activity_listener, DashboardActivityListener,
};
pub use dashboard_stats_service::{
    AuthHealthBridge, AuthHealthProvider, DatabaseDashboardStatsService, LoggingStatsBridge,
    LoggingStatsProvider, PluginHealthProvider, VfsStatsBridge, VfsStatsProvider,
};
pub use db::Db;
pub use record::Record;
pub use sqlite::SqliteDb;
// Note: register_auth_listener is deprecated in favor of the system hooks in oxide-core

/// Simple alias for SqliteDb using in-memory database
pub type SimpleDb = SqliteDb;

/// Database layer version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
