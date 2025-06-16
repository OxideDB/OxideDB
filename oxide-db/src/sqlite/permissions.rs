//! Permission storage implementation for SQLite
//!
//! This module implements permission storage operations for the SQLite database.
//! Permissions are stored in a dedicated table and can be retrieved for authorization.

use super::SqliteDb;
use oxide_core::{AppError, CollectionPermissions};
use oxide_core::auth::PermissionService;
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