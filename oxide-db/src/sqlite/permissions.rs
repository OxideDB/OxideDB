//! Permission storage implementation for SQLite
//!
//! This module implements permission storage operations for the SQLite database.
//! Permissions are stored in a dedicated table and can be retrieved for authorization.

use super::SqliteDb;
use oxide_core::{AppError, CollectionPermissions};
use oxide_core::auth::{PermissionService, PermissionContext, PermissionLevel};
use tokio::task::spawn_blocking;
use tracing::{debug, info};

impl SqliteDb {
    /// Create the permissions table if it doesn't exist
    pub(super) async fn create_permissions_table(&self) -> Result<(), AppError> {
        let connection = self.connection.clone();
        
        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            conn.execute(
                r#"
                CREATE TABLE IF NOT EXISTS collection_permissions (
                    collection TEXT PRIMARY KEY,
                    permissions_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                )
                "#,
                [],
            )
            .map_err(|e| AppError::database(format!("Failed to create permissions table: {}", e)))?;

            debug!("✅ Collection permissions table ready");
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }
}

// Implement PermissionService trait for SqliteDb
#[async_trait::async_trait]
impl PermissionService for SqliteDb {
    /// Check if a user is authenticated
    fn check_authentication(&self, context: &PermissionContext) -> Result<bool, AppError> {
        // A user is considered authenticated if they have valid JWT claims
        Ok(context.is_authenticated())
    }

    /// Check if a user can perform an operation on a collection
    fn check_permission(&self, permissions: &CollectionPermissions, context: &PermissionContext) -> Result<bool, AppError> {
        // Get the operation rule for the requested operation
        let rule = match permissions.get_operation_rule(&context.operation) {
            Some(rule) => rule,
            None => {
                // If no rule is defined, default to superuser only
                debug!("No rule found for operation {:?} on collection {}, defaulting to superuser only", 
                       context.operation, context.collection);
                return Ok(context.is_superuser());
            }
        };

        // Check permission level
        match &rule.permission {
            PermissionLevel::None => {
                debug!("Permission denied: operation {:?} not allowed on collection {}", 
                       context.operation, context.collection);
                Ok(false)
            }
            PermissionLevel::Public => {
                debug!("Permission granted: public access for operation {:?} on collection {}", 
                       context.operation, context.collection);
                Ok(true)
            }
            PermissionLevel::AuthenticatedOnly => {
                let allowed = context.is_authenticated();
                debug!("Permission {}: authenticated access for operation {:?} on collection {}", 
                       if allowed { "granted" } else { "denied" },
                       context.operation, context.collection);
                Ok(allowed)
            }
            PermissionLevel::SuperuserOnly => {
                let allowed = context.is_superuser();
                debug!("Permission {}: superuser access for operation {:?} on collection {}", 
                       if allowed { "granted" } else { "denied" },
                       context.operation, context.collection);
                Ok(allowed)
            }
            PermissionLevel::Rule(rule_expr) => {
                // For now, implement basic rule evaluation
                // In the future, this could be extended with a proper rule engine
                debug!("Evaluating rule '{}' for operation {:?} on collection {}", 
                       rule_expr, context.operation, context.collection);
                
                // Basic rule evaluation (this is a placeholder for more sophisticated logic)
                self.evaluate_permission_rule(rule_expr, context)
            }
        }
    }

