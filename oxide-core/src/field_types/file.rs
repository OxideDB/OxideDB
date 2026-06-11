//! File Field Type
//!
//! This field type stores references to files in the virtual file system,
//! allowing collections to have file attachments without direct filesystem access.

use super::FieldTypeDefinition;
use crate::vfs::{FileId, VfsPath};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use ts_rs::TS;

/// Configuration for File field type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[ts(export)]
pub struct FileFieldConfig {
    /// Whether multiple files can be attached
    pub multiple: bool,
    /// Allowed MIME types (None = all allowed)
    pub allowed_mime_types: Option<Vec<String>>,
    /// Maximum file size in bytes
    pub max_file_size: Option<u64>,
    /// Whether file is required
    pub required: bool,
}

impl Default for FileFieldConfig {
    fn default() -> Self {
        Self {
            multiple: false,
            allowed_mime_types: None,
            max_file_size: Some(10 * 1024 * 1024), // 10MB default
            required: false,
        }
    }
}

/// File reference stored in the database
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FileReference {
    /// File ID in the VFS
    pub file_id: FileId,
    /// Original file name
    pub name: String,
    /// MIME type
    pub mime_type: String,
    /// File size in bytes
    pub size: u64,
    /// Virtual path in the VFS
    pub path: VfsPath,
}

/// File field type implementation
#[derive(Debug, Clone, Default)]
pub struct FileFieldType {
    config: FileFieldConfig,
}

impl FileFieldType {
    /// Create a new file field type with configuration
    pub fn new(config: FileFieldConfig) -> Self {
        Self { config }
    }

    /// Get the field configuration
    pub fn config(&self) -> &FileFieldConfig {
        &self.config
    }

    /// Validate a single file reference
    fn validate_file_reference(&self, file_ref: &FileReference) -> Result<(), String> {
        // Validate MIME type if restrictions are set
        if let Some(allowed_types) = &self.config.allowed_mime_types {
            if !allowed_types
                .iter()
                .any(|allowed| file_ref.mime_type.starts_with(allowed) || allowed == "*")
            {
                return Err(format!(
                    "MIME type '{}' is not allowed. Allowed types: {:?}",
                    file_ref.mime_type, allowed_types
                ));
            }
        }

        // Validate file size
        if let Some(max_size) = self.config.max_file_size {
            if file_ref.size > max_size {
                return Err(format!(
                    "File size {} bytes exceeds maximum allowed size {} bytes",
                    file_ref.size, max_size
                ));
            }
        }

        // Validate file ID is not empty
        if file_ref.file_id.is_empty() {
            return Err("File ID cannot be empty".to_string());
        }

        // Validate file name is not empty
        if file_ref.name.is_empty() {
            return Err("File name cannot be empty".to_string());
        }

        Ok(())
    }
}

impl FieldTypeDefinition for FileFieldType {
    fn type_name(&self) -> &'static str {
        "file"
    }

    fn validate(&self, field_name: &str, value: &JsonValue) -> Result<(), String> {
        match value {
            JsonValue::Null => {
                if self.config.required {
                    return Err(format!("Field '{}' is required", field_name));
                }
                Ok(())
            }
            JsonValue::Object(_) => {
                // Single file reference
                if self.config.multiple {
                    return Err(format!(
                        "Field '{}' expects multiple files but received a single file reference",
                        field_name
                    ));
                }

                let file_ref: FileReference =
                    serde_json::from_value(value.clone()).map_err(|e| {
                        format!(
                            "Field '{}' contains invalid file reference: {}",
                            field_name, e
                        )
                    })?;

                self.validate_file_reference(&file_ref)
                    .map_err(|e| format!("Field '{}': {}", field_name, e))
            }
            JsonValue::Array(files) => {
                // Multiple file references
                if !self.config.multiple {
                    return Err(format!(
                        "Field '{}' expects a single file but received multiple files",
                        field_name
                    ));
                }

                if files.is_empty() && self.config.required {
                    return Err(format!("Field '{}' is required", field_name));
                }

                for (index, file_value) in files.iter().enumerate() {
                    let file_ref: FileReference = serde_json::from_value(file_value.clone())
                        .map_err(|e| {
                            format!(
                                "Field '{}' contains invalid file reference at index {}: {}",
                                field_name, index, e
                            )
                        })?;

                    self.validate_file_reference(&file_ref)
                        .map_err(|e| format!("Field '{}' at index {}: {}", field_name, index, e))?;
                }

                Ok(())
            }
            _ => Err(format!(
                "Field '{}' must be a file reference object or array of file references",
                field_name
            )),
        }
    }

    fn convert_value(&self, value: &JsonValue) -> Result<JsonValue, String> {
        // No automatic conversion for file references - they must be explicitly created
        Ok(value.clone())
    }

    fn sql_type(&self) -> &'static str {
        "TEXT" // Store as JSON
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_file_ref() -> FileReference {
        FileReference {
            file_id: "file123".to_string(),
            name: "test.jpg".to_string(),
            mime_type: "image/jpeg".to_string(),
            size: 1024,
            path: "/uploads/test.jpg".to_string(),
        }
    }

    #[test]
    fn test_single_file_validation() {
        let field_type = FileFieldType::default();
        let file_ref = create_test_file_ref();
        let value = serde_json::to_value(&file_ref).unwrap();

        assert!(field_type.validate("test_field", &value).is_ok());
    }

    #[test]
    fn test_multiple_files_validation() {
        let config = FileFieldConfig {
            multiple: true,
            ..Default::default()
        };
        let field_type = FileFieldType::new(config);

        let file_ref = create_test_file_ref();
        let value = json!([file_ref, file_ref]);

        assert!(field_type.validate("test_field", &value).is_ok());
    }

    #[test]
    fn test_mime_type_validation() {
        let config = FileFieldConfig {
            allowed_mime_types: Some(vec!["image/".to_string()]),
            ..Default::default()
        };
        let field_type = FileFieldType::new(config);

        let file_ref = create_test_file_ref();
        let value = serde_json::to_value(&file_ref).unwrap();

        assert!(field_type.validate("test_field", &value).is_ok());

        // Test invalid MIME type
        let mut invalid_file_ref = create_test_file_ref();
        invalid_file_ref.mime_type = "application/pdf".to_string();
        let invalid_value = serde_json::to_value(&invalid_file_ref).unwrap();

        assert!(field_type.validate("test_field", &invalid_value).is_err());
    }

    #[test]
    fn test_file_size_validation() {
        let config = FileFieldConfig {
            max_file_size: Some(512), // Very small limit
            ..Default::default()
        };
        let field_type = FileFieldType::new(config);

        let file_ref = create_test_file_ref(); // Size is 1024, exceeds limit
        let value = serde_json::to_value(&file_ref).unwrap();

        assert!(field_type.validate("test_field", &value).is_err());
    }

    #[test]
    fn test_required_field_validation() {
        let config = FileFieldConfig {
            required: true,
            ..Default::default()
        };
        let field_type = FileFieldType::new(config);

        // Test null value with required field
        assert!(field_type.validate("test_field", &JsonValue::Null).is_err());

        // Test empty array with required multiple files
        let config = FileFieldConfig {
            multiple: true,
            required: true,
            ..Default::default()
        };
        let field_type = FileFieldType::new(config);

        assert!(field_type.validate("test_field", &json!([])).is_err());
    }

    #[test]
    fn test_invalid_format_validation() {
        let field_type = FileFieldType::default();

        // Test invalid JSON structure
        assert!(field_type
            .validate("test_field", &json!("invalid"))
            .is_err());
        assert!(field_type.validate("test_field", &json!(123)).is_err());
    }
}
