# TypeScript Demo Plugin for OxideDB

This advanced demo plugin showcases the comprehensive capabilities of the OxideDB plugin system, demonstrating how to build sophisticated, production-ready plugins using TypeScript and WebAssembly.

## 🚀 Features Demonstrated

### Core Plugin Capabilities
- **Data Validation**: Comprehensive input validation with custom rules
- **Data Transformation**: Automatic data normalization and enrichment
- **External Integration**: Simulated third-party API calls and validations
- **Security Processing**: PII detection and security-aware data handling
- **Fraud Detection**: Risk scoring and automated fraud prevention
- **Collection-Specific Logic**: Different processing rules per collection type
- **Error Handling**: Robust error management and logging
- **Metadata Processing**: Rich metadata generation and processing

### Business Logic Examples
- **User Management**: Email validation, name normalization, external verification
- **Order Processing**: Fraud detection, inventory integration, payment notifications
- **Sensitive Data**: PII detection, encryption requirements, audit trails
- **Generic Collections**: Flexible processing for any collection type

## 🏗️ Architecture

### Plugin Structure
```
typescript-demo-plugin/
├── src/
│   └── lib.ts              # Main plugin implementation
├── dist/
│   ├── plugin.wasm         # Compiled WebAssembly module
│   └── lib.js              # Compiled JavaScript (for testing)
├── package.json            # Dependencies and build scripts
├── tsconfig.json           # TypeScript configuration
└── README.md              # This file
```

### Key Components

#### 1. Data Validator
```typescript
class DataValidator {
  static validateUserData(data: any): { valid: boolean; errors: string[] }
  static isValidEmail(email: string): boolean
}
```

#### 2. Data Transformer
```typescript
class DataTransformer {
  static normalizeUserData(data: any): any
  static toTitleCase(str: string): string
}
```

#### 3. Third-Party Integration
```typescript
class ThirdPartyIntegration {
  static async enrichUserData(data: any): Promise<any>
  static async validateWithExternalService(data: any): Promise<boolean>
}
```

## 🔧 Building the Plugin

### Prerequisites
```bash
# Install Node.js and npm
# Install AssemblyScript for WebAssembly compilation
npm install -g assemblyscript
```

### Build Process
```bash
# Install dependencies
npm install

# Build TypeScript and WebAssembly
npm run build

# Development mode (watch for changes)
npm run dev

# Clean build artifacts
npm run clean
```

### Build Output
- `dist/plugin.wasm` - WebAssembly module for OxideDB
- `dist/lib.js` - JavaScript version for testing
- `dist/lib.d.ts` - TypeScript definitions

## 🎯 Plugin Functions

### Core Event Handlers

#### `on_before_create()`
Processes data before record creation:
- Validates input data
- Transforms and normalizes fields
- Applies business rules
- Enriches with external data
- Returns modified data or error

#### `on_after_create()`
Handles post-creation tasks:
- Sends notifications
- Updates external systems
- Triggers workflows
- Logs audit events

#### `plugin_init()`
Initializes plugin state and capabilities

#### `plugin_cleanup()`
Cleans up resources when plugin is unloaded

### Collection-Specific Processing

#### Users Collection
```typescript
// Validation rules
- Email format validation
- Required field checking
- Age range validation

// Transformations
- Email normalization (lowercase)
- Name title case conversion
- Metadata enrichment

// External Integration
- External validation service
- Account status assignment
- Risk scoring
```

#### Orders Collection
```typescript
// Validation rules
- Required fields (customer_id, items, total)
- Positive total amount
- Item structure validation

// Business Logic
- Order ID generation
- Fraud score calculation
- Status assignment

// Integration
- Inventory updates
- Payment processor notifications
```

#### Sensitive Data Collection
```typescript
// Security Processing
- PII pattern detection
- Encryption requirement flagging
- Audit trail creation
- Security level assignment
```

## 🔒 Security Features

The plugin demonstrates several security best practices:

