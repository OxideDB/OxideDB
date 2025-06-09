//! OxideDB - A hook-first database with plugin architecture
//!
//! This is the main entry point for OxideDB. It demonstrates the integration
//! of the core architecture components implemented in Milestone 3.

use oxide_api::ApiServer;
use oxide_core::{AppError, Event, EventBus, InMemoryEventBus};
use oxide_db::{Db, SqliteDb};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // Initialize logging
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Starting OxideDB - Milestone 4");

    // Create the event bus - the heart of our hook-first architecture
    let event_bus: Arc<dyn EventBus> = Arc::new(InMemoryEventBus::new());
    info!("✅ Event bus initialized");

    // Register a sample event listener to demonstrate hooking
    event_bus.subscribe(
        "BeforeRecordCreate",
        Box::new(|event| {
            if let Event::BeforeRecordCreate { collection, data } = event {
                info!(
                    "🎣 Hook triggered: About to create record in collection '{}' with data: {}",
                    collection, data
                );
            }
            Ok(())
        }),
    )?;
    info!("✅ Sample event listener registered");

    // Initialize the database with event integration
    let database_path = ":memory:"; // Use in-memory SQLite for demo
    let database: Arc<dyn Db> = Arc::new(SqliteDb::new(database_path, Arc::clone(&event_bus))?);
    database.initialize().await?;
    info!("✅ SQLite database initialized with event integration");

    // Create and start the API server
    info!("🚀 Starting API server...");
    let api_server = ApiServer::new(
        Arc::clone(&database),
        Arc::clone(&event_bus),
        "127.0.0.1".to_string(),
        8080,
    );
    
    info!("🎉 Milestone 4 startup completed successfully!");
    info!("Architecture summary:");
    info!("  ✅ Cargo workspace with 3 crates (oxide-core, oxide-db, oxide-api)");
    info!("  ✅ Hook-first architecture with EventBus");
    info!("  ✅ Database abstraction with SQLite implementation");
    info!("  ✅ Standardized error handling with AppError");
    info!("  ✅ Full async support with Tokio");
    info!("  ✅ All operations route through events for extensibility");
    info!("  ✅ HTTP API server with health check endpoint");
    info!("  ✅ REST API for collections and records");
    info!("");
    info!("🌐 API server is running at: http://127.0.0.1:8080");
    info!("📋 Available endpoints:");
    info!("  - GET  /health                              - Health check");
    info!("  - GET  /collections                         - List collections");
    info!("  - POST /collections                         - Create collection");
    info!("  - DEL  /collections/{{collection}}            - Delete collection");
    info!("  - GET  /collections/{{collection}}/stats      - Collection stats");
    info!("  - GET  /collections/{{collection}}/records    - List records");
    info!("  - POST /collections/{{collection}}/records    - Create record");
    info!("  - GET  /collections/{{collection}}/records/{{id}} - Get record");
    info!("  - PUT  /collections/{{collection}}/records/{{id}} - Update record");
    info!("  - DEL  /collections/{{collection}}/records/{{id}} - Delete record");
    info!("");

    // Start the server (this will run until the process is terminated)
    api_server.start().await?;

    Ok(())
}
