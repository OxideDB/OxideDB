//! Command Handlers
//!
//! This module contains the implementation of CLI commands for OxideDB,
//! providing clean separation between CLI parsing and business logic.

use crate::{config::{RegisterSuperuserArgs, StartArgs}, startup::ApplicationBootstrap, OxideDbConfig, Result};
use oxide_api::services::DatabasePermissionService;
use oxide_core::{AppError, AuthService, EventBus, InMemoryEventBus, register_system_hooks};
use oxide_db::SqliteDb;
use std::sync::Arc;
use tracing::{error, info};

/// Trait for command handlers to enable modular command processing
pub trait CommandHandler {
    type Args;
    fn execute(args: Self::Args) -> impl std::future::Future<Output = Result<()>> + Send;
}

/// Handler for the start server command
pub struct StartCommand;

impl CommandHandler for StartCommand {
    type Args = StartArgs;

    async fn execute(args: Self::Args) -> Result<()> {
        // Create and validate configuration
        let config = OxideDbConfig::from_start_args(&args);
        config.validate()?;

        // Initialize application
        let bootstrap = ApplicationBootstrap::new(config);
        let services = bootstrap.initialize().await?;

        // Start server (this will run indefinitely)
        bootstrap.start_server(services).await
    }
}

/// Handler for the register superuser command
pub struct RegisterSuperuserCommand;

impl CommandHandler for RegisterSuperuserCommand {
    type Args = RegisterSuperuserArgs;

    async fn execute(args: Self::Args) -> Result<()> {
        info!("Registering superuser account");
        info!("Database path: {:?}", args.db_path);
        info!("Email: {}", args.email);

        // Initialize minimal services needed for user registration
        let services = Self::initialize_minimal_services(&args).await?;

        // Register the superuser
        Self::register_superuser(&args, &services).await?;

        info!("✅ Superuser registration completed successfully");
        Ok(())
    }
}

impl RegisterSuperuserCommand {
    /// Initialize minimal services needed for superuser registration
    async fn initialize_minimal_services(args: &RegisterSuperuserArgs) -> Result<MinimalServices> {
        let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
        
        let jwt_secret = std::env::var("JWT_SECRET")
            .unwrap_or_else(|_| "dev_secret_key_change_in_production".to_string());
        let auth_config = oxide_core::auth::AuthServiceConfig::new(jwt_secret);
        let auth_service = Arc::new(AuthService::new(auth_config));

        // Resolve database path
        let database_path = Self::resolve_database_path(&args.db_path)?;

        let database = Arc::new(SqliteDb::new(
            &database_path,
            Arc::clone(&event_bus),
            Arc::clone(&auth_service),
        )?);
        database.initialize().await?;

        // Update auth service with discovered collections
        let auth_collections = database.list_auth_collections().await?;
        auth_service.update_auth_collections(&auth_collections);

        // Create permission service for authorization hooks
        let permission_service = Arc::new(DatabasePermissionService::new(
            Arc::clone(&database) as Arc<dyn oxide_db::Db>
        ));

        // Register all system hooks using the new centralized system
        // This is CRITICAL for password hashing to work properly!
        register_system_hooks(
            event_bus.as_ref(), 
            Arc::clone(&auth_service),
            Arc::clone(&permission_service) as Arc<dyn oxide_core::auth::PermissionService>
        ).await?;
        info!("✅ System hooks registered for password hashing and validation");

        Ok(MinimalServices {
            database,
            auth_service,
        })
    }

    /// Resolve the database path, handling directory vs file paths
    fn resolve_database_path(db_path: &std::path::Path) -> Result<String> {
        // Ensure the database directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
        }

        let database_path = if db_path.is_dir() {
            db_path.join("oxidedb.sqlite").to_string_lossy().to_string()
        } else {
            // Ensure parent directory exists for file path
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| AppError::internal(format!("Failed to create database directory: {}", e)))?;
            }
            db_path.to_string_lossy().to_string()
        };

        Ok(database_path)
    }

    /// Register the superuser with the given arguments
    async fn register_superuser(args: &RegisterSuperuserArgs, services: &MinimalServices) -> Result<()> {
        // Find superuser collection or use default
        let auth_collections = services.database.list_auth_collections().await?;
        let superuser_collection = auth_collections.iter()
            .find(|c| c.name == "_superusers")
            .map(|c| c.name.as_str())
            .unwrap_or("_users");

        if let Some(superuser_config) = services.auth_service.config().get_auth_collection(superuser_collection) {
            let register_request = oxide_db::db::RegisterRequest {
                collection: superuser_collection.to_string(),
                identifier: args.email.clone(),
                credential: args.password.clone(),
                additional_data: Some(serde_json::json!({
                    "verified": true,
                    "name": args.name.clone().unwrap_or_else(|| "System Administrator".to_string()),
                    "role": "superuser"
                })),
            };

            match services.database.register_user(register_request, &superuser_config).await {
                Ok(user_id) => {
                    info!("✅ Superuser registered successfully with ID: {} in collection '{}'", 
                          user_id, superuser_collection);
                }
                Err(e) => {
                    error!("❌ Failed to register superuser: {}", e);
                    return Err(e);
                }
            }
        } else {
            return Err(AppError::internal(format!("Auth collection '{}' not found", superuser_collection)));
        }

        Ok(())
    }
}

/// Minimal services needed for superuser registration
struct MinimalServices {
    database: Arc<SqliteDb>,
    auth_service: Arc<AuthService>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_resolve_database_path_file() {
        let result = RegisterSuperuserCommand::resolve_database_path(&PathBuf::from("test.db"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test.db");
    }

    #[test]
    fn test_resolve_database_path_directory() {
        let temp_dir = std::env::temp_dir().join("oxidedb_test");
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let result = RegisterSuperuserCommand::resolve_database_path(&temp_dir);
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("oxidedb.sqlite"));
        
        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
} 