//! VFS Utility Functions
//!
//! Common utilities for file operations, hashing, compression, and validation.

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

/// Calculate SHA256 hash of file content
pub fn calculate_content_hash(content: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content);
    hex::encode(hasher.finalize())
}

/// Compress content using gzip
pub fn compress_content(content: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content)?;
    encoder.finish()
}

/// Decompress gzip content
pub fn decompress_content(compressed: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut decoder = GzDecoder::new(compressed);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

/// Validate file path for security (prevent directory traversal)
pub fn validate_path(path: &str) -> bool {
    if path.starts_with('/') {
        return false;
    }

    normalize_path(path).is_some()
}

/// Validate a VFS namespace identifier.
///
/// Namespaces map to top-level storage directories, so they must be a single
/// identifier segment rather than a virtual file path.
pub fn validate_namespace(namespace: &str) -> bool {
    if namespace.is_empty() || namespace.len() > 128 {
        return false;
    }

    let mut chars = namespace.chars();
    match chars.next() {
        Some(first) if first == '_' || first.is_ascii_alphanumeric() => {}
        _ => return false,
    }

    chars.all(|ch| ch == '_' || ch == '-' || ch == '.' || ch.is_ascii_alphanumeric())
}

/// Normalize a virtual VFS path while rejecting traversal and unsafe segments.
///
/// A leading slash is treated as a virtual-root marker, so `/uploads/a.png`
/// normalizes to `uploads/a.png`. Backslashes, control characters, `..`
/// segments, and characters that are invalid on common filesystems are rejected.
pub fn normalize_path(path: &str) -> Option<String> {
    if path.contains('\\') {
        return None;
    }

    let invalid_chars = ['<', '>', ':', '"', '|', '?', '*'];
    let mut segments = Vec::new();

    for segment in path.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }

        if segment == ".."
            || segment
                .chars()
                .any(|c| c.is_control() || invalid_chars.contains(&c))
        {
            return None;
        }

        segments.push(segment);
    }

    Some(segments.join("/"))
}

/// Generate a unique file ID
pub fn generate_file_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Detect MIME type from file extension
pub fn detect_mime_type(filename: &str) -> String {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "png" => "image/png".to_string(),
        "gif" => "image/gif".to_string(),
        "webp" => "image/webp".to_string(),
        "svg" => "image/svg+xml".to_string(),
        "pdf" => "application/pdf".to_string(),
        "txt" => "text/plain".to_string(),
        "html" | "htm" => "text/html".to_string(),
        "css" => "text/css".to_string(),
        "js" => "application/javascript".to_string(),
        "json" => "application/json".to_string(),
        "xml" => "application/xml".to_string(),
        "zip" => "application/zip".to_string(),
        "tar" => "application/x-tar".to_string(),
        "gz" => "application/gzip".to_string(),
        "mp4" => "video/mp4".to_string(),
        "mp3" => "audio/mpeg".to_string(),
        "wav" => "audio/wav".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash() {
        let content = b"hello world";
        let hash = calculate_content_hash(content);
        assert_eq!(hash.len(), 64); // SHA256 produces 64 character hex string
    }

    #[test]
    fn test_compression() {
        let content = b"hello world".repeat(100);
        let compressed = compress_content(&content).unwrap();
        assert!(compressed.len() < content.len());

        let decompressed = decompress_content(&compressed).unwrap();
        assert_eq!(content, decompressed);
    }

    #[test]
    fn test_path_validation() {
        assert!(validate_path("folder/file.txt"));
        assert!(validate_path("file.txt"));
        assert!(validate_path("folder/..file.txt"));
        assert!(!validate_path("../file.txt"));
        assert!(!validate_path("/etc/passwd"));
        assert!(!validate_path("folder\\file.txt"));
        assert!(!validate_path("file<.txt"));
    }

    #[test]
    fn test_namespace_validation() {
        assert!(validate_namespace("uploads"));
        assert!(validate_namespace("tenant-1.assets"));
        assert!(validate_namespace("_system"));
        assert!(!validate_namespace(""));
        assert!(!validate_namespace(".hidden"));
        assert!(!validate_namespace("tenant/uploads"));
        assert!(!validate_namespace("../uploads"));
        assert!(!validate_namespace("plugin:uploads"));
        assert!(!validate_namespace("uploads\\files"));
    }

    #[test]
    fn test_path_normalization() {
        assert_eq!(
            normalize_path("/uploads//images/./file.png"),
            Some("uploads/images/file.png".to_string())
        );
        assert_eq!(normalize_path("/"), Some(String::new()));
        assert_eq!(normalize_path(""), Some(String::new()));
        assert_eq!(normalize_path("uploads/../file.png"), None);
        assert_eq!(normalize_path("uploads\\file.png"), None);
    }

    #[test]
    fn test_mime_detection() {
        assert_eq!(detect_mime_type("image.jpg"), "image/jpeg");
        assert_eq!(detect_mime_type("document.pdf"), "application/pdf");
        assert_eq!(detect_mime_type("unknown.xyz"), "application/octet-stream");
    }
}
