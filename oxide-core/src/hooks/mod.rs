//! System Hooks Module
//!
//! This module contains all built-in system hooks organized by functionality.
//! Hooks are the core mechanism for extending OxideDB's behavior through
//! the event system.
//!
//! # Organization
//! - `auth/` - Authentication and authorization hooks
//! - `validation/` - Data validation hooks  
//! - `audit/` - Auditing and logging hooks
//! - `cache/` - Caching hooks
//! - `registry.rs` - Centralized hook registration system
//!
//! # Hook Design Principles
//! 1. Single Responsibility: Each hook has one clear purpose
//! 2. Composable: Hooks can be combined and configured
//! 3. Testable: All hooks are unit testable
//! 4. Reusable: Hooks can be used across different contexts
//! 5. Configurable: Hooks accept configuration parameters

pub mod audit;
pub mod auth;
pub mod registry;
pub mod validation;

// Re-export the main registration function for convenience
pub use registry::register_system_hooks;
