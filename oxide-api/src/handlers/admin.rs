//! Admin UI and static file handlers
//!
//! This module provides handlers for serving the admin UI interface
//! and static assets. The admin UI can be embedded at compile time for
//! easy deployment or served from external filesystem for development.

use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
};
use include_dir::{include_dir, Dir};
use mime_guess::from_path;
use tracing::{debug, warn, error};
use std::path::PathBuf;
use tokio::fs;

// Embed the UI files at compile time
static UI_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../ui/dist");

/// Serve the admin UI index page (embedded version)
///
/// GET /admin
pub async fn serve_admin_ui() -> impl IntoResponse {
    debug!("Serving embedded admin UI index page");

    match UI_DIR.get_file("index.html") {
        Some(file) => match file.contents_utf8() {
            Some(content) => {
                debug!("Successfully served embedded admin UI index page");
                Html(content.to_string()).into_response()
            }
            None => {
                warn!("Failed to read embedded index.html as UTF-8");
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

/// Serve static files for the admin UI (embedded version)
///
/// GET /admin/*path
pub async fn serve_admin_static(Path(path): Path<String>) -> impl IntoResponse {
    // Remove leading slash if present
    let clean_path = path.strip_prefix('/').unwrap_or(&path);
    debug!("Serving embedded admin static file: {}", clean_path);

    match UI_DIR.get_file(clean_path) {
        Some(file) => {
            let mime_type = from_path(file.path()).first_or_octet_stream();
            debug!(
                "Serving embedded static file: {} (type: {})",
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
            debug!("Embedded static file not found: {}, serving index.html for SPA routing", clean_path);
            
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

/// Serve the admin UI index page from external filesystem
///
/// GET /admin (external mode)
pub async fn serve_external_admin_ui(admin_path: PathBuf) -> impl IntoResponse {
    let index_path = admin_path.join("index.html");
    debug!("Serving external admin UI index page from: {:?}", index_path);

    match fs::read_to_string(&index_path).await {
        Ok(content) => {
            debug!("Successfully served external admin UI index page");
            Html(content).into_response()
        }
        Err(e) => {
            error!("Failed to read external index.html from {:?}: {}", index_path, e);
            (
                StatusCode::NOT_FOUND,
                "External admin UI index.html not found",
            )
                .into_response()
        }
    }
}

/// Serve static files for the admin UI from external filesystem
///
/// GET /admin/*path (external mode)
pub async fn serve_external_admin_static(admin_path: PathBuf, Path(path): Path<String>) -> impl IntoResponse {
    // Remove leading slash if present
    let clean_path = path.strip_prefix('/').unwrap_or(&path);
    let file_path = admin_path.join(clean_path);
    debug!("Serving external admin static file: {:?}", file_path);

    // Security check: ensure the requested file is within the admin path
    if !file_path.starts_with(&admin_path) {
        warn!("Attempted directory traversal attack: {:?}", file_path);
        return (StatusCode::FORBIDDEN, "Access denied").into_response();
    }

    match fs::read(&file_path).await {
        Ok(content) => {
            let mime_type = from_path(&file_path).first_or_octet_stream();
            debug!(
                "Serving external static file: {:?} (type: {})",
                file_path,
                mime_type.as_ref()
            );

            match Response::builder()
                .header(header::CONTENT_TYPE, mime_type.as_ref())
                .header(header::CACHE_CONTROL, "public, max-age=3600") // Cache for 1 hour in dev mode
                .body(Body::from(content))
            {
                Ok(response) => response.into_response(),
                Err(e) => {
                    warn!("Failed to build response for {:?}: {}", file_path, e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to build response",
                    )
                        .into_response()
                }
            }
        }
        Err(_) => {
            debug!("External static file not found: {:?}, serving index.html for SPA routing", file_path);
            
            // For SPA routing, serve index.html for any unmatched routes
            let index_path = admin_path.join("index.html");
            match fs::read_to_string(&index_path).await {
                Ok(content) => {
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
                Err(e) => {
                    error!("Failed to read external index.html for SPA fallback from {:?}: {}", index_path, e);
                    (StatusCode::NOT_FOUND, "External admin UI not found").into_response()
                }
            }
        }
    }
}

/// Check if admin UI is available (embedded version)
pub fn is_admin_ui_available() -> bool {
    UI_DIR.get_file("index.html").is_some()
}

/// Check if external admin UI is available
pub async fn is_external_admin_ui_available(admin_path: &PathBuf) -> bool {
    let index_path = admin_path.join("index.html");
    tokio::fs::metadata(&index_path).await.is_ok()
}

/// Get admin UI build information (embedded version)
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
        mode: AdminUiMode::Embedded,
        build_time,
        file_count: Some(UI_DIR.files().count()),
        path: None,
    })
}

/// Get external admin UI build information
pub async fn get_external_admin_ui_info(admin_path: &PathBuf) -> Option<AdminUiInfo> {
    if !is_external_admin_ui_available(admin_path).await {
        return None;
    }

    // Try to read build info from a manifest file if it exists
    let build_info_path = admin_path.join("build-info.json");
    let build_time = if let Ok(content) = tokio::fs::read_to_string(&build_info_path).await {
        serde_json::from_str::<serde_json::Value>(&content)
            .ok()
            .and_then(|v| v.get("build_time").and_then(|t| t.as_str().map(String::from)))
    } else {
        None
    };

    // Count files in the directory
    let file_count = count_files_in_directory(admin_path).await;

    Some(AdminUiInfo {
        available: true,
        mode: AdminUiMode::External,
        build_time,
        file_count,
        path: Some(admin_path.clone()),
    })
}

/// Count files in a directory recursively
async fn count_files_in_directory(path: &PathBuf) -> Option<usize> {
    let mut count = 0;
    let mut stack = vec![path.clone()];

    while let Some(current_path) = stack.pop() {
        if let Ok(mut entries) = tokio::fs::read_dir(&current_path).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let entry_path = entry.path();
                if entry_path.is_file() {
                    count += 1;
                } else if entry_path.is_dir() {
                    stack.push(entry_path);
                }
            }
        }
    }

    Some(count)
}

/// Admin UI mode enum for info reporting
#[derive(Debug, serde::Serialize)]
pub enum AdminUiMode {
    Embedded,
    External,
}

/// Admin UI information
#[derive(Debug, serde::Serialize)]
pub struct AdminUiInfo {
    /// Whether the admin UI is available
    pub available: bool,
    /// Admin UI mode
    pub mode: AdminUiMode,
    /// Build timestamp if available
    pub build_time: Option<String>,
    /// Number of files (if countable)
    pub file_count: Option<usize>,
    /// Filesystem path (for external mode)
    pub path: Option<PathBuf>,
}