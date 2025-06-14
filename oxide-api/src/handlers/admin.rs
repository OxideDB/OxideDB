//! Admin UI and static file handlers
//!
//! This module provides handlers for serving the admin UI interface
//! and static assets. The admin UI is embedded at compile time for
//! easy deployment.

use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
};
use include_dir::{include_dir, Dir};
use mime_guess::from_path;
use tracing::{debug, warn};

// Embed the UI files at compile time
static UI_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../ui/dist");

/// Serve the admin UI index page
///
/// GET /admin
pub async fn serve_admin_ui() -> impl IntoResponse {
    debug!("Serving admin UI index page");

    match UI_DIR.get_file("index.html") {
        Some(file) => match file.contents_utf8() {
            Some(content) => {
                debug!("Successfully served admin UI index page");
                Html(content.to_string()).into_response()
            }
            None => {
                warn!("Failed to read index.html as UTF-8");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to read index.html",
                )
                    .into_response()
            }
        },
        None => {
            warn!("Admin UI index.html not found in embedded files");
            (StatusCode::NOT_FOUND, "Admin UI not found").into_response()
        }
    }
}

/// Serve static files for the admin UI
///
/// GET /admin/*path
pub async fn serve_admin_static(Path(path): Path<String>) -> impl IntoResponse {
    // Remove leading slash if present
    let clean_path = path.strip_prefix('/').unwrap_or(&path);
    debug!("Serving admin static file: {}", clean_path);

    match UI_DIR.get_file(clean_path) {
        Some(file) => {
            let mime_type = from_path(file.path()).first_or_octet_stream();
            debug!(
                "Serving static file: {} (type: {})",
                clean_path,
                mime_type.as_ref()
            );

            match Response::builder()
                .header(header::CONTENT_TYPE, mime_type.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=31536000") // Cache for 1 year
                .body(Body::from(file.contents().to_vec()))
            {
                Ok(response) => response.into_response(),
                Err(e) => {
                    warn!("Failed to build response for {}: {}", clean_path, e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to build response",
                    )
                        .into_response()
                }
            }
        }
        None => {
            debug!("Static file not found: {}, serving index.html for SPA routing", clean_path);
            
            // For SPA routing, serve index.html for any unmatched routes
            match UI_DIR.get_file("index.html") {
                Some(file) => match file.contents_utf8() {
                    Some(content) => {
                        match Response::builder()
                            .header(header::CONTENT_TYPE, "text/html")
                            .body(Body::from(content.as_bytes().to_vec()))
                        {
                            Ok(response) => response.into_response(),
                            Err(e) => {
                                warn!("Failed to build SPA fallback response: {}", e);
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Failed to build response",
                                )
                                    .into_response()
                            }
                        }
                    }
                    None => {
                        warn!("Failed to read index.html as UTF-8 for SPA fallback");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Failed to read index.html",
                        )
                            .into_response()
                    }
                },
                None => {
                    warn!("Admin UI not found for SPA fallback");
                    (StatusCode::NOT_FOUND, "Admin UI not found").into_response()
                }
            }
        }
    }
}

/// Check if admin UI is available
pub fn is_admin_ui_available() -> bool {
    UI_DIR.get_file("index.html").is_some()
}

/// Get admin UI build information
pub fn get_admin_ui_info() -> Option<AdminUiInfo> {
    if !is_admin_ui_available() {
        return None;
    }

    // Try to read build info from a manifest file if it exists
    let build_time = UI_DIR
        .get_file("build-info.json")
        .and_then(|f| f.contents_utf8())
        .and_then(|content| {
            serde_json::from_str::<serde_json::Value>(content)
                .ok()
                .and_then(|v| v.get("build_time").and_then(|t| t.as_str().map(String::from)))
        });

    Some(AdminUiInfo {
        available: true,
        build_time,
        file_count: UI_DIR.files().count(),
    })
}

/// Admin UI information
#[derive(Debug, serde::Serialize)]
pub struct AdminUiInfo {
    /// Whether the admin UI is available
    pub available: bool,
    /// Build timestamp if available
    pub build_time: Option<String>,
    /// Number of embedded files
    pub file_count: usize,
} 