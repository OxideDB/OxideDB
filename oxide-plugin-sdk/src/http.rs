//! HTTP utilities for plugin HTTP request handling

use crate::{PluginResult, PluginError, HttpRequestContext, HttpResponse, Host};

/// HTTP utilities for plugins
pub struct Http;

impl Http {
    /// Get the current HTTP request context
    pub fn get_request() -> PluginResult<HttpRequestContext> {
        Host::get_http_request()
    }

    /// Send an HTTP response
    pub fn send_response(response: HttpResponse) -> PluginResult<()> {
        Host::set_http_response(&response)
    }

    /// Send a JSON response
    pub fn send_json<T: serde::Serialize>(data: &T) -> PluginResult<()> {
        let response = HttpResponse::json(data)?;
        Self::send_response(response)
    }

    /// Send an error response
    pub fn send_error(status_code: u16, message: &str) -> PluginResult<()> {
        let response = HttpResponse::error(status_code, message);
        Self::send_response(response)
    }

    /// Send a text response
    pub fn send_text<S: Into<String>>(text: S) -> PluginResult<()> {
        let response = HttpResponse::text(text);
        Self::send_response(response)
    }

    /// Parse JSON from request body
    pub fn parse_json<T: for<'de> serde::Deserialize<'de>>(request: &HttpRequestContext) -> PluginResult<T> {
        request.body_json().map_err(|e| PluginError::JsonError(e))
    }

    /// Check if request has JSON content type
    pub fn is_json_request(request: &HttpRequestContext) -> bool {
        request.is_json()
    }

    /// Register a route during plugin initialization
    pub fn register_route(method: &str, path: &str, handler: &str) -> PluginResult<()> {
        Host::register_http_route(method, path, handler)
    }
}

/// Trait for HTTP route handlers
/// 
/// Implement this trait for different types of HTTP handlers
pub trait HttpHandler {
    /// Handle an HTTP request
    fn handle(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse>;
}

/// A simple function-based HTTP handler
pub struct FunctionHandler<F> {
    handler: F,
}

impl<F> FunctionHandler<F>
where
    F: Fn(&HttpRequestContext) -> PluginResult<HttpResponse>,
{
    /// Create a new function handler
    pub fn new(handler: F) -> Self {
        Self { handler }
    }
}

impl<F> HttpHandler for FunctionHandler<F>
where
    F: Fn(&HttpRequestContext) -> PluginResult<HttpResponse>,
{
    fn handle(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        (self.handler)(request)
    }
}

/// JSON response builder
pub struct JsonResponseBuilder {
    status_code: u16,
    headers: std::collections::HashMap<String, String>,
}

impl JsonResponseBuilder {
    /// Create a new JSON response builder
    pub fn new() -> Self {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        
        Self {
            status_code: 200,
            headers,
        }
    }

    /// Set the status code
    pub fn status(mut self, status_code: u16) -> Self {
        self.status_code = status_code;
        self
    }

    /// Add a header
    pub fn header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Build the response with JSON data
    pub fn json<T: serde::Serialize>(self, data: &T) -> PluginResult<HttpResponse> {
        let body = serde_json::to_string(data)?;
        Ok(HttpResponse {
            status_code: self.status_code,
            headers: self.headers,
            body,
        })
    }
}

impl Default for JsonResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
} 