1. **Input Validation**: All data is validated before processing
2. **PII Detection**: Automatic detection of sensitive patterns
3. **Error Handling**: Secure error messages without data leakage
4. **Audit Logging**: Comprehensive logging for security events
5. **Risk Assessment**: Automated risk scoring and flagging

## 🧪 Testing the Plugin

### Load the Plugin
```bash
# Copy the compiled WASM file to OxideDB
cp dist/plugin.wasm /path/to/oxidedb/plugins/

# Start OxideDB with plugin loading
./oxidedb --load-plugin typescript-demo-plugin:plugins/plugin.wasm
```

### Test User Creation
```bash
# Valid user creation
curl -X POST http://localhost:8080/collections/users/records \
  -H "Content-Type: application/json" \
  -d '{
    "name": "john doe",
    "email": "JOHN.DOE@EXAMPLE.COM",
    "age": 30
  }'

# Expected: Normalized data with lowercase email and title case name
```

### Test Validation Errors
```bash
# Invalid email
curl -X POST http://localhost:8080/collections/users/records \
  -H "Content-Type: application/json" \
  -d '{
    "name": "John Doe",
    "email": "invalid-email",
    "age": 30
  }'

# Expected: Validation error for invalid email format
```

### Test Order Processing
```bash
# Valid order
curl -X POST http://localhost:8080/collections/orders/records \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "cust_123",
    "items": [{"id": "item_1", "quantity": 2}],
    "total": 99.99
  }'

# Expected: Order with fraud score and processing metadata
```

### Test Sensitive Data
```bash
# Data with PII patterns
curl -X POST http://localhost:8080/collections/sensitive_data/records \
  -H "Content-Type: application/json" \
  -d '{
    "user_id": "user_123",
    "ssn": "123-45-6789",
    "notes": "Sensitive information"
  }'

# Expected: Error due to PII pattern detection
```

## 📊 Plugin Capabilities Assessment

### ✅ Current Strengths
1. **WebAssembly Runtime**: Uses Wasmtime for secure execution
2. **Language Agnostic**: Supports multiple languages via WASM
3. **Event-Driven**: Integrates with hook-first architecture
4. **Memory Management**: Proper host-plugin memory communication
5. **Error Handling**: Comprehensive error propagation
6. **Logging**: Built-in logging capabilities

### ⚠️ Security Gaps Identified
1. **No Capability System**: Plugins have unrestricted access to host functions
2. **Missing Sandboxing**: No explicit resource limitations
3. **No Permission Model**: All plugins can access all host functions
4. **No Network Restrictions**: No control over external communications
5. **No File System Isolation**: No explicit file system restrictions

### 🔧 Recommended Enhancements

#### 1. Capability-Based Security
```rust
// Plugin manifest with explicit capabilities
[plugin.capabilities]
allow_http_requests = ["api.stripe.com", "api.sendgrid.com"]
allow_file_access = false
allow_network_access = ["outbound_only"]
max_memory_mb = 64
max_execution_time_ms = 5000
```

#### 2. Resource Limits
```rust
// Runtime configuration
struct PluginLimits {
    max_memory: usize,
    max_execution_time: Duration,
    max_host_calls: u32,
    allowed_host_functions: Vec<String>,
}
```

#### 3. Permission System
```rust
// Host function access control
if !plugin.has_capability("http_request") {
    return Err(PluginError::PermissionDenied);
}
```

## 🚀 Future Enhancements

1. **Async Support**: Enable async operations in plugins
2. **Streaming Data**: Support for large data processing
3. **Plugin Communication**: Inter-plugin messaging
4. **Hot Reloading**: Dynamic plugin updates
5. **Performance Monitoring**: Plugin execution metrics
6. **Capability Grants**: Runtime capability management

## 📝 Conclusion

This TypeScript demo plugin showcases the powerful capabilities of the OxideDB plugin system while highlighting areas for security enhancement. The current system provides a solid foundation for extensible database operations, but implementing a capability-based security model would make it production-ready for untrusted code execution.

The plugin demonstrates real-world use cases including data validation, transformation, external integrations, and security processing - all essential features for modern database applications.