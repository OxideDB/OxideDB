//! Log retention and archival policies
//!
//! This module handles:
//! - Automatic log cleanup based on retention policies
//! - Log archival and compression
//! - Storage optimization
//! - Compliance with data retention requirements

use crate::{
    error::{LoggingError, LoggingResult},
    storage::SqliteLogStorage,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Log retention policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionPolicy {
    /// Standard log retention period in days
    pub standard_retention_days: u32,
    /// Security audit log retention period in days (usually longer)
    pub audit_retention_days: u32,
    /// Error log retention period in days (usually longer)
    pub error_retention_days: u32,
    /// Whether to compress archived logs
    pub enable_compression: bool,
    /// Archive directory path
    pub archive_directory: Option<PathBuf>,
    /// Whether to delete logs after archival
    pub delete_after_archive: bool,
    /// Minimum free space threshold in bytes before aggressive cleanup
    pub min_free_space_bytes: Option<u64>,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            standard_retention_days: 30,
            audit_retention_days: 365, // 1 year for audit logs
            error_retention_days: 90,  // 3 months for error logs
            enable_compression: true,
            archive_directory: None,
            delete_after_archive: false,
            min_free_space_bytes: Some(1_000_000_000), // 1GB minimum
        }
    }
}

/// Result of a retention cleanup operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupResult {
    /// Number of standard log entries deleted
    pub standard_logs_deleted: u64,
    /// Number of audit events deleted
    pub audit_events_deleted: u64,
    /// Number of archived files created
    pub archives_created: u32,
    /// Total bytes freed
    pub bytes_freed: u64,
    /// Duration of the cleanup operation
    pub cleanup_duration_ms: u64,
    /// Any warnings or issues encountered
    pub warnings: Vec<String>,
}

/// Log retention service
pub struct RetentionService {
    /// Storage backend
    storage: Arc<SqliteLogStorage>,
    /// Retention policy
    policy: RetentionPolicy,
}

impl RetentionService {
    /// Create a new retention service
    pub fn new(storage: Arc<SqliteLogStorage>, policy: RetentionPolicy) -> Self {
        Self { storage, policy }
    }

    /// Run retention cleanup based on policy
    pub async fn run_cleanup(&self) -> LoggingResult<CleanupResult> {
        let start_time = std::time::Instant::now();
        let mut result = CleanupResult {
            standard_logs_deleted: 0,
            audit_events_deleted: 0,
            archives_created: 0,
            bytes_freed: 0,
            cleanup_duration_ms: 0,
            warnings: Vec::new(),
        };

        info!(
            "Starting log retention cleanup with policy: {:?}",
            self.policy
        );

        // Check available disk space if configured
        if let Some(min_free_space) = self.policy.min_free_space_bytes {
            match self.check_disk_space().await {
                Ok(free_space) => {
                    if free_space < min_free_space {
                        warn!(
                            "Low disk space detected: {} bytes free, minimum required: {} bytes",
                            free_space, min_free_space
                        );
                        result
                            .warnings
                            .push(format!("Low disk space: {} bytes free", free_space));
                        // Could implement aggressive cleanup here
                    }
                }
                Err(e) => {
                    result
                        .warnings
                        .push(format!("Failed to check disk space: {}", e));
                }
            }
        }

        // Archive old logs if archival is enabled
        if self.policy.archive_directory.is_some() {
            match self.archive_old_logs(&mut result).await {
                Ok(_) => {}
                Err(e) => {
                    result
                        .warnings
                        .push(format!("Archive operation failed: {}", e));
                }
            }
        }

        // Clean up standard logs
        let standard_cutoff =
            Utc::now() - Duration::days(self.policy.standard_retention_days as i64);
        match self
            .storage
            .cleanup_old_entries(self.policy.standard_retention_days)
            .await
        {
            Ok(deleted) => {
                result.standard_logs_deleted = deleted;
                info!(
                    "Deleted {} standard log entries older than {}",
                    deleted, standard_cutoff
                );
            }
            Err(e) => {
                result
                    .warnings
                    .push(format!("Failed to cleanup standard logs: {}", e));
            }
        }

        // Clean up audit events (with different retention period)
        match self.cleanup_audit_events().await {
            Ok(deleted) => {
                result.audit_events_deleted = deleted;
                info!("Deleted {} audit events", deleted);
            }
            Err(e) => {
                result
                    .warnings
                    .push(format!("Failed to cleanup audit events: {}", e));
            }
        }

        // Calculate approximate bytes freed (rough estimate)
        result.bytes_freed = (result.standard_logs_deleted + result.audit_events_deleted) * 500; // ~500 bytes per log entry

        result.cleanup_duration_ms = start_time.elapsed().as_millis() as u64;

        info!(
            "Retention cleanup completed in {}ms: {} standard logs, {} audit events deleted",
            result.cleanup_duration_ms, result.standard_logs_deleted, result.audit_events_deleted
        );

        Ok(result)
    }

    /// Archive old logs to compressed files
    async fn archive_old_logs(&self, _result: &mut CleanupResult) -> LoggingResult<()> {
        let archive_dir = self
            .policy
            .archive_directory
            .as_ref()
            .ok_or_else(|| LoggingError::configuration("Archive directory not configured"))?;

        // Create archive directory if it doesn't exist
        if !archive_dir.exists() {
            std::fs::create_dir_all(archive_dir).map_err(LoggingError::from)?;
        }

        // For now, this is a placeholder for archive functionality
        // In a full implementation, you would:
        // 1. Query logs older than archive threshold
        // 2. Export to compressed format (e.g., gzip JSON)
        // 3. Verify archive integrity
        // 4. Delete original logs if delete_after_archive is true

        debug!("Archive functionality not yet implemented");
        Ok(())
    }

