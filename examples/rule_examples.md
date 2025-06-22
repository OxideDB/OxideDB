# OxideDB Authorization Rules Examples

This document demonstrates the comprehensive authorization rule system in OxideDB, which supports custom rules with variable substitution and expression evaluation.

## Available Variables

### Request Variables
- `@req.headers.x-api-key` - API key from headers
- `@req.headers.authorization` - Authorization header
- `@req.user.id` - Authenticated user ID
- `@req.user.role` - User role
- `@req.user.email` - User email address
- `@req.user.{custom_field}` - Custom fields from JWT claims

### Record Variables
- `@record.{field_name}` - Any field in the record
- `@record.user_id` - Record owner ID (common pattern)

### System Variables
- `@now` - Current timestamp
- `@now.hour` - Current hour (0-23)
- `@now.minute` - Current minute (0-59)
- `@now.day` - Current day of month (1-31)
- `@now.month` - Current month (1-12)
- `@now.year` - Current year
- `@now.weekday` - Current weekday (0=Sunday, 1=Monday, etc.)

## Rule Examples

### Basic Access Control

```javascript
// Public access
"true"

// No access
"false"

// Authenticated users only
"@req.user.id != ''"

// Superuser only
"@req.user.role = 'superuser'"
```

### API Key Based Access

```javascript
// Require any API key
"@req.headers.x-api-key != ''"

// Require specific API key
"@req.headers.x-api-key = 'your-secret-key'"

// Multiple valid API keys
"@req.headers.x-api-key = 'key1' || @req.headers.x-api-key = 'key2'"
```

### Owner-Based Access

```javascript
// Users can only access their own records
"@req.user.id = @record.user_id"

// Users can access their own records or admins can access any
"@req.user.id = @record.user_id || @req.user.role = 'admin'"

// Only active records can be accessed by owners
"@req.user.id = @record.user_id && @record.status = 'active'"
```

### Time-Based Access

```javascript
// Business hours only (9 AM - 5 PM)
"@now.hour >= 9 && @now.hour <= 17"

// Weekend access only
"@now.weekday = 0 || @now.weekday = 6"

// No access during maintenance window (2-4 AM)
"@now.hour < 2 || @now.hour >= 4"

// Monthly access window (first week of month)
"@now.day <= 7"
```

### IP-Based Access

```javascript
// Internal network only
"@req.headers.x-forwarded-for ~ '192.168.*'"

// Specific IP whitelist
"@req.headers.x-forwarded-for ~ '192.168.1.100' || @req.headers.x-forwarded-for ~ '10.0.0.*'"

// Block specific IPs
"@req.headers.x-forwarded-for !~ '192.168.1.50'"
```

### Complex Rules

```javascript
// Multi-factor rule: authenticated user from internal network during business hours
"@req.user.id != '' && @req.headers.x-forwarded-for ~ '192.168.*' && @now.hour >= 9 && @now.hour <= 17"

// Hierarchical access: owners always, managers during business hours, admins anytime
"@req.user.id = @record.user_id || (@req.user.role = 'manager' && @now.hour >= 9 && @now.hour <= 17) || @req.user.role = 'admin'"

// Content-based access: users can edit their own draft posts
"@req.user.id = @record.author_id && @record.status = 'draft'"

// Geographic and time restrictions
"@req.headers.x-country = 'US' && @now.hour >= 6 && @now.hour <= 22"
```

### Status-Based Rules

```javascript
// Only published content is publicly readable
"@record.status = 'published'"

// Users can edit their own drafts and pending posts
"@req.user.id = @record.user_id && (@record.status = 'draft' || @record.status = 'pending')"

// Admins can access everything, users only active records
"@req.user.role = 'admin' || @record.status = 'active'"
```

## Operators Supported

### Comparison Operators
- `=` - Equality
- `!=` - Not equal
- `>`, `<`, `>=`, `<=` - Numeric comparisons
- `~` - Pattern matching (glob-style with * and ?)
- `!~` - Negative pattern matching

### Logical Operators
- `&&` - Logical AND
- `||` - Logical OR

### Pattern Matching Examples

```javascript
// Wildcard matching
"@req.headers.user-agent ~ '*Chrome*'"

// Multiple patterns
"@req.headers.x-api-key ~ 'dev-*' || @req.headers.x-api-key ~ 'test-*'"

// File extension matching
"@record.filename ~ '*.pdf' || @record.filename ~ '*.doc'"
```

