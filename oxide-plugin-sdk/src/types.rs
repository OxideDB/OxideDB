//! Core types for the OxideDB Plugin SDK

use serde::{de::Error, Deserialize, Serialize};
use std::collections::HashMap;

/// Event payload structure for plugin events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    /// The event type (e.g., "BeforeRecordCreate", "AfterRecordUpdate")
    pub event_type: String,
    /// The collection/table name being operated on
    pub collection: String,
    /// The data being operated on (JSON-serialized)
    pub data: String,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

/// Response from a plugin after processing an event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    /// Whether the plugin wants to allow the operation to continue
    pub allow: bool,
    /// Optional modified data (if the plugin wants to transform the data)
    pub modified_data: Option<String>,
    /// Optional error message (if allow is false)
    pub error_message: Option<String>,
    /// Additional metadata to pass back to the host
    pub metadata: serde_json::Value,
}

impl PluginResponse {
    /// Create a response that allows the operation to continue
    pub fn allow() -> Self {
        Self {
            allow: true,
            modified_data: None,
            error_message: None,
            metadata: serde_json::Value::Null,
        }
    }
    
    /// Create a response that allows the operation with modified data
    pub fn allow_with_data<T: Serialize>(data: &T) -> Result<Self, serde_json::Error> {
        Ok(Self {
            allow: true,
            modified_data: Some(serde_json::to_string(data)?),
            error_message: None,
            metadata: serde_json::Value::Null,
        })
    }
    
    /// Create a response that blocks the operation with an error message
    pub fn deny<S: Into<String>>(error_message: S) -> Self {
        Self {
            allow: false,
            modified_data: None,
            error_message: Some(error_message.into()),
            metadata: serde_json::Value::Null,
        }
    }
    
    /// Add metadata to the response
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}

impl Default for PluginResponse {
    fn default() -> Self {
        Self::allow()
    }
}

/// HTTP request context passed to plugin HTTP handlers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequestContext {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: String,
    /// Request path
    pub path: String,
    /// Query parameters
    pub query_params: HashMap<String, String>,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body
    pub body: Option<String>,
    /// Path parameters from route matching
    pub path_params: HashMap<String, String>,
    /// User context (if authenticated)
    pub user: Option<serde_json::Value>,
}

impl HttpRequestContext {
    /// Get a query parameter by name
    pub fn get_query(&self, name: &str) -> Option<&String> {
        self.query_params.get(name)
    }
    
    /// Get a header by name
    pub fn get_header(&self, name: &str) -> Option<&String> {
        self.headers.get(name)
    }
    
    /// Get a path parameter by name
    pub fn get_path_param(&self, name: &str) -> Option<&String> {
        self.path_params.get(name)
    }
    
    /// Parse the request body as JSON
    pub fn body_json<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        match &self.body {
            Some(body) => serde_json::from_str(body),
            None => Err(serde_json::Error::custom("No request body")),
        }
    }
    
    /// Check if the request has a JSON content type
    pub fn is_json(&self) -> bool {
        self.get_header("content-type")
            .map(|ct| ct.contains("application/json"))
            .unwrap_or(false)
    }
}

/// HTTP response from plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponse {
    /// HTTP status code
    pub status_code: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: String,
}

impl HttpResponse {
    /// Create a new HTTP response
    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            headers: HashMap::new(),
            body: String::new(),
        }
    }
    
    /// Create a successful response with JSON body
    pub fn json<T: Serialize>(data: &T) -> Result<Self, serde_json::Error> {
        let mut response = Self::new(200);
        response.headers.insert("Content-Type".to_string(), "application/json".to_string());
        response.body = serde_json::to_string(data)?;
        Ok(response)
    }
    
    /// Create an error response
    pub fn error(status_code: u16, message: &str) -> Self {
        let mut response = Self::new(status_code);
        response.headers.insert("Content-Type".to_string(), "application/json".to_string());
        response.body = format!(r#"{{"error": "{}"}}"#, message);
        response
    }
    
    /// Create a text response
    pub fn text<S: Into<String>>(text: S) -> Self {
        let mut response = Self::new(200);
        response.headers.insert("Content-Type".to_string(), "text/plain".to_string());
        response.body = text.into();
        response
    }
    
    /// Add a header
    pub fn with_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }
    
    /// Set the response body
    pub fn with_body<S: Into<String>>(mut self, body: S) -> Self {
        self.body = body.into();
        self
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self::new(200)
    }
}

/// Database record structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Record ID
    pub id: String,
    /// Record data as JSON
    pub data: serde_json::Value,
    /// Creation timestamp
    pub created_at: Option<String>,
    /// Last update timestamp  
    pub updated_at: Option<String>,
}

/// Database operation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseResult {
    /// Whether the operation was successful
    pub success: bool,
    /// Result data (records for read, ID for create, count for update/delete)
    pub data: Option<serde_json::Value>,
    /// Error message if operation failed
    pub error: Option<String>,
}

impl DatabaseResult {
    /// Check if the operation was successful
    pub fn is_success(&self) -> bool {
        self.success
    }
    
    /// Get the error message if operation failed
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    
    /// Get the result data
    pub fn data(&self) -> Option<&serde_json::Value> {
        self.data.as_ref()
    }
    
    /// Parse the result data as a specific type
    pub fn parse_data<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        match &self.data {
            Some(data) => serde_json::from_value(data.clone()),
            None => Err(serde_json::Error::custom("No result data")),
        }
    }
    
    /// Parse the result data as a list of records
    pub fn parse_records(&self) -> Result<Vec<Record>, serde_json::Error> {
        self.parse_data()
    }
    
    /// Parse the result data as a single record
    pub fn parse_record(&self) -> Result<Record, serde_json::Error> {
        self.parse_data()
    }
}

/// Log level for plugin logging
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Error,
    Warn,
    Debug,
}

impl LogLevel {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Error => "ERROR", 
            LogLevel::Warn => "WARN",
            LogLevel::Debug => "DEBUG",
        }
    }
} 