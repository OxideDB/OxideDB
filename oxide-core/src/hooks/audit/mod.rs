//! Audit Hooks
//!
//! This module contains hooks for auditing and logging system activities.
//! These hooks help track changes, security events, and system operations.

pub mod activity_logger;
pub mod security_audit;

pub use activity_logger::ActivityLoggerHook;
pub use security_audit::SecurityAuditHook;