## Best Practices

1. **Start Simple**: Begin with basic rules and add complexity as needed
2. **Test Thoroughly**: Use the built-in test framework to verify rule behavior
3. **Document Rules**: Complex rules should be documented for maintainability
4. **Performance**: Simple rules evaluate faster than complex ones
5. **Security**: Always default to restrictive rules and explicitly allow access
6. **Fallbacks**: Consider what happens when variables are missing or empty

## Common Patterns

### Multi-Tenant Applications
```javascript
// Users can only access data from their organization
"@req.user.org_id = @record.org_id"
```

### Hierarchical Permissions
```javascript
// Managers can access their team's data
"@req.user.team_id = @record.team_id && @req.user.role = 'manager'"
```

### Audit Trail Protection
```javascript
// Audit logs are read-only except for system
"@req.user.role = 'system' || @req.method = 'GET'"
```

### Feature Flags
```javascript
// Beta features only for beta users
"@req.user.beta_user = true || @req.user.role = 'admin'"
```

# OxideDB Plugin Examples

This document provides comprehensive examples of OxideDB plugin capabilities, including event handling, HTTP route exposure, CRUD operations, and custom business logic.

## Basic Plugin Structure

Every OxideDB plugin must implement the basic interface:

```rust
// Import host functions
extern "C" {
    fn log_info(ptr: *const u8, len: usize);
    fn get_event_payload() -> i32;
    // ... other host functions
}

// Export required functions
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn plugin_cleanup() -> i32 { 0 }
```

## HTTP Route Registration and Handling

Plugins can expose custom HTTP endpoints for business logic:

### 1. Registering Routes

```rust
// Register HTTP routes during plugin initialization
#[no_mangle]
pub extern "C" fn plugin_init() -> i32 {
    // Register GET route for listing items
    if host_register_http_route("GET", "/api/hello/items", "handle_get_items").is_err() {
        return 1;
    }
    
    // Register POST route for creating items
    if host_register_http_route("POST", "/api/hello/items", "handle_create_item").is_err() {
        return 1;
    }
    
    // Register route with path parameters
    if host_register_http_route("GET", "/api/hello/items/:id", "handle_get_item").is_err() {
        return 1;
    }
    
    0 // Success
}
```

### 2. HTTP Request Handlers

```rust
#[no_mangle]
pub extern "C" fn handle_get_items() -> i32 {
    match handle_get_items_impl() {
        Ok(_) => 0,
        Err(e) => {
            host_log_error(&format!("Error: {}", e));
            set_error_response(500, &e);
            1
        }
    }
}

fn handle_get_items_impl() -> Result<(), String> {
    // Read records from database
    host_read_records("items", None)?;
    
    // Prepare response data
    let items = serde_json::json!([
        {"id": "1", "name": "Item 1", "created_by": "plugin"},
        {"id": "2", "name": "Item 2", "created_by": "plugin"}
    ]);
    
    // Set HTTP response
    host_set_http_response(
        200,
        &serde_json::json!({"Content-Type": "application/json"}),
        &items.to_string()
    )?;
    
    Ok(())
}
```

### 3. Handling POST Requests with Body

```rust
#[no_mangle]
pub extern "C" fn handle_create_item() -> i32 {
    match handle_create_item_impl() {
        Ok(_) => 0,
        Err(e) => {
            set_error_response(400, &e);
            1
        }
    }
}

fn handle_create_item_impl() -> Result<(), String> {
    // Get HTTP request context
    let request = host_get_http_request()?;
    
    // Parse request body
    let body = request.body.ok_or("Request body required")?;
    let mut item_data: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| format!("Invalid JSON: {}", e))?;
    
    // Add plugin metadata
    if let Some(obj) = item_data.as_object_mut() {
        obj.insert("created_by".to_string(), serde_json::Value::String("plugin".to_string()));
        obj.insert("created_at".to_string(), serde_json::Value::String("2024-01-01T00:00:00Z".to_string()));
    }
    
    // Create record in database
    host_create_record("items", &item_data.to_string())?;
    
    // Return success response
    let response = serde_json::json!({
        "success": true,
        "message": "Item created successfully",
        "data": item_data
    });
    
    host_set_http_response(
        201,
        &serde_json::json!({"Content-Type": "application/json"}),
        &response.to_string()
    )?;
    
    Ok(())
}
```

