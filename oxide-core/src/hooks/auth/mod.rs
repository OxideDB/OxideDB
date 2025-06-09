//! Authentication Hooks
//!
//! This module contains all authentication-related hooks including
//! password hashing, user validation, and authentication events.

pub mod password_hash;
pub mod user_validation;

pub use password_hash::PasswordHashingHook;
pub use user_validation::UserValidationHook; 