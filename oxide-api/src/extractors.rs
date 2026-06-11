//! Request extractors for the API
//!
//! This module provides custom extractors for common request data like
//! authenticated user information, request IDs, etc.

use axum::{async_trait, extract::FromRequestParts, http::request::Parts};
use oxide_core::Claims;

use crate::{errors::ApiError, middleware::ClaimsExtension};

/// Extractor for authenticated user information
///
/// This extractor retrieves user claims from the request extensions,
/// which are set by the authentication middleware.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub claims: Claims,
}

impl AuthenticatedUser {
    /// Get the user ID from the claims
    pub fn user_id(&self) -> &str {
        &self.claims.sub
    }

    /// Get the user email from the claims
    pub fn email(&self) -> &str {
        &self.claims.email
    }

    /// Get the user role from the claims
    pub fn role(&self) -> &str {
        &self.claims.role
    }

    /// Check if the user is a superuser
    pub fn is_superuser(&self) -> bool {
        self.claims.role == "superuser"
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Extract claims from request extensions
        let claims_ext = parts
            .extensions
            .get::<ClaimsExtension>()
            .ok_or_else(|| ApiError::auth("Authentication required"))?;

        let claims = claims_ext
            .0
            .clone()
            .ok_or_else(|| ApiError::auth("Invalid authentication token"))?;

        Ok(AuthenticatedUser { claims })
    }
}