### 4. Path Parameters and Query Parameters

```rust
fn handle_get_item_impl() -> Result<(), String> {
    let request = host_get_http_request()?;
    
    // Get path parameter
    let item_id = request.path_params.get("id")
        .ok_or("Item ID not found in path")?;
    
    // Get query parameters
    let include_metadata = request.query_params
        .get("metadata")
        .map(|v| v == "true")
        .unwrap_or(false);
    
    // Create filter for specific item
    let filter = serde_json::json!({"id": item_id});
    host_read_records("items", Some(&filter.to_string()))?;
    
    // Build response based on query parameters
    let mut item = serde_json::json!({
        "id": item_id,
        "name": format!("Item {}", item_id),
        "description": "Retrieved by plugin"
    });
    
    if include_metadata {
        item["metadata"] = serde_json::json!({
            "retrieved_by": "plugin",
            "retrieved_at": "2024-01-01T00:00:00Z"
        });
    }
    
    host_set_http_response(
        200,
        &serde_json::json!({"Content-Type": "application/json"}),
        &item.to_string()
    )?;
    
    Ok(())
}
```

## CRUD Operations

Plugins can perform database operations with proper security controls:

### 1. Creating Records

```rust
fn create_user_record(user_data: &serde_json::Value) -> Result<String, String> {
    // Add validation
    if !user_data.get("email").and_then(|e| e.as_str()).map(|e| e.contains('@')).unwrap_or(false) {
        return Err("Invalid email address".to_string());
    }
    
    // Add plugin metadata
    let mut enhanced_data = user_data.clone();
    if let Some(obj) = enhanced_data.as_object_mut() {
        obj.insert("created_by_plugin".to_string(), serde_json::Value::String("user-manager".to_string()));
        obj.insert("validation_passed".to_string(), serde_json::Value::Bool(true));
    }
    
    // Create in database
    host_create_record("users", &enhanced_data.to_string())?;
    
    Ok("User created successfully".to_string())
}
```

### 2. Reading Records with Filters

```rust
fn get_users_by_role(role: &str) -> Result<serde_json::Value, String> {
    // Create filter
    let filter = serde_json::json!({
        "role": role,
        "active": true
    });
    
    // Read from database
    host_read_records("users", Some(&filter.to_string()))?;
    
    // For demonstration, return mock data
    // In real implementation, you'd get the actual results from the host
    Ok(serde_json::json!([
        {"id": "1", "name": "John", "role": role},
        {"id": "2", "name": "Jane", "role": role}
    ]))
}
```

### 3. Complex Filtering

```rust
fn search_items(search_query: &str, category: Option<&str>, limit: Option<i32>) -> Result<serde_json::Value, String> {
    let mut filter = serde_json::json!({
        "$or": [
            {"name": {"$regex": search_query, "$options": "i"}},
            {"description": {"$regex": search_query, "$options": "i"}}
        ]
    });
    
    if let Some(cat) = category {
        filter["category"] = serde_json::Value::String(cat.to_string());
    }
    
    if let Some(lim) = limit {
        filter["$limit"] = serde_json::Value::Number(serde_json::Number::from(lim));
    }
    
    host_read_records("items", Some(&filter.to_string()))?;
    
    Ok(serde_json::json!({
        "query": search_query,
        "category": category,
        "limit": limit,
        "results": []  // Would be populated by actual database results
    }))
}
```

## Custom Business Logic Examples

### 1. Data Processing Pipeline

