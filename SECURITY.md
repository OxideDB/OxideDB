# OxideDB Plugin Security System

This document describes the capability-based security system implemented for OxideDB plugins to ensure safe execution of untrusted code in production environments.

## Overview

The OxideDB plugin security system provides comprehensive protection against malicious or poorly written plugins through:

- **Capability-based access control**: Plugins must be explicitly granted permissions
- **Trust level management**: Different security policies based on plugin trustworthiness
- **Resource limits**: Memory, execution time, and API call restrictions
- **Runtime monitoring**: Real-time tracking of plugin behavior
- **Violation handling**: Automatic suspension of misbehaving plugins
- **Audit logging**: Complete security event tracking

## Security Architecture

### Trust Levels

Plugins are assigned one of four trust levels:

```rust
pub enum PluginTrustLevel {
    Untrusted,    // Minimal capabilities, strict limits
    Sandboxed,    // Limited capabilities, moderate limits
    Trusted,      // Broader capabilities, relaxed limits
    System,       // Full capabilities, minimal restrictions
}
```

### Capabilities

Plugins must be explicitly granted capabilities to access system resources:

```rust
pub enum PluginCapability {
    LogInfo,                                    // Write info logs
    LogError,                                   // Write error logs
    AccessCollection(String),                   // Access specific collection
    HttpRequest(String),                        // Make HTTP requests to URL
    FileRead(String),                          // Read files from path
    FileWrite(String),                         // Write files to path
    DatabaseQuery(String),                     // Execute database queries
    NetworkAccess(String),                     // Network access to domain
    SystemCommand(String),                     // Execute system commands
    EnvironmentVariable(String),               // Access environment variables
}
```

### Resource Limits

Each plugin operates within defined resource constraints:

```rust
pub struct ResourceLimits {
    pub max_memory_mb: u32,              // Maximum memory usage
    pub max_execution_time_ms: u64,      // Maximum execution time per call
    pub max_host_function_calls: u32,    // Maximum host function calls
}
```

## Usage Examples

### Basic Plugin Loading

```rust
use oxide_core::plugin_security::*;
use oxidedb::plugin_runtime::WasmtimePluginRuntime;

// Create runtime with default security
let mut runtime = WasmtimePluginRuntime::new()?;

// Load untrusted plugin (gets minimal capabilities)
runtime.load_plugin("my_plugin", &wasm_bytes)?;
```

### Secure Plugin Loading

```rust
// Load plugin with specific trust level and capabilities
runtime.load_plugin_with_trust(
    "trusted_plugin",
    &wasm_bytes,
    PluginTrustLevel::Trusted,
    vec![
        PluginCapability::LogInfo,
        PluginCapability::AccessCollection("users".to_string()),
        PluginCapability::HttpRequest("https://api.example.com".to_string()),
    ],
    ResourceLimits {
        max_memory_mb: 10,
        max_execution_time_ms: 1000,
        max_host_function_calls: 100,
    },
)?;
```

### Runtime Capability Management

```rust
// Grant additional capability
runtime.grant_plugin_capability(
    "my_plugin",
    PluginCapability::AccessCollection("posts".to_string()),
)?;

// Revoke capability
runtime.revoke_plugin_capability(
    "my_plugin",
    &PluginCapability::LogError,
)?;

// Check capability
let has_access = runtime.plugin_has_capability(
    "my_plugin",
    &PluginCapability::LogInfo,
);
```

### Security Monitoring

```rust
// Check if plugin is suspended
if runtime.is_plugin_suspended("my_plugin") {
    println!("Plugin is suspended due to security violations");
}

// Get execution statistics
if let Some(stats) = runtime.get_plugin_stats("my_plugin") {
    println!("Total calls: {}", stats.total_calls);
    println!("Failed calls: {}", stats.failed_calls);
    println!("Memory usage: {} bytes", stats.memory_usage_bytes);
}

// View audit log
let audit_log = runtime.get_plugin_audit_log("my_plugin");
for entry in audit_log {
    println!("{:?}: {:?}", entry.timestamp, entry.event_type);
}
```

## Security Policies

Configure system-wide security policies:

```rust
let security_policies = SecurityPolicies {
    default_trust_level: PluginTrustLevel::Untrusted,
    max_violations_before_suspension: 3,
    violation_window: Duration::from_secs(300),
    require_explicit_capabilities: true,
    audit_all_operations: true,
};

let runtime = WasmtimePluginRuntime::new_with_security_policies(security_policies)?;
```

