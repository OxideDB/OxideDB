//! Plugin HTTP Route Handlers
//!
//! This module provides HTTP handlers for plugin-registered routes,
//! including authorization and security validation.

pub mod admin_pages;
pub mod analysis;
pub mod audit;
pub mod capabilities;
pub mod installation;
pub mod management;
pub mod permissions;
pub mod routes;
pub mod types;

use crate::{errors::ApiError, extractors::AuthenticatedUser};

// Re-export commonly used types and functions
pub use admin_pages::*;
pub use analysis::*;
pub use audit::*;
pub use capabilities::*;
pub use installation::*;
pub use management::*;
pub use permissions::*;
pub use routes::*;
pub use types::*;

fn ensure_plugin_superuser(
    authenticated_user: &AuthenticatedUser,
    action: &str,
) -> Result<(), ApiError> {
    if authenticated_user.is_superuser() {
        Ok(())
    } else {
        Err(ApiError::forbidden(format!(
            "Superuser privileges are required to {}",
            action
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxide_core::Claims;

    fn authenticated_user(role: &str) -> AuthenticatedUser {
        AuthenticatedUser {
            claims: Claims::new(
                "user-1".to_string(),
                "user@example.com".to_string(),
                role.to_string(),
                "_users".to_string(),
                1,
            ),
        }
    }

    #[test]
    fn plugin_control_plane_requires_superuser() {
        assert!(
            ensure_plugin_superuser(&authenticated_user("superuser"), "manage plugins").is_ok()
        );

        let result = ensure_plugin_superuser(&authenticated_user("user"), "manage plugins");
        assert!(matches!(result, Err(ApiError::Forbidden { .. })));
    }
}
