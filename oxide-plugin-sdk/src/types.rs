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

/// Plugin metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    /// Plugin name (unique identifier)
    pub name: String,
    /// Plugin version (semantic versioning recommended)
    pub version: String,
    /// Plugin description
    pub description: String,
    /// Plugin author(s)
    pub author: String,
    /// Plugin homepage or repository URL
    pub homepage: Option<String>,
    /// Plugin license identifier (e.g., MIT, Apache-2.0)
    pub license: Option<String>,
    /// Minimum required OxideDB version
    pub min_oxide_version: Option<String>,
    /// Plugin keywords/tags for discovery
    pub keywords: Vec<String>,
    /// Plugin categories for organization
    pub categories: Vec<String>,
    /// Dependencies on other plugins
    pub dependencies: Vec<String>,
    /// Changelog or release notes
    pub changelog: Option<String>,
    /// Build timestamp (ISO 8601 format)
    pub build_timestamp: String,
    /// Supported features/capabilities
    pub features: Vec<String>,
    /// Required capabilities for plugin operation
    pub required_capabilities: Vec<String>,
    /// Recommended trust level for this plugin
    pub recommended_trust_level: Option<String>,
    /// Security-related metadata
    pub security: PluginSecurityMetadata,
    /// JSON schema for plugin configuration
    pub config_schema: Option<serde_json::Value>,
    /// Custom metadata fields
    pub custom: Option<serde_json::Value>,
}

/// Security-related metadata for plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginSecurityMetadata {
    /// Hash of the plugin binary for integrity verification
    pub binary_hash: Option<String>,
    /// Hash algorithm used (e.g., "SHA-256")
    pub hash_algorithm: Option<String>,
    /// Digital signature for code signing (if available)
    pub signature: Option<String>,
    /// Certificate chain for verification
    pub certificate_chain: Option<Vec<String>>,
    /// Security contact information
    pub security_contact: Option<String>,
    /// Known security vulnerabilities or advisories
    pub security_advisories: Vec<String>,
    /// Audit information
    pub audit_info: Option<PluginAuditInfo>,
}

/// Plugin audit information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginAuditInfo {
    /// Audit date (ISO 8601 format)
    pub audit_date: String,
    /// Auditor name or organization
    pub auditor: String,
    /// Audit report URL or identifier
    pub report_url: Option<String>,
    /// Audit status (e.g., "passed", "failed", "pending")
    pub status: String,
}

impl Default for PluginSecurityMetadata {
    fn default() -> Self {
        Self {
            binary_hash: None,
            hash_algorithm: None,
            signature: None,
            certificate_chain: None,
            security_contact: None,
            security_advisories: Vec::new(),
            audit_info: None,
        }
    }
}

impl PluginMetadata {
    /// Create a new plugin metadata with basic information
    pub fn new(name: String, version: String, description: String, author: String) -> Self {
        let build_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string();
        Self {
            name,
            version,
            description,
            author,
            homepage: None,
            license: None,
            min_oxide_version: None,
            keywords: Vec::new(),
            categories: Vec::new(),
            dependencies: Vec::new(),
            changelog: None,
            build_timestamp,
            features: Vec::new(),
            required_capabilities: Vec::new(),
            recommended_trust_level: None,
            security: PluginSecurityMetadata::default(),
            config_schema: None,
            custom: None,
        }
    }

    /// Set the plugin homepage URL
    pub fn with_homepage(mut self, homepage: String) -> Self {
        self.homepage = Some(homepage);
        self
    }

    /// Set the plugin license
    pub fn with_license(mut self, license: String) -> Self {
        self.license = Some(license);
        self
    }

    /// Set the minimum OxideDB version requirement
    pub fn with_min_oxide_version(mut self, version: String) -> Self {
        self.min_oxide_version = Some(version);
        self
    }

    /// Set plugin keywords/tags
    pub fn with_keywords(mut self, keywords: Vec<String>) -> Self {
        self.keywords = keywords;
        self
    }

    /// Set plugin categories
    pub fn with_categories(mut self, categories: Vec<String>) -> Self {
        self.categories = categories;
        self
    }

