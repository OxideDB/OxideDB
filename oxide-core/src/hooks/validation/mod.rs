//! Validation Hooks
//!
//! This module contains hooks for validating data before it's stored in the database.
//! These hooks ensure data integrity and consistency across the system.

pub mod schema_validator;
pub mod data_sanitizer;

pub use schema_validator::SchemaValidatorHook;
pub use data_sanitizer::DataSanitizerHook; 