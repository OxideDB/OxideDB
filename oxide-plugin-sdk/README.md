# OxideDB Plugin SDK

A high-level SDK for developing OxideDB plugins with WebAssembly (WASM). This SDK abstracts away the low-level FFI boilerplate and provides a clean, Rust-idiomatic interface for plugin development.

## Features

- **Event Handling**: Simple trait-based event handling for database operations
- **HTTP Routes**: Easy HTTP route registration and handling
- **Database Operations**: High-level database CRUD operations
- **VFS Operations**: Safe namespace-scoped file access for plugin data workflows
- **Logging**: Structured logging support with multiple levels
- **Memory Management**: Automatic memory management for WASM
- **Type Safety**: Strongly-typed interfaces with comprehensive error handling

## Quick Start

### Basic Plugin

Create a simple plugin that validates data before creation:

```rust
use oxide_plugin_sdk::prelude::*;

#[derive(Default)]
struct MyPlugin;

impl PluginEventHandler for MyPlugin {
    fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        log_info!("Creating record in collection: {}", event.collection);
        
        // Parse the incoming data
        let data: JsonValue = serde_json::from_str(&event.data)?;
        
        // Validate required fields
        if !data.get("name").and_then(|v| v.as_str()).map_or(false, |s| !s.is_empty()) {
            return Ok(PluginResponse::deny("Name field is required"));
        }
        
        // Add plugin metadata
        let mut enhanced_data = data;
        enhanced_data["processed_by"] = json!("my-plugin");
        enhanced_data["processed_at"] = json!("2024-01-01T00:00:00Z");
        
        PluginResponse::allow_with_data(&enhanced_data)
    }
}

// Export the plugin
export_plugin!(MyPlugin);
```

### HTTP Plugin

Create a plugin that handles HTTP requests:

```rust
use oxide_plugin_sdk::prelude::*;

#[derive(Default)]
struct HttpPlugin;

impl PluginEventHandler for HttpPlugin {
    fn on_init(&mut self) -> PluginResult<()> {
        // Register HTTP routes
        Http::register_route("GET", "/api/hello", "handle_hello")?;
        Http::register_route("POST", "/api/data", "handle_post_data")?;
        Ok(())
    }
}

impl PluginHttpHandler for HttpPlugin {
    fn handle_request(&mut self, request: &HttpRequestContext) -> PluginResult<HttpResponse> {
        match (request.method.as_str(), request.path.as_str()) {
            ("GET", "/api/hello") => {
                json_response!({"message": "Hello from plugin!"})
            }
            ("POST", "/api/data") => {
                let data: JsonValue = Http::parse_json(request)?;
                
                // Process the data
                let result = Database::create("processed_data", &data)?;
                
                if result.is_success() {
                    json_response!({"success": true, "id": result.data()})
                } else {
                    error_response!(400, "Failed to save data")
                }
            }
            _ => error_response!(404, "Route not found"),
        }
    }
}

// Export the HTTP plugin
export_http_plugin!(HttpPlugin);
```

### Database Operations

Perform database operations within your plugin:

```rust
use oxide_plugin_sdk::prelude::*;

#[derive(Default)]
struct DatabasePlugin;

impl PluginEventHandler for DatabasePlugin {
    fn on_before_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        // Read existing records to check for duplicates
        let existing = Database::read_typed("users")?;
        
        let new_data: JsonValue = serde_json::from_str(&event.data)?;
        let new_email = new_data.get("email").and_then(|v| v.as_str());
        
        if let Some(email) = new_email {
            for record in existing {
                if let Some(existing_email) = record.data.get("email").and_then(|v| v.as_str()) {
                    if existing_email == email {
                        return Ok(PluginResponse::deny("Email already exists"));
                    }
                }
            }
        }
        
        Ok(PluginResponse::allow())
    }
    
    fn on_after_create(&mut self, event: &EventPayload) -> PluginResult<PluginResponse> {
        // Log user creation for audit
        let audit_data = json!({
            "action": "user_created",
            "collection": event.collection,
            "timestamp": "2024-01-01T00:00:00Z",
            "data": event.data
        });
        
        Database::create("audit_log", &audit_data)?;
        
        Ok(PluginResponse::allow())
    }
}

export_plugin!(DatabasePlugin);
```