    /// Set plugin features
    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    /// Set required capabilities
    pub fn with_required_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.required_capabilities = capabilities;
        self
    }

    /// Set recommended trust level
    pub fn with_recommended_trust_level(mut self, trust_level: String) -> Self {
        self.recommended_trust_level = Some(trust_level);
        self
    }

    /// Set security metadata
    pub fn with_security(mut self, security: PluginSecurityMetadata) -> Self {
        self.security = security;
        self
    }

    /// Set plugin configuration schema
    pub fn with_config_schema(mut self, schema: serde_json::Value) -> Self {
        self.config_schema = Some(schema);
        self
    }

    /// Set build timestamp
    pub fn with_build_timestamp(mut self, timestamp: String) -> Self {
        self.build_timestamp = timestamp;
        self
    }

    /// Set custom metadata
    pub fn with_custom(mut self, custom: serde_json::Value) -> Self {
        self.custom = Some(custom);
        self
    }

    /// Validate the metadata for completeness and consistency
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("Plugin name cannot be empty".to_string());
        }

        if self.version.trim().is_empty() {
            return Err("Plugin version cannot be empty".to_string());
        }

        if self.description.trim().is_empty() {
            return Err("Plugin description cannot be empty".to_string());
        }

        if self.author.trim().is_empty() {
            return Err("Plugin author cannot be empty".to_string());
        }

        // Basic semantic version validation
        if !self.version.chars().any(|c| c.is_ascii_digit()) {
            return Err("Plugin version should contain at least one digit".to_string());
        }

        Ok(())
    }

    /// Check if the plugin is compatible with a given OxideDB version
    pub fn is_compatible_with(&self, oxide_version: &str) -> bool {
        if let Some(min_version) = &self.min_oxide_version {
            // Simple string comparison for now
            // In a real implementation, you'd use proper semver comparison
            oxide_version >= min_version
        } else {
            true // No minimum version requirement
        }
    }

    /// Convert to JSON for storage or transmission
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Create from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Default for PluginMetadata {
    fn default() -> Self {
        Self::new(
            "unknown-plugin".to_string(),
            "0.1.0".to_string(),
            "No description provided".to_string(),
            "Unknown Author".to_string(),
        )
    }
}

/// Plugin runtime information that can be queried from a loaded plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRuntimeInfo {
    /// Plugin metadata
    pub metadata: PluginMetadata,
    /// Runtime-specific information
    pub runtime: RuntimeDetails,
    /// Plugin status
    pub status: PluginStatus,
    /// Execution statistics
    pub stats: PluginStats,
}

/// Runtime details about the plugin environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDetails {
    /// Runtime type (e.g., "wasmtime")
    pub runtime_type: String,
    /// Runtime version
    pub runtime_version: String,
    /// SDK version used to build the plugin
    pub sdk_version: String,
    /// Plugin load timestamp
    pub loaded_at: String,
    /// Memory usage information
    pub memory_info: MemoryInfo,
}

/// Memory usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryInfo {
    /// Current memory usage in bytes
    pub current_bytes: u64,
    /// Peak memory usage in bytes
    pub peak_bytes: u64,
    /// Memory limit in bytes
    pub limit_bytes: u64,
}

/// Plugin execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginStatus {
    /// Plugin is loaded and running
    Active,
    /// Plugin is loaded but suspended
    Suspended,
    /// Plugin encountered an error
    Error(String),
    /// Plugin is being unloaded
    Unloading,
}

/// Plugin execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStats {
    /// Total number of function calls
    pub total_calls: u64,
    /// Total execution time in milliseconds
    pub total_execution_time_ms: u64,
    /// Number of successful operations
    pub successful_operations: u64,
    /// Number of failed operations
    pub failed_operations: u64,
    /// Last execution timestamp
    pub last_execution: Option<String>,
}

impl Default for PluginStats {
    fn default() -> Self {
        Self {
            total_calls: 0,
            total_execution_time_ms: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_execution: None,
        }
    }
} 