# OxideDB Plugin ZIP+TOML Architecture

## Overview

OxideDB now uses a secure, metadata-driven plugin architecture that separates plugin metadata from executable code. This approach significantly improves security by avoiding the need to preload WASM modules for metadata extraction and enables proper digital signature verification.

## Architecture Benefits

### 🔒 **Enhanced Security**
- **No WASM Preloading**: Metadata is read from TOML files without executing any WebAssembly code
- **Digital Signature Support**: ZIP packages can be digitally signed for tamper detection
- **Capability Validation**: Required capabilities are declared upfront and validated before installation
- **Trust Level Recommendations**: Plugins can declare their recommended trust level

### 📦 **Better Distribution**
- **Self-Contained Packages**: Everything needed for the plugin is in one ZIP file
- **Version Management**: Clear versioning and dependency information
- **Dependency Declaration**: Plugins can declare dependencies on other plugins or system components

### 🛠 **Developer Experience**
- **Readable Metadata**: TOML files are human-readable and easy to edit
- **Rich Metadata**: Support for homepage, license, changelog, keywords, and more
- **Configuration Schema**: JSON schema support for plugin configuration validation

## Plugin Package Structure

A plugin package is a ZIP file containing:

```
plugin-name-1.0.0.zip
├── plugin.toml          # Plugin manifest (required)
├── plugin-name.wasm     # WebAssembly binary (required)
├── admin/               # Admin UI pages and assets (optional)
│   ├── index.html
│   ├── app.js
│   └── styles.css
├── signature            # Digital signature (optional)
└── README.md           # Documentation (optional)
```

### Required Files

1. **`plugin.toml`** - Plugin manifest with metadata and security requirements
2. **`*.wasm`** - WebAssembly binary (filename must match `wasm_file` in manifest)

### Optional Files

1. **`signature`** or **`plugin.sig`** - Digital signature for verification
2. **`README.md`** - Plugin documentation
3. **`LICENSE`** - License file
4. **`admin/` assets** - HTML, CSS, and JavaScript for custom admin pages
5. **Additional assets** - Any other files the plugin needs

## Plugin Manifest Format (`plugin.toml`)

```toml
[plugin]
name = "my-awesome-plugin"
version = "1.0.0"
description = "A plugin that does amazing things"
author = "Your Name <your.email@example.com>"
homepage = "https://github.com/yourname/my-awesome-plugin"
license = "MIT"
min_oxide_version = "0.1.0"
keywords = ["database", "utility", "transformation"]
categories = ["data-processing", "utilities"]
changelog = "v1.0.0: Initial release"
build_timestamp = "2024-01-01T00:00:00Z"
wasm_file = "my-awesome-plugin.wasm"

[security]
required_capabilities = [
    "LogInfo",
    "ReadEventData",
    "CreateRecords",
    "HandleHttpRequests"
]
recommended_trust_level = "PartiallyTrusted"
security_contact = "security@example.com"
security_advisories = []

[security.audit_info]
audit_date = "2024-01-01"
auditor = "Security Audit Firm"
report_url = "https://example.com/audit-report"
status = "passed"

[[admin.pages]]
slug = "settings"
title = "My Plugin Settings"
description = "Configure My Plugin"
entry = "admin/index.html"
icon = "settings"
nav_group = "My Plugin"

[dependencies]
[dependencies.plugins]
# other-plugin = ">=1.0.0"

[dependencies.system]
min_memory_mb = 8
cpu_features = ["wasm-simd"]
system_libs = []

[config]
schema = {
    type = "object",
    properties = {
        api_key = { type = "string", description = "API key for external service" },
        batch_size = { type = "integer", default = 100, minimum = 1, maximum = 1000 }
    },
    required = ["api_key"]
}
defaults = { batch_size = 100 }
```

## Admin UI Pages

Plugins can contribute custom pages to the OxideDB admin console by declaring `[[admin.pages]]` entries in `plugin.toml` and packaging the compiled UI under the ZIP's `admin/` directory.

- `slug` is the stable admin route segment and must use lowercase letters, numbers, hyphens, or underscores.
- `entry` must point to an HTML file under `admin/`.
- Assets referenced by the entry HTML should also live under `admin/`.
- Packaged pages can use the shared stylesheet at `/admin/plugin-pages/style.css` for OxideDB-compatible controls and panels.
- Installed pages are exposed in the admin sidebar for enabled plugins and are served from protected `/admin/plugin-pages/assets/{plugin}/...` URLs.

Example HTML entry:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <link rel="stylesheet" href="/admin/plugin-pages/style.css" />
    <title>My Plugin Settings</title>
  </head>
  <body>
    <main class="oxide-page">
      <section class="oxide-panel">
        <div class="oxide-panel-header">
          <h1 class="oxide-panel-title">Settings</h1>
        </div>
        <div class="oxide-panel-body">
          <label>
            <span class="oxide-label">API key</span>
            <input class="oxide-input" name="api_key" />
          </label>
          <button class="oxide-button" type="button">Save</button>
        </div>
      </section>
    </main>
  </body>
</html>
```

## Plugin Installation Process

### 1. Package Analysis
```bash
# Analyze a plugin package before installation
curl -X POST http://localhost:8080/api/admin/plugins/analyze \
  -F "plugin_package=@my-plugin-1.0.0.zip"