### VFS Operations

Read, write, move, list, and delete files in namespaces granted to your plugin:

```rust
use oxide_plugin_sdk::prelude::*;

fn archive_report() -> PluginResult<()> {
    let metadata = Vfs::write(
        "reports",
        "incoming/q2.txt",
        b"quarterly results".to_vec(),
        Some("text/plain"),
    )?;

    Vfs::move_file(
        "reports",
        FileIdentifier::id(metadata.id),
        "archive/2026/q2.txt",
        false,
    )?;

    let files = Vfs::list("reports", "archive", true)?;
    log_info!("Archived report count: {}", files.total_count);

    Ok(())
}
```

Grant VFS access with an `AccessVfs` capability, for example:
`AccessVfs(namespaces=["reports"], operations=["read", "write", "move", "list"])`.

## Configuration

Add the SDK to your plugin's `Cargo.toml`:

```toml
[package]
name = "my-oxide-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
oxide-plugin-sdk = { path = "../oxide-plugin-sdk", features = ["http", "database"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[features]
default = []
```

## Features

The SDK supports the following optional features:

- `http` - Enable HTTP request handling capabilities
- `database` - Enable database operation utilities
- `debug` - Enable additional debugging features

## API Reference

### Core Traits

- `PluginEventHandler` - Handle database events (before/after create, update, delete)
- `PluginHttpHandler` - Handle HTTP requests (when `http` feature is enabled)

### Utilities

- `Host` - Low-level host function interface
- `Database` - High-level database operations
- `Http` - HTTP request/response utilities
- `Logger` - Structured logging

### Response Types

- `PluginResponse` - Response from event handlers
- `HttpResponse` - HTTP response with status, headers, and body
- `DatabaseResult` - Database operation results

### Macros

- `export_plugin!` - Export a basic plugin
- `export_http_plugin!` - Export a plugin with HTTP capabilities
- `log_info!`, `log_error!`, etc. - Logging macros
- `json_response!`, `error_response!` - HTTP response macros

## Memory Management

The SDK automatically handles WASM memory management, including:

- Memory allocation and deallocation
- Data passing between host and plugin
- Response buffer management

You don't need to worry about the low-level memory operations.

## Error Handling

The SDK uses Rust's `Result` type for comprehensive error handling:

```rust
fn handle_operation() -> PluginResult<()> {
    let data = Database::read("collection")?;
    // Handle success case
    Ok(())
}
```

## Logging

Use the built-in logging macros for consistent logging:

```rust
log_info!("Processing record: {}", record_id);
log_error!("Failed to validate data: {}", error);
log_warn!("Deprecated field used: {}", field_name);
log_debug!("Debug info: {:?}", debug_data);
```

Or use the structured logging builder:

```rust
LogBuilder::info()
    .message("User action performed")
    .field("user_id", user_id)
    .field("action", "login")
    .emit();
```

## Testing

Create unit tests for your plugin logic:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validation_logic() {
        let mut plugin = MyPlugin::default();
        let event = EventPayload {
            event_type: "BeforeRecordCreate".to_string(),
            collection: "users".to_string(),
            data: r#"{"name": "John Doe"}"#.to_string(),
            metadata: json!({}),
        };
        
        let response = plugin.on_before_create(&event).unwrap();
        assert!(response.allow);
    }
}
```

## Best Practices

1. **Error Handling**: Always use `?` operator for error propagation
2. **Logging**: Use appropriate log levels and include context
3. **Validation**: Validate all input data before processing
4. **Resources**: Clean up resources in the `on_cleanup` method
5. **Testing**: Write comprehensive unit tests for your plugin logic

## Examples

See the `examples/` directory for more comprehensive plugin examples.

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your option. 