## Security Violations

The system automatically detects and responds to various security violations:

- **Unauthorized function calls**: Plugin attempts to call functions without required capabilities
- **Resource limit exceeded**: Plugin exceeds memory, time, or API call limits
- **Invalid host function call**: Plugin calls non-existent or malformed host functions
- **Capability violation**: Plugin attempts to access resources beyond its granted capabilities

### Violation Handling

When violations occur:

1. **Logging**: All violations are logged with detailed context
2. **Counting**: Violations are counted within a time window
3. **Suspension**: Plugins are automatically suspended after exceeding violation thresholds
4. **Audit Trail**: Complete audit trail is maintained for forensic analysis

## Production Deployment

### Recommended Security Configuration

For production environments with untrusted plugins:

```rust
let production_policies = SecurityPolicies {
    default_trust_level: PluginTrustLevel::Untrusted,
    max_violations_before_suspension: 1,  // Zero tolerance
    violation_window: Duration::from_secs(60),
    require_explicit_capabilities: true,
    audit_all_operations: true,
};

// Strict resource limits for untrusted plugins
let strict_limits = ResourceLimits {
    max_memory_mb: 1,
    max_execution_time_ms: 100,
    max_host_function_calls: 5,
};
```

### Security Best Practices

1. **Principle of Least Privilege**: Grant only the minimum capabilities required
2. **Regular Auditing**: Monitor audit logs for suspicious activity
3. **Resource Monitoring**: Track resource usage patterns
4. **Capability Review**: Regularly review and update plugin capabilities
5. **Trust Level Management**: Carefully manage plugin trust levels
6. **Violation Response**: Have procedures for handling security violations

### Plugin Validation

Before loading plugins in production:

1. **Static Analysis**: Analyze WASM bytecode for suspicious patterns
2. **Capability Assessment**: Determine minimum required capabilities
3. **Resource Profiling**: Measure resource usage under normal conditions
4. **Security Testing**: Test plugin behavior under various security scenarios
5. **Trust Verification**: Verify plugin source and integrity

## Integration with Existing Systems

The security system integrates with OxideDB's existing permission system:

- Plugin capabilities can be mapped to collection permissions
- User authentication context is available to plugins
- Database access is controlled through existing permission rules
- API access follows the same authentication/authorization patterns

## Monitoring and Alerting

### Key Metrics to Monitor

- Plugin execution times
- Memory usage patterns
- Security violation rates
- Capability usage frequency
- Plugin suspension events

### Recommended Alerts

- High violation rate (> 1 per minute)
- Plugin suspension events
- Unusual resource usage patterns
- Failed capability grants/revocations
- Audit log anomalies

## Troubleshooting

### Common Issues

1. **Plugin Fails to Load**
   - Check WASM compilation errors
   - Verify security policy compatibility
   - Review capability requirements

2. **Security Violations**
   - Review audit logs for violation details
   - Check plugin capability requirements
   - Verify resource limit appropriateness

3. **Performance Issues**
   - Monitor resource usage statistics
   - Review execution time patterns
   - Check for resource limit constraints

### Debug Mode

Enable detailed security logging:

```rust
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
```

## API Reference

See the complete API documentation in the source code:

- `oxide_core::plugin_security` - Core security types and traits
- `oxidedb::plugin_runtime::WasmtimePluginRuntime` - Secure runtime implementation
- `examples/secure_plugin_example.rs` - Comprehensive usage examples

## Security Considerations

### Known Limitations

1. **WASM Sandbox**: Relies on WASM's built-in sandboxing
2. **Host Function Security**: Security depends on proper host function implementation
3. **Resource Measurement**: Resource limits are approximate
4. **Side Channel Attacks**: Limited protection against timing attacks

### Future Enhancements

- Network traffic analysis
- Advanced static analysis
- Machine learning-based anomaly detection
- Integration with external security tools
- Enhanced resource measurement

## Contributing

When contributing to the security system:

1. Follow secure coding practices
2. Add comprehensive tests for security features
3. Update documentation for new capabilities
4. Consider security implications of changes
5. Review existing security policies

For security-related issues, please follow responsible disclosure practices.