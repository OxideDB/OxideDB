//! Request extractors for the API
//!
//! This module provides custom extractors for common request data like
//! authenticated user information, request IDs, etc.

use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts},
    http::request::Parts,
};
use oxide_core::Claims;
use std::future::{ready, Future};

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

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // Extract claims from request extensions
        let result = parts
            .extensions
            .get::<ClaimsExtension>()
            .ok_or_else(|| ApiError::auth("Authentication required"))
            .and_then(|claims_ext| {
                claims_ext
                    .0
                    .clone()
                    .ok_or_else(|| ApiError::auth("Invalid authentication token"))
            })
            .map(|claims| AuthenticatedUser { claims });

        ready(result)
    }
}

impl<S> OptionalFromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Option<Self>, Self::Rejection>> + Send {
        let user = parts
            .extensions
            .get::<ClaimsExtension>()
            .and_then(|claims_ext| claims_ext.0.clone())
            .map(|claims| AuthenticatedUser { claims });

        ready(Ok(user))
    }
}
