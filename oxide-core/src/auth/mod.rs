//! Authentication and authorization system
//!
//! This module provides the core authentication and authorization functionality
//! for OxideDB, including JWT token management, password hashing, and permission
//! checking. It supports multiple authentication methods and collection-based
//! authentication configuration.
//!
//! ## Module Organization
//!
//! The authentication system is organized into several focused modules:
//!
//! - [`types`] - Core types, enums, and configuration structures
//! - [`jwt`] - JWT token management and claims handling
//! - [`password`] - Secure password hashing with Argon2
//! - [`permissions`] - Permission system and access control
//! - [`rules`] - Rule evaluation engine for custom permission expressions
//! - [`service`] - Main authentication service orchestrating all functionality
//! - [`legacy`] - Backward compatibility support for existing installations
//!
//! ## Quick Start
//!
//! ```rust
//! use oxide_core::auth::{AuthService, AuthServiceConfig, UserRole};
//! # use oxide_core::AppError;
//! # fn main() -> Result<(), AppError> {
//!
//! // Create auth service
//! let config = AuthServiceConfig::new("your-jwt-secret".to_string());
//! let auth_service = AuthService::new(config);
//!
//! // Hash a password
//! let hashed = auth_service.hash_password("user_password")?;
//!
//! // Generate a JWT token
//! let token = auth_service.generate_token(
//!     "user123".to_string(),
//!     "user@example.com".to_string(),
//!     UserRole::User,
//!     "users".to_string()
//! )?;
//!
//! // Verify a token
//! let claims = auth_service.verify_token(&token)?;
//! # Ok(())
//! # }
//! ```

pub mod types;
pub mod jwt;
pub mod password;
pub mod permissions;
pub mod rules;
pub mod service;

// Re-export commonly used types for convenience
pub use types::{
    UserRole, AuthMethod, AuthCollectionConfig, AuthServiceConfig,
    CrudOperation, PermissionLevel
};

pub use jwt::{Claims, RefreshClaims, TokenPair, JwtService};
pub use password::PasswordService;
pub use permissions::{
    OperationRule, CollectionPermissions, PermissionContext, 
    PermissionService, DefaultPermissionService
};
pub use rules::RuleEvaluator;
pub use service::{AuthService, AuthTokens};

#[cfg(test)]
mod tests {
    use crate::auth::types::Operation;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_user_role_parsing() {
        assert_eq!("user".parse::<UserRole>().unwrap(), UserRole::User);
        assert_eq!("superuser".parse::<UserRole>().unwrap(), UserRole::Superuser);
        assert_eq!("admin".parse::<UserRole>().unwrap(), UserRole::Custom("admin".to_string()));
    }

    #[test]
    fn test_auth_service_config() {
        let config = AuthServiceConfig::new("secret".to_string());
        
        let auth_config = AuthCollectionConfig {
            collection: "users".to_string(),
            auth_method: AuthMethod::EmailPassword,
            identifier_field: "email".to_string(),
            credential_field: "password".to_string(),
            default_role: UserRole::User,
            registration_enabled: true,
            email_verification_required: false,
            custom_claim_fields: vec!["name".to_string()],
            refresh_tokens_enabled: false,
            refresh_tokens_required: false,
        };
        
        config.add_auth_collection(auth_config);
        
        assert!(config.is_auth_collection("users"));
        assert!(!config.is_auth_collection("posts"));
        assert_eq!(config.list_auth_collections(), vec!["users"]);
    }

    #[test]
    fn test_claims_creation() {
        let claims = Claims::new(
            "user123".to_string(),
            "test@example.com".to_string(),
            "user".to_string(),
            "users".to_string(),
            24,
        );
        
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.email, "test@example.com");
        assert_eq!(claims.role, "user");
        assert_eq!(claims.auth_collection, "users");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_rule_evaluator_boolean_literals() {
        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        );
        let evaluator = RuleEvaluator::new(&context);

