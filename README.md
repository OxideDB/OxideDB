# OxideDB

A hook-first database with plugin architecture built in Rust, featuring a modern React-based admin interface.

## 🚀 Milestone 3: Frontend Implementation

This milestone implements a complete React-based admin UI for OxideDB, providing a modern web interface for database management.

## 🏗️ Architecture

OxideDB follows a modular, hook-first architecture:

```
oxidebase/
├── oxide-core/         # Core abstractions and event system
├── oxide-db/           # Database implementations (SQLite)
├── oxide-api/          # HTTP API server
├── oxidedb/           # Main binary integration
├── hello-plugin/      # Example WASM plugin
└── ui/                # React admin interface (NEW!)
```

## ✨ Features

### Backend
- **Hook-First Architecture**: All operations dispatch events for extensibility
- **Plugin System**: WASM-based plugins for custom business logic
- **Database Abstraction**: Pluggable database backends (SQLite implemented)
- **REST API**: Full HTTP API for all database operations
- **Type Safety**: Comprehensive error handling with custom types
- **Async Support**: Built on Tokio for high performance

### Frontend (NEW!)
- **Modern React UI**: Built with React 19, TypeScript, and Vite
- **Collections Management**: Create, view, and delete collections
- **Records CRUD**: Full create, read, update, delete operations
- **Health Monitoring**: Real-time system status
- **Responsive Design**: Mobile-friendly with TailwindCSS
- **Type Safety**: Full TypeScript integration with backend API

## 🛠️ Technology Stack

### Backend
- **Rust** - Systems programming language
- **Tokio** - Async runtime
- **Axum** - Web framework
- **SQLite** - Database (via rusqlite)
- **Wasmtime** - WASM plugin runtime
- **Serde** - Serialization

### Frontend
- **React 19** - UI framework
- **TypeScript** - Type safety
- **Vite** - Build tool and dev server
- **React Router** - Client-side routing
- **TailwindCSS** - Utility-first CSS
- **Lucide React** - Icons

## 📋 Prerequisites

- **Rust** 1.75+ with `wasm32-unknown-unknown` target
- **Node.js** 18+ and npm
- **Git**

## 🚀 Quick Start

### 1. Clone and Setup

```bash
git clone <repository-url>
cd oxidebase
```

### 2. Build the Backend

```bash
# Install WASM target for plugins
rustup target add wasm32-unknown-unknown

# Build the hello plugin
cd hello-plugin
cargo build --release --target wasm32-unknown-unknown
cd ..

# Build and run the main application
cargo run --bin oxidedb start
```

The backend will start on `http://localhost:8080`

### Production JWT Configuration

Set a strong `JWT_SECRET` before exposing OxideDB beyond local development. New JWTs include a `kid` header so deployments can rotate keys without immediately invalidating every session:

```bash
export JWT_SECRET="new-active-secret-at-least-32-characters"
export OXIDEDB_JWT_KEY_ID="2026-06-rotation"
export OXIDEDB_JWT_PREVIOUS_KEYS='{"2026-03-rotation":"old-secret-at-least-32-characters"}'
```

`JWT_SECRET` is the active signing key. `OXIDEDB_JWT_PREVIOUS_KEYS` is a JSON object of previous key ids to secrets that should remain accepted during the rotation window. Remove old keys after their tokens have expired.

### Container Deployment

The root `Dockerfile` builds the React admin UI and the `oxidedb` binary in separate stages, then ships a slim runtime image. Persistent state is kept outside the image:

- `/data` for SQLite data
- `/plugins` for installed plugin packages
- `/logs` for audit/application logs

Run with Compose using an env file based on `.env.production.example`:

```bash
docker compose --env-file .env.production up -d --build
```

For a local smoke test without a TLS proxy:

```bash
docker build -t oxidedb:prod .
docker run --rm -p 8080:8080 \
  -e JWT_SECRET="replace-with-a-random-secret-at-least-32-characters" \
  -e OXIDEDB_REQUIRE_HTTPS=false \
  -v oxidedb-data:/data \
  -v oxidedb-plugins:/plugins \
  -v oxidedb-logs:/logs \
  oxidedb:prod
```

The image defaults to `OXIDEDB_ENV=production`, secure cookies, strict plugin policy, and HTTPS enforcement. Keep `OXIDEDB_REQUIRE_HTTPS=true` for internet-exposed deployments behind a TLS proxy that sends `X-Forwarded-Proto: https`.

### 3. Setup and Run the Frontend

```bash
# Navigate to UI directory
cd ui

# Install dependencies
npm install

# Start development server
npm run dev
```

The frontend will be available at `http://localhost:3000`

## 📱 Using the Admin UI

1. **Collections Page**: 
   - View all collections
   - Create new collections
   - Delete existing collections
   - View collection statistics

2. **Records Management**:
   - Browse records in any collection
   - Create new records with JSON data
   - Edit existing records
   - Delete records

3. **Health Monitoring**:
   - Check system status
   - Monitor database connectivity
   - View system information

## 🔌 API Endpoints

The REST API provides the following endpoints:

```
GET    /health                              # Health check
GET    /collections                         # List collections
POST   /collections                         # Create collection
DELETE /collections/{collection}            # Delete collection
GET    /collections/{collection}/stats      # Collection statistics
GET    /collections/{collection}/records    # List records
POST   /collections/{collection}/records    # Create record
GET    /collections/{collection}/records/{id} # Get record
PUT    /collections/{collection}/records/{id} # Update record
DELETE /collections/{collection}/records/{id} # Delete record
```

