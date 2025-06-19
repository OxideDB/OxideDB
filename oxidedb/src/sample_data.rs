//! Sample Data Population
//!
//! This module provides functions to populate the database and logging system
//! with sample data for demonstration and testing purposes.

use crate::Result;
use oxide_core::{AppError, AuthService, LogContext, ApplicationLogger, SecurityAuditor};
use oxide_db::SqliteDb;
use oxide_logging::LogServiceBridge;
use std::sync::Arc;
use tracing::{info, warn};

/// Populate the database with sample data
pub async fn populate_database_samples(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
) -> Result<()> {
    info!("🚀 Auto-populating database with sample data...");

    // Get auth service configuration to determine available collections
    let auth_collections = database.list_auth_collections().await?;
    
    if auth_collections.is_empty() {
        warn!("No auth collections found. Auth system may not be properly configured.");
        return Ok(());
    }

    info!("Found {} auth collections: {:?}", 
        auth_collections.len(), 
        auth_collections.iter().map(|c| &c.name).collect::<Vec<_>>()
    );

    // Register sample users
    register_sample_superuser(database, auth_service, &auth_collections).await?;
    register_sample_user(database, auth_service, &auth_collections).await?;

    // Demonstrate authentication
    demonstrate_authentication(database, auth_service, &auth_collections).await?;

    info!("✅ Database sample data populated successfully");
    Ok(())
}

/// Register a sample superuser
async fn register_sample_superuser(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
    auth_collections: &[oxide_core::CollectionSchema],
) -> Result<()> {
    // Try to register a superuser in the "superusers" collection if it exists
    let superuser_collection = auth_collections.iter()
        .find(|c| c.name == "superusers")
        .map(|c| c.name.as_str())
        .unwrap_or("users"); // Fallback to users if superusers doesn't exist

    if let Some(superuser_config) = auth_service.config().get_auth_collection(superuser_collection) {
        if superuser_config.registration_enabled {
            let register_request = oxide_db::db::RegisterRequest {
                collection: superuser_collection.to_string(),
                identifier: "admin@example.com".to_string(),
                credential: "secure_password_123".to_string(),
                additional_data: Some(serde_json::json!({
                    "verified": true,
                    "name": "System Administrator",
                    "role": "superuser"
                })),
            };

            match database.register_user(register_request, &superuser_config).await {
                Ok(superuser_id) => {
                    info!("✅ Sample superuser registered with ID: {} in collection '{}'", 
                          superuser_id, superuser_collection);
                }
                Err(e) => {
                    warn!("Failed to register superuser: {} (this may be expected if user already exists)", e);
                }
            }
        } else {
            info!("Registration is disabled for collection '{}'", superuser_collection);
        }
    }

    Ok(())
}

/// Register a sample regular user
async fn register_sample_user(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
    auth_collections: &[oxide_core::CollectionSchema],
) -> Result<()> {
    let user_collection = auth_collections.iter()
        .find(|c| c.name == "users")
        .map(|c| c.name.as_str())
        .unwrap_or_else(|| auth_collections.first().map(|c| c.name.as_str()).unwrap_or("users"));

    if let Some(user_config) = auth_service.config().get_auth_collection(user_collection) {
        if user_config.registration_enabled {
            let register_request = oxide_db::db::RegisterRequest {
                collection: user_collection.to_string(),
                identifier: "user@example.com".to_string(),
                credential: "user_password_456".to_string(),
                additional_data: Some(serde_json::json!({
                    "verified": false,
                    "name": "Regular User",
                    "role": "user"
                })),
            };

            match database.register_user(register_request, &user_config).await {
                Ok(user_id) => {
                    info!("✅ Sample user registered with ID: {} in collection '{}'", 
                          user_id, user_collection);
                }
                Err(e) => {
                    warn!("Failed to register user: {} (this may be expected if user already exists)", e);
                }
            }
        }
    }

    Ok(())
}

