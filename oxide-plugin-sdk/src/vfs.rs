//! VFS utilities for plugin file operations.

use crate::{
    FileIdentifier, FileListRequest, FileListResponse, FileMetadata, FileMoveRequest,
    FileReadRequest, FileReadResponse, FileWriteRequest, Host, PluginError, PluginResult,
    VfsUsageStats,
};

/// Virtual file system utilities for plugins.
pub struct Vfs;

impl Vfs {
    /// Write a file with default metadata and no overwrite.
    pub fn write(
        namespace: &str,
        path: &str,
        content: impl Into<Vec<u8>>,
        mime_type: Option<&str>,
    ) -> PluginResult<FileMetadata> {
        Self::write_with_request(
            namespace,
            FileWriteRequest {
                path: path.to_string(),
                content: content.into(),
                mime_type: mime_type.map(str::to_string),
                custom_metadata: None,
                tags: None,
                overwrite: false,
            },
        )
    }

    /// Write or replace a file with default metadata.
    pub fn overwrite(
        namespace: &str,
        path: &str,
        content: impl Into<Vec<u8>>,
        mime_type: Option<&str>,
    ) -> PluginResult<FileMetadata> {
        Self::write_with_request(
            namespace,
            FileWriteRequest {
                path: path.to_string(),
                content: content.into(),
                mime_type: mime_type.map(str::to_string),
                custom_metadata: None,
                tags: None,
                overwrite: true,
            },
        )
    }

    /// Write a file using a full request.
    pub fn write_with_request(
        namespace: &str,
        request: FileWriteRequest,
    ) -> PluginResult<FileMetadata> {
        Host::vfs_write_file(namespace, &request)
    }

    /// Read file metadata and content.
    pub fn read(namespace: &str, identifier: FileIdentifier) -> PluginResult<FileReadResponse> {
        Self::read_with_request(
            namespace,
            FileReadRequest {
                identifier,
                include_content: true,
            },
        )
    }

    /// Read only file metadata.
    pub fn metadata(namespace: &str, identifier: FileIdentifier) -> PluginResult<FileMetadata> {
        Ok(Self::read_with_request(
            namespace,
            FileReadRequest {
                identifier,
                include_content: false,
            },
        )?
        .metadata)
    }

    /// Read file content bytes.
    pub fn read_bytes(namespace: &str, identifier: FileIdentifier) -> PluginResult<Vec<u8>> {
        Self::read(namespace, identifier)?
            .content
            .ok_or_else(|| PluginError::InvalidData("VFS response did not include content".into()))
    }

    /// Read a file using a full request.
    pub fn read_with_request(
        namespace: &str,
        request: FileReadRequest,
    ) -> PluginResult<FileReadResponse> {
        Host::vfs_read_file(namespace, &request)
    }

    /// Move or rename a file.
    pub fn move_file(
        namespace: &str,
        identifier: FileIdentifier,
        new_path: &str,
        overwrite: bool,
    ) -> PluginResult<FileMetadata> {
        Self::move_with_request(
            namespace,
            FileMoveRequest {
                identifier,
                new_path: new_path.to_string(),
                overwrite,
            },
        )
    }

    /// Move or rename a file using a full request.
    pub fn move_with_request(
        namespace: &str,
        request: FileMoveRequest,
    ) -> PluginResult<FileMetadata> {
        Host::vfs_move_file(namespace, &request)
    }

    /// Delete a file.
    pub fn delete(namespace: &str, identifier: FileIdentifier) -> PluginResult<()> {
        Host::vfs_delete_file(namespace, &identifier)
    }

    /// List files in a namespace root or directory.
    pub fn list(
        namespace: &str,
        directory: impl Into<String>,
        recursive: bool,
    ) -> PluginResult<FileListResponse> {
        Self::list_with_request(
            namespace,
            FileListRequest {
                directory: directory.into(),
                recursive,
                mime_filter: None,
                tag_filter: None,
                offset: None,
                limit: None,
            },
        )
    }

    /// List files using a full request.
    pub fn list_with_request(
        namespace: &str,
        request: FileListRequest,
    ) -> PluginResult<FileListResponse> {
        Host::vfs_list_files(namespace, &request)
    }

    /// Get namespace usage statistics.
    pub fn usage(namespace: &str) -> PluginResult<VfsUsageStats> {
        Host::vfs_get_usage_stats(namespace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn move_request_serializes_with_core_contract_shape() {
        let request = FileMoveRequest {
            identifier: FileIdentifier::Path("drafts/report.txt".to_string()),
            new_path: "published/report.txt".to_string(),
            overwrite: true,
        };

        let value = serde_json::to_value(request).unwrap();

        assert_eq!(
            value,
            json!({
                "identifier": { "Path": "drafts/report.txt" },
                "new_path": "published/report.txt",
                "overwrite": true
            })
        );
    }

    #[test]
    fn list_request_serializes_optional_filters() {
        let request = FileListRequest {
            directory: "uploads".to_string(),
            recursive: true,
            mime_filter: Some("image/".to_string()),
            tag_filter: Some(vec!["public".to_string()]),
            offset: Some(10),
            limit: Some(25),
        };

        let value = serde_json::to_value(request).unwrap();

        assert_eq!(value["directory"], "uploads");
        assert_eq!(value["recursive"], json!(true));
        assert_eq!(value["mime_filter"], "image/");
        assert_eq!(value["tag_filter"], json!(["public"]));
        assert_eq!(value["offset"], 10);
        assert_eq!(value["limit"], 25);
    }
}