## 🧪 Testing the Plugin System

The hello-plugin demonstrates the WASM plugin integration:

```bash
# The plugin will automatically:
# 1. Load during startup
# 2. Register for BeforeRecordCreate events  
# 3. Reject operations with an error message
# 4. Log all interactions

# Try creating a record to see the plugin in action
curl -X POST http://localhost:8080/collections/test/records \
  -H "Content-Type: application/json" \
  -d '{"name": "test", "value": 42}'
```

## 🏗️ Development

### Backend Development

```bash
# Run with debug logging
cargo run --bin oxidedb start --log-level debug

# Run tests
cargo test

# Check code
cargo clippy
cargo fmt
```

### Frontend Development

```bash
cd ui

# Start dev server
npm run dev

# Build for production
npm run build

# Preview production build
npm run preview

# Lint code
npm run lint
```

## 📁 Project Structure

```
oxidebase/
├── oxide-core/
│   ├── src/
│   │   ├── lib.rs           # Core abstractions
│   │   ├── error.rs         # Error types
│   │   ├── event/           # Event system
│   │   ├── auth.rs          # Authentication
│   │   └── plugin_api/      # Plugin contracts
│   └── Cargo.toml
├── oxide-db/
│   ├── src/
│   │   ├── lib.rs           # Database abstractions
│   │   ├── sqlite.rs        # SQLite implementation
│   │   └── db.rs            # Core traits
│   └── Cargo.toml
├── oxide-api/
│   ├── src/
│   │   ├── lib.rs           # API module exports
│   │   ├── server.rs        # HTTP server
│   │   ├── handlers/        # Request handlers
│   │   └── middleware/      # HTTP middleware
│   └── Cargo.toml
├── oxidedb/
│   ├── src/
│   │   ├── main.rs          # Main application
│   │   └── plugin_runtime.rs # WASM integration
│   └── Cargo.toml
├── hello-plugin/
│   ├── src/
│   │   └── lib.rs           # Example plugin
│   └── Cargo.toml
├── ui/                      # React Admin Interface
│   ├── src/
│   │   ├── components/      # UI components
│   │   ├── pages/           # Route pages
│   │   ├── services/        # API client
│   │   ├── types/           # TypeScript types
│   │   ├── App.tsx          # Main app
│   │   └── main.tsx         # Entry point
│   ├── public/              # Static assets
│   ├── package.json
│   ├── vite.config.ts
│   └── tailwind.config.js
└── Cargo.toml               # Workspace config
```

## 🎯 Key Architectural Principles

1. **Hook-First Design**: Every operation triggers events for maximum extensibility
2. **Plugin Architecture**: WASM plugins can modify behavior without recompilation
3. **Type Safety**: Comprehensive error handling and type definitions
4. **Modularity**: Clear separation between core, database, API, and UI layers
5. **Modern Stack**: Latest versions of Rust, React, and supporting tools

## 🚦 Health Check

You can verify the system is working by:

1. **Backend Health**: `curl http://localhost:8080/health`
2. **Frontend Access**: Visit `http://localhost:3000`
3. **Plugin Integration**: Create a collection and record (should be rejected by plugin)

## 🔮 Future Enhancements

- User authentication and authorization
- Real-time updates via WebSockets
- Advanced query capabilities
- Plugin marketplace
- Performance analytics
- Database migrations
- Backup and restore functionality

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

**🎉 Milestone 3 Complete!** 

The frontend implementation provides a complete admin interface for OxideDB, demonstrating the full stack integration of the hook-first architecture with a modern React UI. 

graph TB
    subgraph "Frontend Layer"
        UI[React Admin UI<br/>Port 3000]
        UI --> |HTTP API Calls| API
    end
    
    subgraph "Backend Layer"
        API[Axum API Server<br/>Port 8080]
        DB[SQLite Database]
        EVENTS[Event Bus<br/>Hook System]
        PLUGINS[WASM Plugin<br/>Runtime]
        
        API --> |Database Operations| DB
        API --> |Emit Events| EVENTS
        EVENTS --> |Load & Execute| PLUGINS
        PLUGINS --> |Modify Operations| EVENTS
    end
    
    subgraph "Crate Architecture"
        CORE[oxide-core<br/>Events, Auth, Types]
        DBCRATE[oxide-db<br/>Database Abstraction]
        APICRATE[oxide-api<br/>HTTP Handlers]
        MAIN[oxidedb<br/>Main Binary]
        PLUGIN[hello-plugin<br/>WASM Plugin]
        
        MAIN --> APICRATE
        MAIN --> DBCRATE
        MAIN --> CORE
        APICRATE --> DBCRATE
        APICRATE --> CORE
        DBCRATE --> CORE
        PLUGIN --> CORE
    end
    
    subgraph "Features"
        F1[Collections CRUD]
        F2[Records Management] 
        F3[Health Monitoring]
        F4[Plugin System]
        F5[Type Safety]
        F6[Modern UI/UX]
    end
    
    UI -.-> F1
    UI -.-> F2
    UI -.-> F3
    API -.-> F4
    CORE -.-> F5
    UI -.-> F6