/// Demonstrate authentication functionality
async fn demonstrate_authentication(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
    auth_collections: &[oxide_core::CollectionSchema],
) -> Result<()> {
    let superuser_collection = auth_collections.iter()
        .find(|c| c.name == "superusers")
        .map(|c| c.name.as_str())
        .unwrap_or("users");

    if let Some(superuser_config) = auth_service.config().get_auth_collection(superuser_collection) {
        let auth_request = oxide_db::db::AuthRequest {
            collection: superuser_collection.to_string(),
            identifier: "admin@example.com".to_string(),
            credential: "secure_password_123".to_string(),
        };

        match database.authenticate_user(auth_request, &superuser_config).await {
            Ok(auth_response) => {
                info!(
                    "✅ User authenticated. ID: {}, Collection: {}, Token starts with: {}...",
                    auth_response.user_id,
                    auth_response.auth_collection,
                    &auth_response.token[..20.min(auth_response.token.len())]
                );
            }
            Err(e) => {
                warn!("Authentication failed: {}", e);
            }
        }
    }

    Ok(())
}

/// Populate sample log data for demonstration
pub async fn populate_logging_samples(
    logging_service: &LogServiceBridge,
) -> Result<()> {
    info!("🚀 Populating sample log data...");

    // Create some sample log entries
    let context = LogContext::new()
        .with_user_id("admin@example.com")
        .with_client_ip("127.0.0.1")
        .with_user_agent("OxideDB-Demo/1.0");

    // Log system startup
    logging_service.log_system_event(
        "system_startup".to_string(),
        "OxideDB system started successfully".to_string(),
        context.clone(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log system event: {}", e)))?;

    // Log some authentication events
    logging_service.log_authentication(
        "admin@example.com".to_string(),
        "login".to_string(),
        "success".to_string(),
        context.clone(),
        Some(10), // Low risk
    ).await.map_err(|e| AppError::internal(format!("Failed to log auth event: {}", e)))?;

    // Log data access
    logging_service.log_data_access(
        "admin@example.com".to_string(),
        "users".to_string(),
        "read".to_string(),
        context.clone().with_collection("users"),
    ).await.map_err(|e| AppError::internal(format!("Failed to log data access: {}", e)))?;

    // Log configuration change
    logging_service.log_configuration_change(
        "admin@example.com".to_string(),
        "auth_settings".to_string(),
        "update_retention_policy".to_string(),
        context.clone(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log config change: {}", e)))?;

    // Log some informational messages
    logging_service.info(
        "Sample data population completed successfully".to_string(),
        "system".to_string(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log info: {}", e)))?;

    logging_service.warn(
        "This is a demonstration warning message".to_string(),
        "demo".to_string(),
    ).await.map_err(|e| AppError::internal(format!("Failed to log warning: {}", e)))?;

    // Log security audit events
    log_sample_security_events(logging_service, &context).await?;

    // Force flush all pending logs to ensure they're written to database
    logging_service.flush().await.map_err(|e| AppError::internal(format!("Failed to flush logs: {}", e)))?;

    info!("✅ Sample log data populated successfully");
    Ok(())
}

/// Log sample security events for demonstration
async fn log_sample_security_events(
    logging_service: &LogServiceBridge,
    context: &LogContext,
) -> Result<()> {
    // Log a failed authentication attempt
    logging_service.log_authentication(
        "unknown@example.com".to_string(),
        "login".to_string(),
        "failed".to_string(),
        context.clone().with_user_id("unknown@example.com"),
        Some(75), // Medium-high risk
    ).await.map_err(|e| AppError::internal(format!("Failed to log failed auth: {}", e)))?;

    // Log a permission denied event
    logging_service.log_security_violation(
        "user@example.com".to_string(),
        "permission_denied".to_string(),
        "Attempted to access admin endpoint".to_string(),
        context.clone().with_user_id("user@example.com"),
    ).await.map_err(|e| AppError::internal(format!("Failed to log security violation: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sample_data_functions_exist() {
        // These tests just ensure the functions compile and can be called
        // Real testing would require setting up a test database and logging system
        
        // Test that the functions exist and have the right signatures
        let _ = populate_database_samples;
        let _ = populate_logging_samples;
    }
} 