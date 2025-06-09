//! Authentication-related operations for SQLite database

use super::connection::SqliteDb;
use crate::db::{Db, ListParams};
use oxide_core::{
    AppError, UserRole,
    BeforeEventContext, AfterEventContext, BeforeEventType, AfterEventType,
};
use tracing::{info, debug};

impl SqliteDb {
    /// Initialize authentication collections (users and superusers)
    pub(super) async fn initialize_auth_collections(&self) -> Result<(), AppError> {
        use oxide_core::auth::create_auth_collections;
        
        let (users_schema, superusers_schema) = create_auth_collections();
        
        // Create users collection if it doesn't exist
        if !self.collection_exists("users").await? {
            self.create_collection_with_schema(users_schema).await?;
            info!("✅ Created users auth collection");
        } else {
            debug!("Users auth collection already exists");
        }
        
        // Create superusers collection if it doesn't exist
        if !self.collection_exists("superusers").await? {
            self.create_collection_with_schema(superusers_schema).await?;
            info!("✅ Created superusers auth collection");
        } else {
            debug!("Superusers auth collection already exists");
        }
        
        info!("✅ Auth collections initialized");
        Ok(())
    }

    /// Authenticate a user with email and password
    pub async fn authenticate_user(&self, email: &str, password: &str) -> Result<(String, String), AppError> {
        info!("🔑 Authenticating user: {}", email);
        
        // Create context for BeforeUserAuth event
        let mut auth_context = BeforeEventContext {
            collection: "auth".to_string(),
            data: serde_json::json!({"email": email}),
            metadata: serde_json::json!({}),
            record_id: None,
            old_data: None,
        };
        
        // Dispatch BeforeUserAuth event
        self.event_bus
            .dispatch_before(BeforeEventType::UserAuth, &mut auth_context)
            .await?;

        // Try to find user in users collection first
        let user_record = match self.find_user_by_email(email, "users").await {
            Ok(record) => record,
            Err(_) => self.find_user_by_email(email, "superusers").await?,
        };

        // Verify password
        let password_hash = user_record.data.get("passwordHash")
            .and_then(|h| h.as_str())
            .ok_or_else(|| AppError::auth("Invalid user data: missing password hash"))?;

        if !self.auth_service.verify_password(password, password_hash)? {
            return Err(AppError::auth("Invalid email or password"));
        }

        // Determine user role based on collection
        let role = if user_record.collection == "superusers" {
            UserRole::Superuser
        } else {
            UserRole::User
        };

        // Generate JWT token
        let token = self.auth_service.generate_token(
            user_record.id.clone(),
            email.to_string(),
            role,
        )?;

        // Dispatch AfterUserAuth event
        self.event_bus
            .dispatch_after(AfterEventType::UserAuthenticated, &AfterEventContext::UserAuthenticated {
                user_id: user_record.id.clone(),
                email: email.to_string(),
            })
            .await?;

        info!("✅ User authenticated successfully: {}", email);
        Ok((user_record.id, token))
    }

    /// Find a user by email in the specified collection
    async fn find_user_by_email(&self, email: &str, collection: &str) -> Result<super::super::Record, AppError> {
        let records = <Self as Db>::list_records(self, collection, ListParams::default()).await?;
        
        for record in records {
            if let Some(user_email) = record.data.get("email").and_then(|e| e.as_str()) {
                if user_email == email {
                    return Ok(record);
                }
            }
        }
        
        Err(AppError::not_found("user", email))
    }

    /// Register a new user
    /// 
    /// This method creates a user record using the standard create_record flow,
    /// allowing all validation, sanitization, and business logic to be handled by hooks.
    pub async fn register_user(&self, email: &str, password: &str, is_superuser: bool) -> Result<String, AppError> {
        let collection = if is_superuser { "superusers" } else { "users" };
        
        info!("📝 Registering new user in {} collection: {}", collection, email);

        // Check if user already exists (this is domain logic that should remain in DB layer)
        if self.find_user_by_email(email, "users").await.is_ok() || 
           self.find_user_by_email(email, "superusers").await.is_ok() {
            return Err(AppError::conflict("User with this email already exists"));
        }

        // Create minimal user data - hooks will handle validation, hashing, and field additions
        let user_data = serde_json::json!({
            "email": email,
            "password": password,
            "verified": !is_superuser // superusers are verified by default
        });

        // Use the standard record creation flow - hooks will handle all transformations
        let record = <Self as Db>::create_record(self, collection, user_data).await?;

        // Dispatch user registration event (this may be redundant if hooks already handle it)
        self.event_bus
            .dispatch_after(AfterEventType::UserRegistered, &AfterEventContext::UserRegistered {
                user_id: record.id.clone(),
                email: email.to_string(),
                metadata: record.data.clone(),
            })
            .await?;

        info!("✅ User registered successfully: {}", email);
        Ok(record.id)
    }
}
