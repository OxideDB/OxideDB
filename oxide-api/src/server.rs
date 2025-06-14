//! HTTP server implementation

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get},
    Router,
};
use oxide_core::{event::EventBus, AppError};
use oxide_db::Db;

use std::sync::Arc;
use tokio::net::TcpListener;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};

use crate::handlers::{
    CollectionHandlers, CollectionStats, HealthHandlers, HealthStatus, RecordHandlers,
};
use oxide_core::event::{RecordData, RecordId};
use oxide_core::CollectionSchema;
use oxide_db::db::ListParams;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<dyn Db>,
    pub event_bus: Arc<dyn EventBus>,
}

/// The API server that handles HTTP requests
///
/// This server provides a REST API for interacting with the OxideDB database.
/// It integrates with the event system and database abstraction layer to
/// provide a complete HTTP interface.
pub struct ApiServer {
    db: Arc<dyn Db>,
    event_bus: Arc<dyn EventBus>,
    host: String,
    port: u16,
}

impl ApiServer {
    /// Create a new API server instance
    ///
    /// # Arguments
    /// * `db` - The database implementation to use
    /// * `event_bus` - The event bus for dispatching events
    /// * `host` - The host address to bind to
    /// * `port` - The port to listen on
    pub fn new(db: Arc<dyn Db>, event_bus: Arc<dyn EventBus>, host: String, port: u16) -> Self {
        Self {
            db,
            event_bus,
            host,
            port,
        }
    }

    /// Start the API server
    ///
    /// This method starts the HTTP server and begins listening for requests.
    /// It will run indefinitely until stopped.
    pub async fn start(&self) -> Result<(), AppError> {
        info!("Starting API server on {}:{}", self.host, self.port);

        let state = AppState {
            db: Arc::clone(&self.db),
            event_bus: Arc::clone(&self.event_bus),
        };

        let app = Router::new()
            // Health endpoint - critical for Task 4.1
            .route("/health", get(health_check))
            // Collection endpoints
            .route(
                "/collections",
                get(list_collections).post(create_collection),
            )
            .route("/collections/:collection", delete(delete_collection))
            .route("/collections/:collection/stats", get(collection_stats))
            .route("/collections/:collection/schema", get(collection_schema).put(update_collection_schema))
            // Record endpoints
            .route(
                "/collections/:collection/records",
                get(list_records).post(create_record),
            )
            .route(
                "/collections/:collection/records/:id",
                get(get_record).put(update_record).delete(delete_record),
            )
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CorsLayer::permissive()),
            )
            .with_state(state);

        let listener = TcpListener::bind(&self.address()).await.map_err(|e| {
            AppError::internal(format!("Failed to bind to {}: {}", self.address(), e))
        })?;

        info!("✅ API server listening on {}", self.address());

        axum::serve(listener, app)
            .await
            .map_err(|e| AppError::internal(format!("Server error: {}", e)))?;

        Ok(())
    }

    /// Stop the API server gracefully
    pub async fn stop(&self) -> Result<(), AppError> {
        info!("Stopping API server");

        // TODO: Implement graceful shutdown
        // In a real implementation, this would:
        // 1. Stop accepting new connections
        // 2. Wait for existing requests to complete
        // 3. Close the database connection
        // 4. Shutdown the event bus

        Ok(())
    }

    /// Get the database instance
    pub fn db(&self) -> &Arc<dyn Db> {
        &self.db
    }

    /// Get the event bus instance
    pub fn event_bus(&self) -> &Arc<dyn EventBus> {
        &self.event_bus
    }

    /// Get the server address
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

// HTTP Handler Functions

/// Health check endpoint - verifies database connection
async fn health_check(
    State(state): State<AppState>,
) -> Result<Json<HealthStatus>, (StatusCode, String)> {
    match HealthHandlers::health_check(state.db).await {
        Ok(status) => {
            if status.database.starts_with("unhealthy") {
                warn!("Health check failed: database unhealthy");
                Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Database connection failed".to_string(),
                ))
            } else {
                Ok(Json(status))
            }
        }
        Err(e) => {
            warn!("Health check error: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

/// List all collections
async fn list_collections(
    State(state): State<AppState>,
) -> Result<Json<Vec<String>>, (StatusCode, String)> {
    match CollectionHandlers::list_collections(state.db).await {
        Ok(collections) => Ok(Json(collections)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Create a new collection
async fn create_collection(
    State(state): State<AppState>,
    Json(schema): Json<CollectionSchema>,
) -> Result<StatusCode, (StatusCode, String)> {
    match CollectionHandlers::create_collection(state.db, schema).await {
        Ok(()) => Ok(StatusCode::CREATED),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Delete a collection
async fn delete_collection(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    match CollectionHandlers::delete_collection(state.db, collection).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Get collection statistics
async fn collection_stats(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<Json<CollectionStats>, (StatusCode, String)> {
    match CollectionHandlers::get_collection_stats(state.db, collection).await {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => Err((StatusCode::NOT_FOUND, e.to_string())),
    }
}

/// Get collection schema
async fn collection_schema(
    State(state): State<AppState>,
    Path(collection): Path<String>,
) -> Result<Json<CollectionSchema>, (StatusCode, String)> {
    match CollectionHandlers::get_collection_schema(state.db, collection).await {
        Ok(schema) => Ok(Json(schema)),
        Err(e) => Err((StatusCode::NOT_FOUND, e.to_string())),
    }
}

/// Update collection schema
async fn update_collection_schema(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(schema): Json<CollectionSchema>,
) -> Result<StatusCode, (StatusCode, String)> {
    match CollectionHandlers::update_collection_schema(state.db, collection, schema).await {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// List records in a collection
async fn list_records(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Query(params): Query<ListParams>,
) -> Result<Json<Vec<oxide_db::Record>>, (StatusCode, String)> {
    match RecordHandlers::list_records(state.db, collection, params).await {
        Ok(records) => Ok(Json(records)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Create a new record
async fn create_record(
    State(state): State<AppState>,
    Path(collection): Path<String>,
    Json(data): Json<RecordData>,
) -> Result<Json<oxide_db::Record>, (StatusCode, String)> {
    match RecordHandlers::create_record(state.db, collection, data).await {
        Ok(record) => Ok(Json(record)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Get a specific record
async fn get_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, RecordId)>,
) -> Result<Json<oxide_db::Record>, (StatusCode, String)> {
    match RecordHandlers::get_record(state.db, collection, id).await {
        Ok(record) => Ok(Json(record)),
        Err(e) => Err((StatusCode::NOT_FOUND, e.to_string())),
    }
}

/// Update a specific record
async fn update_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, RecordId)>,
    Json(data): Json<RecordData>,
) -> Result<Json<oxide_db::Record>, (StatusCode, String)> {
    match RecordHandlers::update_record(state.db, collection, id, data).await {
        Ok(record) => Ok(Json(record)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// Delete a specific record
async fn delete_record(
    State(state): State<AppState>,
    Path((collection, id)): Path<(String, RecordId)>,
) -> Result<Json<oxide_db::Record>, (StatusCode, String)> {
    match RecordHandlers::delete_record(state.db, collection, id).await {
        Ok(record) => Ok(Json(record)),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}
