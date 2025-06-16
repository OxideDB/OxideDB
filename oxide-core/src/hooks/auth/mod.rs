//! Authentication Hooks
//!
//! This module contains all authentication-related hooks including
//! password hashing, user validation, authorization, and authentication events.

pub mod password_hash;
pub mod user_validation;
pub mod authorization;

pub use password_hash::PasswordHashingHook;
pub use user_validation::UserValidationHook;
pub use authorization::AuthorizationHook; 