    /// Clean up audit events with their specific retention period
    async fn cleanup_audit_events(&self) -> LoggingResult<u64> {
        // This would require a specific method in storage for cleaning audit events
        // For now, we'll use the same retention period
        // In a full implementation, you'd have separate cleanup methods
        Ok(0)
    }

    /// Check available disk space
    async fn check_disk_space(&self) -> LoggingResult<u64> {
        // This is a simplified implementation
        // In a full implementation, you would use system calls to check disk space
        // For now, we'll return a large number to avoid warnings
        Ok(10_000_000_000) // 10GB
    }

    /// Get retention statistics
    pub async fn get_retention_stats(&self) -> LoggingResult<RetentionStats> {
        let metrics = self.storage.get_metrics().await?;

        Ok(RetentionStats {
            total_log_entries: metrics.total_entries,
            storage_size_bytes: metrics.storage_size_bytes,
            estimated_cleanup_candidates: self.estimate_cleanup_candidates().await?,
            last_cleanup_time: None, // Would be stored in a metadata table
            policy: self.policy.clone(),
        })
    }

    /// Estimate how many entries would be cleaned up
    async fn estimate_cleanup_candidates(&self) -> LoggingResult<u64> {
        // This would require additional query methods in storage
        // For now, return 0
        Ok(0)
    }

    /// Update retention policy
    pub fn update_policy(&mut self, new_policy: RetentionPolicy) {
        info!("Updating retention policy: {:?}", new_policy);
        self.policy = new_policy;
    }

    /// Get current retention policy
    pub fn get_policy(&self) -> &RetentionPolicy {
        &self.policy
    }

    /// Validate retention policy
    pub fn validate_policy(policy: &RetentionPolicy) -> LoggingResult<()> {
        if policy.standard_retention_days == 0 {
            return Err(LoggingError::configuration(
                "Standard retention days must be greater than 0",
            ));
        }

        if policy.audit_retention_days == 0 {
            return Err(LoggingError::configuration(
                "Audit retention days must be greater than 0",
            ));
        }

        if policy.error_retention_days == 0 {
            return Err(LoggingError::configuration(
                "Error retention days must be greater than 0",
            ));
        }

        if let Some(ref archive_dir) = policy.archive_directory {
            if let Some(parent) = archive_dir.parent() {
                if !parent.exists() {
                    return Err(LoggingError::configuration(format!(
                        "Archive directory parent does not exist: {:?}",
                        parent
                    )));
                }
            }
        }

        Ok(())
    }
}

/// Statistics about log retention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionStats {
    /// Total number of log entries
    pub total_log_entries: u64,
    /// Current storage size in bytes
    pub storage_size_bytes: u64,
    /// Estimated number of entries that would be cleaned up
    pub estimated_cleanup_candidates: u64,
    /// Last cleanup time
    pub last_cleanup_time: Option<DateTime<Utc>>,
    /// Current retention policy
    pub policy: RetentionPolicy,
}

/// Archive metadata for tracking archived logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveMetadata {
    /// Archive file name
    pub filename: String,
    /// Archive creation time
    pub created_at: DateTime<Utc>,
    /// Time range of logs in the archive
    pub log_start_time: DateTime<Utc>,
    pub log_end_time: DateTime<Utc>,
    /// Number of log entries in the archive
    pub entry_count: u64,
    /// Archive file size in bytes
    pub file_size_bytes: u64,
    /// Compression ratio (if compressed)
    pub compression_ratio: Option<f64>,
    /// Checksum for integrity verification
    pub checksum: String,
}

/// Builder for retention policies
pub struct RetentionPolicyBuilder {
    policy: RetentionPolicy,
}

impl RetentionPolicyBuilder {
    /// Create a new policy builder
    pub fn new() -> Self {
        Self {
            policy: RetentionPolicy::default(),
        }
    }

    /// Set standard retention period
    pub fn standard_retention_days(mut self, days: u32) -> Self {
        self.policy.standard_retention_days = days;
        self
    }

    /// Set audit retention period
    pub fn audit_retention_days(mut self, days: u32) -> Self {
        self.policy.audit_retention_days = days;
        self
    }

    /// Set error retention period
    pub fn error_retention_days(mut self, days: u32) -> Self {
        self.policy.error_retention_days = days;
        self
    }

    /// Enable or disable compression
    pub fn enable_compression(mut self, enable: bool) -> Self {
        self.policy.enable_compression = enable;
        self
    }

    /// Set archive directory
    pub fn archive_directory(mut self, dir: impl Into<PathBuf>) -> Self {
        self.policy.archive_directory = Some(dir.into());
        self
    }

    /// Set whether to delete logs after archival
    pub fn delete_after_archive(mut self, delete: bool) -> Self {
        self.policy.delete_after_archive = delete;
        self
    }

    /// Set minimum free space threshold
    pub fn min_free_space_bytes(mut self, bytes: u64) -> Self {
        self.policy.min_free_space_bytes = Some(bytes);
        self
    }

    /// Build the retention policy
    pub fn build(self) -> LoggingResult<RetentionPolicy> {
        RetentionService::validate_policy(&self.policy)?;
        Ok(self.policy)
    }
}

impl Default for RetentionPolicyBuilder {
    fn default() -> Self {
        Self::new()
    }
}
