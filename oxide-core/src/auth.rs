//! Authentication utilities for OxideDB
//!
//! This module provides password hashing and JWT token management
//! for the authentication system.

use crate::AppError;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::fmt;

/// JWT Claims structure
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,    // Subject (user id)
    pub email: String,  // User email
    pub exp: i64,       // Expiration time (timestamp)
    pub iat: i64,       // Issued at time (timestamp)
    pub role: String,   // User role (user or superuser)
}

impl Claims {
    /// Create new claims for a user
    pub fn new(user_id: String, email: String, role: String, expires_in_hours: i64) -> Self {
        let now = Utc::now();
        let exp = now + Duration::hours(expires_in_hours);
        
        Self {
            sub: user_id,
            email,
            exp: exp.timestamp(),
            iat: now.timestamp(),
            role,
        }
    }
}

/// User role enum
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    User,
    Superuser,
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::User => write!(f, "user"),
            UserRole::Superuser => write!(f, "superuser"),
        }
    }
}

impl std::str::FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(UserRole::User),
            "superuser" => Ok(UserRole::Superuser),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

/// Authentication service for password hashing and JWT management
pub struct AuthService {
    jwt_secret: String,
}

impl AuthService {
    /// Create a new AuthService with a JWT secret
    pub fn new(jwt_secret: String) -> Self {
        Self { jwt_secret }
    }

    /// Hash a password using Argon2
    pub fn hash_password(&self, password: &str) -> Result<String, AppError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| AppError::auth(format!("Failed to hash password: {}", e)))?;
        
        Ok(password_hash.to_string())
    }

    /// Verify a password against a hash
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AppError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| AppError::auth(format!("Invalid password hash: {}", e)))?;
        
        let argon2 = Argon2::default();
        match argon2.verify_password(password.as_bytes(), &parsed_hash) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Generate a JWT token for a user
    pub fn generate_token(&self, user_id: String, email: String, role: UserRole) -> Result<String, AppError> {
        let claims = Claims::new(user_id, email, role.to_string(), 24); // 24 hours expiry
        
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_ref()),
        )
        .map_err(|e| AppError::auth(format!("Failed to generate token: {}", e)))?;
        
        Ok(token)
    }

    /// Verify and decode a JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AppError> {
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_ref()),
            &Validation::new(Algorithm::HS256),
        )
        .map_err(|e| AppError::auth(format!("Invalid token: {}", e)))?;
        
        Ok(token_data.claims)
    }
}

/// Create default auth collections schemas
pub fn create_auth_collections() -> (crate::collection::CollectionSchema, crate::collection::CollectionSchema) {
    use crate::collection::{CollectionSchema, CollectionType, FieldDefinition};
use crate::field_types::FieldType;
    use std::collections::HashMap;
    
    // Users collection schema
    let mut users_schema = CollectionSchema::new("users".to_string(), CollectionType::Auth);
    let mut users_fields = HashMap::new();
    
    users_fields.insert("email".to_string(), FieldDefinition {
        field_type: FieldType::Email,
        required: true,
        unique: true, // Email must be unique
        default: None,
        validation: None,
    });
    
    users_fields.insert("password".to_string(), FieldDefinition {
        field_type: FieldType::Password,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    
    users_fields.insert("verified".to_string(), FieldDefinition {
        field_type: FieldType::Boolean,
        required: false,
        unique: false,
        default: Some(serde_json::json!(false)),
        validation: None,
    });
    
    users_schema.fields = users_fields;
    
    // Superusers collection schema
    let mut superusers_schema = CollectionSchema::new("superusers".to_string(), CollectionType::Auth);
    let mut superusers_fields = HashMap::new();
    
    superusers_fields.insert("email".to_string(), FieldDefinition {
        field_type: FieldType::Email,
        required: true,
        unique: true, // Email must be unique
        default: None,
        validation: None,
    });
    
    superusers_fields.insert("password".to_string(), FieldDefinition {
        field_type: FieldType::Password,
        required: true,
        unique: false,
        default: None,
        validation: None,
    });
    
    superusers_fields.insert("verified".to_string(), FieldDefinition {
        field_type: FieldType::Boolean,
        required: false,
        unique: false,
        default: Some(serde_json::json!(true)),
        validation: None,
    });
    
    superusers_schema.fields = superusers_fields;
    
    (users_schema, superusers_schema)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let auth_service = AuthService::new("test_secret".to_string());
        
        let password = "test_password_123";
        let hash = auth_service.hash_password(password).unwrap();
        
        assert!(auth_service.verify_password(password, &hash).unwrap());
        assert!(!auth_service.verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_jwt_tokens() {
        let auth_service = AuthService::new("test_secret".to_string());
        
        let user_id = "user123".to_string();
        let email = "test@example.com".to_string();
        let role = UserRole::User;
        
        let token = auth_service.generate_token(user_id.clone(), email.clone(), role).unwrap();
        let claims = auth_service.verify_token(&token).unwrap();
        
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.email, email);
        assert_eq!(claims.role, "user");
    }
} 