//! Sample Data Population
//!
//! This module provides functions to populate the database and logging system
//! with sample data for demonstration and testing purposes.

use crate::Result;
use oxide_core::{
    event::context::RequestContext, AppError, ApplicationLogger, AuthService, LogContext,
    SecurityAuditor,
};
use oxide_db::{
    db::{AuthRequest, RegisterRequest},
    SqliteDb,
};
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

    info!(
        "Found {} auth collections: {:?}",
        auth_collections.len(),
        auth_collections.iter().map(|c| &c.name).collect::<Vec<_>>()
    );

    // Register sample users
    register_sample_superuser(database, auth_service, &auth_collections).await?;
    register_sample_user(database, auth_service, &auth_collections).await?;

    // Demonstrate authentication and permissions
    demonstrate_authentication(database, auth_service, &auth_collections).await?;

    info!("✅ Database sample data populated successfully");
    Ok(())
}

/// Register a sample superuser for testing
async fn register_sample_superuser(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
    auth_collections: &[oxide_core::CollectionSchema],
) -> Result<()> {
    // Try to register a superuser in the "_superusers" collection if it exists
    let superuser_collection = auth_collections
        .iter()
        .find(|c| c.name == "_superusers")
        .map(|c| c.name.as_str())
        .unwrap_or("_users"); // Fallback to _users if _superusers doesn't exist

    // Check if this auth collection exists and has the right config
    if let Some(auth_config) = auth_service
        .config()
        .get_auth_collection(superuser_collection)
    {
        info!(
            "📝 Registering sample superuser in collection '{}'",
            superuser_collection
        );

        let register_request = RegisterRequest {
            collection: superuser_collection.to_string(),
            identifier: "admin@oxide.rs".to_string(),
            credential: "admin123".to_string(),
            additional_data: Some(serde_json::json!({
                "name": "System Administrator"
            })),
        };

        match database.register_user(register_request, &auth_config).await {
            Ok(user_id) => {
                info!("✅ Sample superuser registered with ID: {}", user_id);
            }
            Err(e) if e.to_string().contains("already exists") => {
                info!("ℹ️ Sample superuser already exists");
            }
            Err(e) => {
                warn!("❌ Failed to register sample superuser: {}", e);
            }
        }
    } else {
        warn!("No superuser auth collection found");
    }

    Ok(())
}

/// Register a sample user for testing
async fn register_sample_user(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
    auth_collections: &[oxide_core::CollectionSchema],
) -> Result<()> {
    let user_collection = auth_collections
        .iter()
        .find(|c| c.name == "_users")
        .map(|c| c.name.as_str())
        .unwrap_or_else(|| {
            auth_collections
                .first()
                .map(|c| c.name.as_str())
                .unwrap_or("_users")
        });

    // Check if this auth collection exists and has the right config
    if let Some(auth_config) = auth_service.config().get_auth_collection(user_collection) {
        info!(
            "📝 Registering sample user in collection '{}'",
            user_collection
        );

        let register_request = RegisterRequest {
            collection: user_collection.to_string(),
            identifier: "user@oxide.rs".to_string(),
            credential: "user123".to_string(),
            additional_data: Some(serde_json::json!({
                "name": "Sample User"
            })),
        };

        match database.register_user(register_request, &auth_config).await {
            Ok(user_id) => {
                info!("✅ Sample user registered with ID: {}", user_id);
            }
            Err(e) if e.to_string().contains("already exists") => {
                info!("ℹ️ Sample user already exists");
            }
            Err(e) => {
                warn!("❌ Failed to register sample user: {}", e);
            }
        }
    } else {
        warn!("No user auth collection found");
    }

    Ok(())
}

/// Demonstrate authentication functionality
async fn demonstrate_authentication(
    database: &Arc<SqliteDb>,
    auth_service: &Arc<AuthService>,
    auth_collections: &[oxide_core::CollectionSchema],
) -> Result<()> {
    let superuser_collection = auth_collections
        .iter()
        .find(|c| c.name == "_superusers")
        .map(|c| c.name.as_str())
        .unwrap_or("_users");

    if let Some(auth_config) = auth_service
        .config()
        .get_auth_collection(superuser_collection)
    {
        info!("🔐 Testing authentication for sample superuser");

        let auth_request = AuthRequest {
            collection: superuser_collection.to_string(),
            identifier: "admin@oxide.rs".to_string(),
            credential: "admin123".to_string(),
        };

        match database.authenticate_user(auth_request, &auth_config).await {
            Ok(auth_response) => {
                info!(
                    "✅ Superuser authentication successful! Token generated for user: {}",
                    auth_response.user_id
                );
                info!("   Role: {}", auth_response.role);
                info!("   Collection: {}", auth_response.auth_collection);
            }
            Err(e) => {
                warn!("❌ Failed to authenticate sample superuser: {}", e);
            }
        }
    }

    // Also test with a test collection creation to ensure everything is working
    info!("🗂️ Creating test collection to verify database functionality");
    create_test_collection(database).await?;

    Ok(())
}