        assert!(evaluator.evaluate("true").unwrap());
        assert!(!evaluator.evaluate("false").unwrap());
    }

    #[test]
    fn test_rule_evaluator_request_headers() {
        let mut metadata = std::collections::HashMap::new();
        let headers = json!({
            "x-api-key": "secret123",
            "authorization": "Bearer token123"
        });
        metadata.insert("headers".to_string(), headers);

        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        ).with_metadata(metadata);

        let evaluator = RuleEvaluator::new(&context);

        // Test header access
        assert!(evaluator.evaluate("@req.headers.x-api-key = 'secret123'").unwrap());
        assert!(!evaluator.evaluate("@req.headers.x-api-key = 'wrong'").unwrap());
        assert!(evaluator.evaluate("@req.headers.x-api-key != ''").unwrap());
    }

    #[test]
    fn test_rule_evaluator_header_case_sensitivity() {
        // Test with different header case variations as they would appear in real HTTP requests
        let mut metadata = std::collections::HashMap::new();
        let headers = json!({
            "X-API-Key": "your-secret-key",  // Mixed case as often sent by clients
            "Content-Type": "application/json",
            "authorization": "Bearer token123"
        });
        metadata.insert("headers".to_string(), headers);

        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        ).with_metadata(metadata);

        let evaluator = RuleEvaluator::new(&context);

        // The rule looks for "x-api-key" but header is "X-API-Key"
        // This should work due to lowercase fallback
        let result = evaluator.evaluate("@req.headers.x-api-key = 'your-secret-key'");
        println!("Rule evaluation result: {:?}", result);
        
        // Test the rule evaluation directly
        
        assert!(result.unwrap(), "Rule should match despite case difference");

        // Test existence check too
        assert!(evaluator.evaluate("@req.headers.x-api-key != ''").unwrap());
    }

    #[test]
    fn test_rule_evaluator_user_variables() {
        let claims = Claims::new(
            "user123".to_string(),
            "user@example.com".to_string(),
            "superuser".to_string(),
            "users".to_string(),
            24,
        );

        let context = PermissionContext::new(
            Some(claims),
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        );

        let evaluator = RuleEvaluator::new(&context);

        // Test user variable access
        assert!(evaluator.evaluate("@req.user.id = 'user123'").unwrap());
        assert!(evaluator.evaluate("@req.user.role = 'superuser'").unwrap());
        assert!(evaluator.evaluate("@req.user.email = 'user@example.com'").unwrap());
        assert!(!evaluator.evaluate("@req.user.id = 'wrong'").unwrap());
    }

    #[test]
    fn test_rule_evaluator_record_variables() {
        let record_data = json!({
            "user_id": "user123",
            "status": "active",
            "score": 85
        });

        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        ).with_record_data(record_data);

        let evaluator = RuleEvaluator::new(&context);

        // Test record variable access
        assert!(evaluator.evaluate("@record.user_id = 'user123'").unwrap());
        assert!(evaluator.evaluate("@record.status = 'active'").unwrap());
        assert!(!evaluator.evaluate("@record.user_id = 'wrong'").unwrap());
    }

    #[test]
    fn test_rule_evaluator_system_variables() {
        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        );

        let evaluator = RuleEvaluator::new(&context);

        // Test time-based rules (these will vary based on when test runs)
        let hour_rule = "@now.hour >= 0 && @now.hour <= 23";
        assert!(evaluator.evaluate(hour_rule).unwrap());

        let minute_rule = "@now.minute >= 0 && @now.minute <= 59";
        assert!(evaluator.evaluate(minute_rule).unwrap());
    }

    #[test]
    fn test_rule_evaluator_comparisons() {
        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        );

        let evaluator = RuleEvaluator::new(&context);

        // Test numeric comparisons
        assert!(evaluator.evaluate("10 > 5").unwrap());
        assert!(evaluator.evaluate("5 < 10").unwrap());
        assert!(evaluator.evaluate("10 >= 10").unwrap());
        assert!(evaluator.evaluate("5 <= 10").unwrap());
        assert!(!evaluator.evaluate("5 > 10").unwrap());
    }

    #[test]
    fn test_rule_evaluator_logical_operators() {
        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        );

        let evaluator = RuleEvaluator::new(&context);

        // Test logical operators
        assert!(evaluator.evaluate("true && true").unwrap());
        assert!(!evaluator.evaluate("true && false").unwrap());
        assert!(evaluator.evaluate("true || false").unwrap());
        assert!(!evaluator.evaluate("false || false").unwrap());
    }

    #[test]
    fn test_rule_evaluator_string_matching() {
        let mut metadata = std::collections::HashMap::new();
        let headers = json!({
            "x-forwarded-for": "192.168.1.100"
        });
        metadata.insert("headers".to_string(), headers);

        let context = PermissionContext::new(
            None,
            Operation::Crud(CrudOperation::Read),
            "test".to_string(),
            None,
        ).with_metadata(metadata);

        let evaluator = RuleEvaluator::new(&context);

        // Test pattern matching
        assert!(evaluator.evaluate("@req.headers.x-forwarded-for ~ '192.168.1.*'").unwrap());
        assert!(!evaluator.evaluate("@req.headers.x-forwarded-for ~ '10.0.0.*'").unwrap());
    }

    #[test]
    fn test_rule_evaluator_complex_rules() {
        let claims = Claims::new(
            "user123".to_string(),
            "user@example.com".to_string(),
            "user".to_string(),
            "users".to_string(),
            24,
        );

        let record_data = json!({
            "user_id": "user123",
            "status": "active"
        });

        let context = PermissionContext::new(
            Some(claims),
            Operation::Crud(CrudOperation::Update),
            "posts".to_string(),
            None,
        ).with_record_data(record_data);

        let evaluator = RuleEvaluator::new(&context);

        // Test owner access rule
        assert!(evaluator.evaluate("@req.user.id = @record.user_id").unwrap());
        
        // Test combined conditions
        assert!(evaluator.evaluate("@req.user.id = @record.user_id && @record.status = 'active'").unwrap());
        assert!(!evaluator.evaluate("@req.user.id = @record.user_id && @record.status = 'deleted'").unwrap());
    }
} 