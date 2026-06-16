//! Password hashing and verification
//!
//! This module provides secure password hashing and verification using Argon2,
//! following security best practices for password storage.

use crate::AppError;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

/// Password service for hashing and verification
#[derive(Clone)]
pub struct PasswordService;

impl PasswordService {
    /// Create a new password service
    pub fn new() -> Self {
        Self
    }

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String, AppError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        argon2
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|e| AppError::internal(format!("Password hashing failed: {}", e)))
    }

    /// Verify a password against its hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::internal(format!("Invalid password hash: {}", e)))?;

        let argon2 = Argon2::default();
        Ok(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }
}

impl Default for PasswordService {
    fn default() -> Self {
        Self::new()
    }
}
