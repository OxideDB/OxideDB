# Hello Plugin for OxideDB

A demonstration WebAssembly plugin for OxideDB that showcases the plugin API and security system integration.

## Overview

This plugin demonstrates:
- Integration with OxideDB's security-enhanced plugin system
- Event processing with proper capability-based security
- Data modification and validation
- Error handling and logging
- Plugin lifecycle management

## Security Capabilities

The plugin requires the following security capabilities to function:

- **ReadEventData**: Access to event payload data
- **LogInfo**: Ability to log informational messages
- **LogError**: Ability to log error messages
- **BlockOperations**: Ability to prevent operations from proceeding

## Features

### Data Processing
- Validates collection names for security (blocks access to "admin" and "system" collections)
- Adds plugin metadata to processed records
- Preserves original data structure while enhancing it

### Security Integration
- Works with OxideDB's capability-based security model
- Properly handles security violations
- Logs security-related events

### Error Handling
- Comprehensive error logging using both info and error log levels
- Graceful handling of malformed data
- Proper error propagation to the host system

## Building

```bash
# Build the plugin for WebAssembly
cargo build --target wasm32-unknown-unknown --release
```

The compiled plugin will be available at:
`target/wasm32-unknown-unknown/release/hello_plugin.wasm`

## Plugin Configuration

The plugin configuration is defined in `plugin.toml` and includes:
- Required security capabilities
- Plugin metadata
- Exported functions
- Behavior settings

## API Compatibility

This plugin is compatible with:
- OxideDB plugin API v2.0+
- Security-enhanced plugin runtime
- Capability-based security model

## Usage Example

When a create operation is performed on a collection, this plugin:

1. **Security Check**: Validates the collection name
2. **Data Enhancement**: Adds plugin metadata to the record
3. **Logging**: Records the processing activity
4. **Response**: Returns the modified data or blocks the operation

### Allowed Operation
```json
{
  "event_type": "before_create",
  "collection": "users",
  "data": "{\"name\": \"John\", \"email\": \"john@example.com\"}"
}
```

Result: Operation proceeds with enhanced data including `plugin_processed_at` and `plugin_name` fields.

### Blocked Operation
```json
{
  "event_type": "before_create",
  "collection": "admin_users",
  "data": "{\"name\": \"Admin\", \"role\": \"administrator\"}"
}
```

Result: Operation is blocked due to restricted collection name.

## Testing

The plugin includes basic tests that can be run with:

```bash
cargo test
```

## Integration

To use this plugin with OxideDB:

1. Build the plugin as shown above
2. Copy the `.wasm` file to your OxideDB plugins directory
3. Ensure the plugin configuration (`plugin.toml`) is properly set up
4. Grant the required security capabilities in your OxideDB configuration
5. Load the plugin through the OxideDB plugin management interface

## Security Considerations

- The plugin validates collection names to prevent access to sensitive collections
- All host function calls are protected by the capability system
- Error messages are logged appropriately without exposing sensitive information
- The plugin follows the principle of least privilege