/// Create a test collection
async fn create_test_collection(database: &Arc<SqliteDb>) -> Result<()> {
    let test_schema = oxide_core::CollectionSchema::new(
        "test_collection".to_string(),
        oxide_core::CollectionType::Base,
    );

    match database.create_collection_with_schema(test_schema).await {
        Ok(_) => {
            info!("✅ Test collection created successfully");
        }
        Err(e) if e.to_string().contains("already exists") => {
            info!("ℹ️ Test collection already exists");
        }
        Err(e) => {
            warn!("❌ Failed to create test collection: {}", e);
        }
    }

    // Test record creation context (just for demonstrating the new system)
    let test_record = serde_json::json!({
        "message": "Hello from OxideDB!",
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        "active": true
    });

    let _request_context = RequestContext::anonymous();
    let _create_context = oxide_core::event::context::BeforeEventContext::new_create(
        "_users".to_string(),
        test_record,
    );

    info!("📝 Sample data setup complete! You can now:");
    info!("   - Login as admin@oxide.rs with password 'admin123'");
    info!("   - Login as user@oxide.rs with password 'user123'");
    info!("   - Use the API to manage collections and records");

    Ok(())
}

/// Populate sample log data for demonstration
pub async fn populate_logging_samples(logging_service: &LogServiceBridge) -> Result<()> {
    info!("🚀 Populating sample log data...");

    // Create some sample log entries
    let context = LogContext::new()
        .with_user_id("admin@example.com")
        .with_client_ip("127.0.0.1")
        .with_user_agent("OxideDB-Demo/1.0");

    // Log system startup
    logging_service
        .log_system_event(
            "system_startup".to_string(),
            "OxideDB system started successfully".to_string(),
            context.clone(),
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log system event: {}", e)))?;

    // Log some authentication events
    logging_service
        .log_authentication(
            "admin@example.com".to_string(),
            "login".to_string(),
            "success".to_string(),
            context.clone(),
            Some(10), // Low risk
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log auth event: {}", e)))?;

    // Log data access
    logging_service
        .log_data_access(
            "admin@example.com".to_string(),
            "users".to_string(),
            "read".to_string(),
            context.clone().with_collection("users"),
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log data access: {}", e)))?;

    // Log configuration change
    logging_service
        .log_configuration_change(
            "admin@example.com".to_string(),
            "auth_settings".to_string(),
            "update_retention_policy".to_string(),
            context.clone(),
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log config change: {}", e)))?;

    // Log some informational messages
    logging_service
        .info(
            "Sample data population completed successfully".to_string(),
            "system".to_string(),
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log info: {}", e)))?;

    logging_service
        .warn(
            "This is a demonstration warning message".to_string(),
            "demo".to_string(),
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log warning: {}", e)))?;

    // Log security audit events
    log_sample_security_events(logging_service, &context).await?;

    // Force flush all pending logs to ensure they're written to database
    logging_service
        .flush()
        .await
        .map_err(|e| AppError::internal(format!("Failed to flush logs: {}", e)))?;

    info!("✅ Sample log data populated successfully");
    Ok(())
}

/// Log sample security events for demonstration
async fn log_sample_security_events(
    logging_service: &LogServiceBridge,
    context: &LogContext,
) -> Result<()> {
    // Log a failed authentication attempt
    logging_service
        .log_authentication(
            "unknown@example.com".to_string(),
            "login".to_string(),
            "failed".to_string(),
            context.clone().with_user_id("unknown@example.com"),
            Some(75), // Medium-high risk
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log failed auth: {}", e)))?;

    // Log a permission denied event
    logging_service
        .log_security_violation(
            "user@example.com".to_string(),
            "permission_denied".to_string(),
            "Attempted to access admin endpoint".to_string(),
            context.clone().with_user_id("user@example.com"),
        )
        .await
        .map_err(|e| AppError::internal(format!("Failed to log security violation: {}", e)))?;

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