```rust
#[no_mangle]
pub extern "C" fn handle_process_data() -> i32 {
    match process_data_pipeline() {
        Ok(_) => 0,
        Err(e) => {
            set_error_response(400, &e);
            1
        }
    }
}

fn process_data_pipeline() -> Result<(), String> {
    let request = host_get_http_request()?;
    let body = request.body.ok_or("Request body required")?;
    let input_data: serde_json::Value = serde_json::from_str(&body)?;
    
    // Step 1: Validate input
    validate_input_data(&input_data)?;
    
    // Step 2: Transform data
    let transformed_data = transform_data(input_data.clone())?;
    
    // Step 3: Enrich with external data
    let enriched_data = enrich_data(transformed_data)?;
    
    // Step 4: Store processed data (optional)
    if request.query_params.get("save").map(|s| s == "true").unwrap_or(false) {
        host_create_record("processed_data", &enriched_data.to_string())?;
    }
    
    // Step 5: Return results
    let response = serde_json::json!({
        "success": true,
        "input": input_data,
        "output": enriched_data,
        "processing_steps": ["validate", "transform", "enrich", "store"]
    });
    
    host_set_http_response(
        200,
        &serde_json::json!({"Content-Type": "application/json"}),
        &response.to_string()
    )?;
    
    Ok(())
}

fn validate_input_data(data: &serde_json::Value) -> Result<(), String> {
    // Custom validation logic
    if data.get("required_field").is_none() {
        return Err("Missing required field".to_string());
    }
    Ok(())
}

fn transform_data(data: serde_json::Value) -> Result<serde_json::Value, String> {
    match data {
        serde_json::Value::Object(mut obj) => {
            // Add processing metadata
            obj.insert("processed_at".to_string(), serde_json::Value::String("2024-01-01T00:00:00Z".to_string()));
            obj.insert("processor".to_string(), serde_json::Value::String("data-processor-plugin".to_string()));
            
            // Transform string values to uppercase
            for (_, value) in obj.iter_mut() {
                if let Some(string_val) = value.as_str() {
                    *value = serde_json::Value::String(string_val.to_uppercase());
                }
            }
            
            Ok(serde_json::Value::Object(obj))
        }
        other => Ok(other)
    }
}

fn enrich_data(data: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut enriched = data;
    
    // Add computed fields
    if let Some(obj) = enriched.as_object_mut() {
        obj.insert("computed_score".to_string(), serde_json::Value::Number(serde_json::Number::from(95)));
        obj.insert("enrichment_source".to_string(), serde_json::Value::String("plugin-computed".to_string()));
    }
    
    Ok(enriched)
}
```

### 2. Authentication and Authorization

```rust
fn handle_protected_endpoint() -> Result<(), String> {
    let request = host_get_http_request()?;
    
    // Check authentication
    let user = request.user.ok_or("Authentication required")?;
    let user_id = user.get("id").and_then(|id| id.as_str())
        .ok_or("Invalid user context")?;
    
    // Check authorization
    let user_role = user.get("role").and_then(|role| role.as_str())
        .unwrap_or("user");
    
    if !["admin", "moderator"].contains(&user_role) {
        return Err("Insufficient permissions".to_string());
    }
    
    // Log access
    host_log_info(&format!("Protected endpoint accessed by user: {}", user_id));
    
    // Proceed with business logic
    let response = serde_json::json!({
        "message": "Access granted",
        "user_id": user_id,
        "role": user_role,
        "timestamp": "2024-01-01T00:00:00Z"
    });
    
    host_set_http_response(
        200,
        &serde_json::json!({"Content-Type": "application/json"}),
        &response.to_string()
    )?;
    
    Ok(())
}
```

## Security Configuration

Configure plugin capabilities in `plugin.toml`:

```toml
[plugin]
name = "my-business-plugin"
version = "1.0.0"
description = "Custom business logic plugin"

[security]
capabilities = [
    "LogInfo",
    "LogError",
    "RegisterHttpRoutes",
    "HandleHttpRequests",
    "CreateRecords",
    "ReadRecords",
    "UpdateRecords",
]

[security.collections]
items = ["create", "read", "update"]
users = ["read"]
processed_data = ["create", "read"]

[security.http_routes]
allowed_paths = ["/api/business/*"]
allowed_methods = ["GET", "POST", "PUT"]

[hooks]
exports = [
    "plugin_init",
    "plugin_cleanup",
    "handle_get_items",
    "handle_create_item",
    "handle_process_data",
]
```

## Best Practices

1. **Error Handling**: Always handle errors gracefully and provide meaningful error messages
2. **Input Validation**: Validate all input data before processing
3. **Security**: Request only the minimum required capabilities
4. **Logging**: Use appropriate log levels and provide context
5. **Performance**: Minimize database operations and processing time
6. **Testing**: Test all endpoints and error conditions thoroughly

## Complete Example Plugin

See `hello-plugin/src/lib.rs` for a complete example that demonstrates:
- Event processing (on_before_create)
- HTTP route registration and handling
- CRUD operations with proper error handling
- Custom business logic implementation
- Security validation and capability usage
- Comprehensive logging and monitoring 