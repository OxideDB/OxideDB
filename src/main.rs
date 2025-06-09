//! OxideDB - A hook-first database with plugin architecture
//!
//! This is the main entry point for OxideDB. It demonstrates the integration
//! of the core architecture components implemented in Milestone 2.

use oxide_core::{AppError, InMemoryEventBus, Event};
use oxide_db::{Db, SimpleDb};
use oxide_api::ApiServer;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    // 1. Initialize the logger
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting OxideDB - Milestone 4: Basic Web Server");

    // 2. Initialize the EventBus
    let event_bus = Arc::new(InMemoryEventBus::new());
    info!("✅ Event bus initialized");

    // Register a sample event listener to demonstrate hooking
    event_bus.subscribe("BeforeRecordCreate", Box::new(|event| {
        if let Event::BeforeRecordCreate { collection, data } = event {
            info!("🎣 Hook triggered: About to create record in collection '{}' with data: {}", collection, data);
        }
        Ok(())
    }))?;
    
    event_bus.subscribe("AfterRecordCreate", Box::new(|event| {
        if let Event::AfterRecordCreate { collection, record } = event {
            info!("🎣 Hook triggered: Record created in collection '{}' with ID: {}", collection, record.id);
        }
        Ok(())
    }))?;
    info!("✅ Sample event listeners registered");

    // 3. Initialize the database connection
    let database = Arc::new(SimpleDb::new(Arc::clone(&event_bus)));
    database.initialize().await?;
    info!("✅ Database initialized with event integration");

    // Demonstrate the working system by creating a sample record
    info!("🚀 Demonstrating hook-first architecture...");
    
    // This will trigger BeforeRecordCreate and AfterRecordCreate events
    let sample_data = serde_json::json!({
        "name": "John Doe",
        "email": "john@example.com",
        "created_at": "2024-01-01T00:00:00Z"
    });
    
    let record = database.create_record("users", sample_data).await?;
    info!("✅ Sample record created with ID: {}", record.id);

    // Show event bus statistics
    info!("📊 Event bus statistics:");
    info!("  - Events dispatched: {}", event_bus.events_dispatched());
    info!("  - Listeners for BeforeRecordCreate: {}", event_bus.listener_count("BeforeRecordCreate"));

    // 4. Start the oxide-api web server
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
    info!("  ✅ Web server with /health endpoint");
    info!("");
    info!("🌐 API Endpoints available:");
    info!("  - GET  /health");
    info!("  - GET  /collections");
    info!("  - POST /collections");
    info!("  - GET  /collections/{{collection}}/records");
    info!("  - POST /collections/{{collection}}/records");
    info!("Server starting on http://{}...", api_server.address());

    // This will run indefinitely until interrupted
    api_server.start().await?;

    Ok(())
} 