    /// Store permissions for a collection
    async fn store_permissions(&self, permissions: &CollectionPermissions) -> Result<(), AppError> {
        debug!("Storing permissions for collection: {}", permissions.collection);

        // Ensure permissions table exists
        self.create_permissions_table().await?;

        let permissions_json = serde_json::to_string(permissions)
            .map_err(|e| AppError::internal(format!("Failed to serialize permissions: {}", e)))?;
        
        let collection_name = permissions.collection.clone();
        let connection = self.connection.clone();
        let updated_at = permissions.updated_at;

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            // Use INSERT OR REPLACE to handle both new and existing permissions
            conn.execute(
                r#"
                INSERT OR REPLACE INTO collection_permissions 
                (collection, permissions_json, created_at, updated_at) 
                VALUES (?1, ?2, COALESCE((SELECT created_at FROM collection_permissions WHERE collection = ?1), ?3), ?3)
                "#,
                [&collection_name, &permissions_json, &updated_at.to_string()],
            )
            .map_err(|e| AppError::database(format!("Failed to store permissions: {}", e)))?;

            info!("✅ Stored permissions for collection: {}", collection_name);
            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// Get permissions for a collection
    async fn get_permissions(&self, collection: &str) -> Result<Option<CollectionPermissions>, AppError> {
        debug!("Getting permissions for collection: {}", collection);

        // Ensure permissions table exists
        self.create_permissions_table().await?;

        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        let permissions_json = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT permissions_json FROM collection_permissions WHERE collection = ?1")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let result = stmt.query_row([&collection_name], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            });

            match result {
                Ok(json) => Ok(Some(json)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(AppError::database(format!("Failed to get permissions: {}", e))),
            }
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        match permissions_json {
            Some(json) => {
                let permissions: CollectionPermissions = serde_json::from_str(&json)
                    .map_err(|e| AppError::internal(format!("Failed to deserialize permissions: {}", e)))?;
                
                debug!("✅ Found permissions for collection: {}", collection);
                Ok(Some(permissions))
            }
            None => {
                debug!("No custom permissions found for collection: {}", collection);
                Ok(None)
            }
        }
    }

    /// Delete permissions for a collection (revert to defaults)
    async fn delete_permissions(&self, collection: &str) -> Result<(), AppError> {
        debug!("Deleting permissions for collection: {}", collection);

        // Ensure permissions table exists
        self.create_permissions_table().await?;

        let collection_name = collection.to_string();
        let connection = self.connection.clone();

        spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let rows_affected = conn.execute(
                "DELETE FROM collection_permissions WHERE collection = ?1",
                [&collection_name],
            )
            .map_err(|e| AppError::database(format!("Failed to delete permissions: {}", e)))?;

            if rows_affected > 0 {
                info!("✅ Deleted permissions for collection: {}", collection_name);
            } else {
                debug!("No permissions to delete for collection: {}", collection_name);
            }

            Ok::<(), AppError>(())
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        Ok(())
    }

    /// List all collections that have custom permissions
    async fn list_collections_with_permissions(&self) -> Result<Vec<String>, AppError> {
        debug!("Listing collections with custom permissions");

        // Ensure permissions table exists
        self.create_permissions_table().await?;

        let connection = self.connection.clone();

        let collections = spawn_blocking(move || {
            let conn = connection
                .lock()
                .map_err(|_| AppError::database("Failed to acquire database lock"))?;

            let mut stmt = conn
                .prepare("SELECT collection FROM collection_permissions ORDER BY collection")
                .map_err(|e| AppError::database(format!("Failed to prepare statement: {}", e)))?;

            let collections = stmt
                .query_map([], |row| {
                    let collection: String = row.get(0)?;
                    Ok(collection)
                })
                .map_err(|e| AppError::database(format!("Failed to query collections: {}", e)))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| AppError::database(format!("Failed to collect collections: {}", e)))?;

            Ok::<Vec<String>, AppError>(collections)
        })
        .await
        .map_err(|e| AppError::internal(format!("Task join error: {}", e)))??;

        debug!("✅ Found {} collections with custom permissions", collections.len());
        Ok(collections)
    }
}

impl SqliteDb {
    /// Basic rule evaluation for permission rules
    /// This is a simplified implementation - a full rule engine would be more robust
    fn evaluate_permission_rule(&self, rule_expr: &str, context: &PermissionContext) -> Result<bool, AppError> {
        // For now, support some basic rule patterns
        // In a full implementation, this would parse and evaluate complex rule expressions
        
        // Example rules:
        // "@request.auth.id = @record.owner_id" - only record owner can access
        // "@request.auth.role = 'admin'" - only admin role
        // "@request.auth != null" - any authenticated user
        
        match rule_expr {
            "@request.auth != null" => Ok(context.is_authenticated()),
            rule if rule.contains("@request.auth.role = 'admin'") => Ok(context.is_superuser()),
            rule if rule.contains("@request.auth.role = 'superuser'") => Ok(context.is_superuser()),
            rule if rule.contains("@request.auth.id = @record.owner_id") => {
                // This would require record data to evaluate properly
                // For now, just check if user is authenticated
                Ok(context.is_authenticated())
            }
            _ => {
                debug!("Unknown rule expression: {}, defaulting to deny", rule_expr);
                Ok(false)
            }
        }
    }
}