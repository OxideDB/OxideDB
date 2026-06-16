# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

### Rust Backend
```bash
# Build entire workspace
cargo build

# Build with release optimizations
cargo build --release

# Run main application (starts server on port 8080)
cargo run --bin oxidedb start

# Run with debug logging
cargo run --bin oxidedb start --log-level debug

# Run tests
cargo test

# Code quality checks
cargo clippy
cargo clippy -- -D warnings  # Fail on warnings (project requirement)
cargo fmt

# Build plugins (example WASM plugins)
cargo build --release --target wasm32-unknown-unknown --bin hello-plugin
```

### Frontend (React UI)
```bash
cd ui

# Install dependencies
npm install

# Development server (port 3000)
npm run dev

# Production build
npm run build

# Lint code
npm run lint

# Preview production build
npm run preview
```

## Architecture Overview

OxideDB is a **hook-first database** with a plugin architecture built in Rust. The core architectural principle is that all database operations must dispatch `Before...` and `After...` events through a central `EventBus` for maximum extensibility.

### Crate Structure
- **`oxide-core/`**: Core abstractions, event system, auth, plugin API contracts, VFS types. No knowledge of databases or web servers.
- **`oxide-db/`**: Database implementations (SQLite). Depends on `oxide-core` only.
- **`oxide-api/`**: HTTP API server using Axum. Depends on both `oxide-db` and `oxide-core`.
- **`oxide-logging/`**: Centralized logging system with API endpoints and retention.
- **`oxide-vfs/`**: Virtual File System with namespace isolation, deduplication, and quota management.
- **`oxide-plugin-runtime/`**: WebAssembly plugin runtime environment with host functions and security.
- **`oxide-plugin-sdk/`**: High-level SDK for developing WASM plugins with trait-based event handling.
- **`oxide-typegen/`**: TypeScript binding generation for the frontend.
- **`oxidedb/`**: Main binary that integrates all components and WASM plugin runtime.
- **`hello-plugin/`**: Example WASM plugin demonstrating the plugin system.
- **`ui/`**: React 19 admin interface with TypeScript, Vite, TailwindCSS, and comprehensive UI components.

### Event System
All core business logic is implemented through the event system:
- Every database operation triggers `BeforeCreate`, `AfterCreate`, etc. events
- Plugins can subscribe to these events to modify behavior
- Internal core listeners implement the actual database operations
- This ensures complete extensibility without code modification

### Plugin System
- WASM-based plugins can be loaded at runtime
- High-level SDK (`oxide-plugin-sdk`) provides trait-based event handling
- Plugins can handle HTTP requests through dedicated HTTP handler traits
- Runtime environment (`oxide-plugin-runtime`) provides secure execution with host functions
- Plugin API contract is versioned and treated as immutable
- Security sandbox prevents malicious plugin behavior
- Support for database operations, logging, and VFS access from plugins

## Key Development Rules

### Code Quality (Non-negotiable)
- **No `.unwrap()` or `.expect()`** in application code - use `?` operator and custom `AppError` types
- **All code must be clippy-clean** with zero warnings (`cargo clippy -- -D warnings`)
- **All public functions/structs/traits must have doc comments** (`///`)
- **Security first**: treat all external input as malicious until proven otherwise
- **No blocking operations in async code** - use `tokio::task::spawn_blocking` when needed

### Architecture Rules
- **Hook-first principle**: Never implement core business logic directly - always dispatch events
- **Correct crate placement**: Logic must go in the appropriate crate based on dependencies
- **Define traits before implementations**: Abstract first, then implement
- **Plugin API is immutable**: Changes require versioning and formal design discussion

## Testing

Run the full test suite:
```bash
cargo test
```

For testing the plugin system, the hello-plugin will automatically reject all record creation operations with an error message, demonstrating the event hook integration.

## API Endpoints

The HTTP API runs on port 8080:
- `GET /health` - System health check
- `GET /collections` - List all collections  
- `POST /collections` - Create new collection
- `DELETE /collections/{name}` - Delete collection
- `GET /collections/{name}/records` - List records in collection
- `POST /collections/{name}/records` - Create record
- `GET/PUT/DELETE /collections/{name}/records/{id}` - Individual record operations
- `GET /logs/*` - Logging API endpoints with pagination and filtering
- `GET /plugins` - List installed plugins
- `POST /plugins/install` - Install plugin from ZIP
- `DELETE /plugins/{name}` - Uninstall plugin
- `GET /plugins/{name}/config` - Get plugin configuration
- `PUT /plugins/{name}/config` - Update plugin configuration
- `POST /plugins/{name}/enable` - Enable plugin
- `POST /plugins/{name}/disable` - Disable plugin
- Authentication endpoints for user management and JWT tokens

## UI Integration

The React frontend connects to the Rust backend API and provides:
- Collections management (CRUD operations)
- Records browsing and editing with JSON editor
- Plugin management interface with installation, configuration, and status monitoring
- Permissions and authentication management
- Real-time health monitoring
- Comprehensive audit logging interface with search and filtering
- Settings management
- Responsive design with dark/light theme support using shadcn/ui components

## Environment Setup

Required tools:
- Rust 1.93+ with `wasm32-unknown-unknown` target for plugins
- Node.js 20.19.x, 22.13+, or 24+ with npm for the frontend
- SQLite database is embedded, no external setup needed

## Common Development Workflow

1. Make changes to Rust code in appropriate crate
2. Run `cargo clippy` and `cargo fmt` 
3. Run `cargo test` to ensure tests pass
4. For UI changes, run `npm run lint` in the `ui/` directory
5. Test integration by running both backend (`cargo run --bin oxidedb`) and frontend (`npm run dev`)
6. Plugin changes require rebuilding with `--target wasm32-unknown-unknown`
7. Use the VFS system for file operations within collections to maintain security and isolation