```

### 2. Plugin Installation
```bash
# Install a plugin package
curl -X POST http://localhost:8080/api/admin/plugins \
  -F "plugin_package=@my-plugin-1.0.0.zip" \
  -F "trust_level=PartiallyTrusted" \
  -F "capabilities=[\"LogInfo\",\"ReadEventData\"]"
```

### 3. Security Validation
- Package integrity verification (ZIP structure)
- Digital signature verification (if present)
- Capability requirement validation
- Trust level compatibility check
- WASM binary validation

## Security Model

### Trust Levels
- **Untrusted**: Minimal capabilities, heavily sandboxed
- **PartiallyTrusted**: Limited capabilities, moderate sandbox
- **FullyTrusted**: Most capabilities, light sandbox
- **System**: All capabilities, minimal restrictions

### Capability System
Plugins must declare required capabilities in their manifest:

```toml
[security]
required_capabilities = [
    "LogInfo",           # Write to info logs
    "LogError",          # Write to error logs
    "ReadEventData",     # Read event payloads
    "ModifyEventData",   # Modify event payloads
    "CreateRecords",     # Create database records
    "ReadRecords",       # Read database records
    "UpdateRecords",     # Update database records
    "DeleteRecords",     # Delete database records
    "RegisterHttpRoutes", # Register HTTP routes
    "HandleHttpRequests", # Handle HTTP requests
    "BlockOperations"    # Block/deny operations
]
```

### Digital Signatures

Plugins can be digitally signed for verification:

1. **Create a signature** for your ZIP package
2. **Include the signature** in the ZIP as `signature` or `plugin.sig`
3. **Configure trusted keys** in OxideDB
4. **Verification happens** automatically during installation

```bash
# Example: Sign a plugin package (implementation depends on your key infrastructure)
gpg --detach-sign --armor my-plugin-1.0.0.zip
zip -u my-plugin-1.0.0.zip my-plugin-1.0.0.zip.asc
```

## API Endpoints

### Plugin Management
- `POST /api/admin/plugins/analyze` - Analyze plugin package
- `POST /api/admin/plugins` - Install plugin package
- `GET /api/admin/plugins` - List installed plugins
- `GET /api/admin/plugins/{name}` - Get plugin details
- `PUT /api/admin/plugins/{name}/enable` - Enable plugin
- `PUT /api/admin/plugins/{name}/disable` - Disable plugin
- `DELETE /api/admin/plugins/{name}` - Uninstall plugin

### Plugin Security
- `GET /api/admin/plugins/{name}/permissions` - Get plugin permissions
- `PUT /api/admin/plugins/{name}/permissions` - Update plugin permissions
- `POST /api/admin/plugins/{name}/capabilities/{capability}` - Grant capability
- `DELETE /api/admin/plugins/{name}/capabilities/{capability}` - Revoke capability

## Migration from Legacy Format

For plugins currently using embedded metadata, you can create a TOML manifest:

1. **Extract metadata** from your existing plugin
2. **Create a `plugin.toml`** file with the extracted information
3. **Package everything** in a ZIP file
4. **Test the package** using the analyze endpoint
5. **Install the new package** using the ZIP format

## Best Practices

### For Plugin Developers

1. **Minimal Capabilities**: Only request capabilities you actually need
2. **Clear Documentation**: Provide detailed descriptions and documentation
3. **Proper Versioning**: Use semantic versioning for your plugins
4. **Security Contact**: Provide a way for security researchers to contact you
5. **Regular Updates**: Keep your plugins updated and address security issues promptly

### For Plugin Users

1. **Verify Signatures**: Only install signed plugins from trusted sources
2. **Review Capabilities**: Understand what capabilities you're granting
3. **Monitor Behavior**: Keep an eye on plugin behavior and logs
4. **Regular Updates**: Update plugins regularly for security fixes
5. **Backup First**: Always backup your database before installing new plugins

### For System Administrators

1. **Signature Verification**: Configure and enforce signature verification
2. **Capability Policies**: Establish policies for capability grants
3. **Trust Level Policies**: Define when different trust levels are appropriate
4. **Monitoring**: Set up monitoring for plugin behavior and security events
5. **Regular Audits**: Perform regular security audits of installed plugins

## Example: Creating a Plugin Package

```bash
# 1. Create plugin directory
mkdir my-plugin-1.0.0
cd my-plugin-1.0.0

# 2. Create plugin.toml
cat > plugin.toml << 'EOF'
[plugin]
name = "my-plugin"
version = "1.0.0"
description = "My awesome plugin"
author = "Me <me@example.com>"
wasm_file = "my-plugin.wasm"

[security]
required_capabilities = ["LogInfo", "ReadEventData"]
recommended_trust_level = "PartiallyTrusted"
EOF

# 3. Copy WASM binary
cp ../target/wasm32-unknown-unknown/release/my_plugin.wasm my-plugin.wasm

# 4. Create ZIP package
zip -r ../my-plugin-1.0.0.zip .

# 5. Optionally sign the package
# (signing method depends on your infrastructure)

# 6. Test the package
curl -X POST http://localhost:8080/api/admin/plugins/analyze \
  -F "plugin_package=@../my-plugin-1.0.0.zip"
```

This new architecture provides a much more secure and maintainable approach to plugin distribution and management in OxideDB.
