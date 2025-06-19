//! Event System Metrics and Observability
//!
//! This module provides comprehensive metrics collection and reporting
//! for the event system, enabling monitoring and debugging in production.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::{BeforeEventType, AfterEventType};

/// Comprehensive metrics for the event system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventMetrics {
    /// Overall system metrics
    pub system: SystemMetrics,
    /// Per-event-type metrics for Before events
    pub before_events: HashMap<String, EventTypeMetrics>,
    /// Per-event-type metrics for After events
    pub after_events: HashMap<String, EventTypeMetrics>,
    /// Per-handler metrics
    pub handlers: HashMap<String, HandlerMetrics>,
    /// Performance metrics
    pub performance: PerformanceMetrics,
    /// Error metrics
    pub errors: ErrorMetrics,
    /// Timestamp when metrics were last updated
    pub last_updated: u64,
}

impl EventMetrics {
    /// Create new empty metrics
    pub fn new() -> Self {
        Self {
            system: SystemMetrics::default(),
            before_events: HashMap::new(),
            after_events: HashMap::new(),
            handlers: HashMap::new(),
            performance: PerformanceMetrics::default(),
            errors: ErrorMetrics::default(),
            last_updated: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    /// Get total number of events processed
    pub fn total_events(&self) -> u64 {
        self.before_events.values().map(|m| m.total_dispatched).sum::<u64>()
            + self.after_events.values().map(|m| m.total_dispatched).sum::<u64>()
    }

    /// Get total number of failed events
    pub fn total_failures(&self) -> u64 {
        self.before_events.values().map(|m| m.total_failures).sum::<u64>()
            + self.after_events.values().map(|m| m.total_failures).sum::<u64>()
    }

    /// Get overall success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        let total = self.total_events();
        if total == 0 {
            return 100.0;
        }
        let failures = self.total_failures();
        ((total - failures) as f64 / total as f64) * 100.0
    }

    /// Get average processing time across all events
    pub fn avg_processing_time_ms(&self) -> f64 {
        let mut total_time = 0.0;
        let mut total_count = 0u64;

        for metrics in self.before_events.values() {
            total_time += metrics.avg_execution_time_ms * metrics.total_dispatched as f64;
            total_count += metrics.total_dispatched;
        }

        for metrics in self.after_events.values() {
            total_time += metrics.avg_execution_time_ms * metrics.total_dispatched as f64;
            total_count += metrics.total_dispatched;
        }

        if total_count == 0 {
            0.0
        } else {
            total_time / total_count as f64
        }
    }
}

impl Default for EventMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// System-wide metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemMetrics {
    /// When the event system was started
    pub startup_time: u64,
    /// Total uptime in milliseconds
    pub uptime_ms: u64,
    /// Total number of handlers registered
    pub total_handlers_registered: u64,
    /// Total number of handlers unregistered
    pub total_handlers_unregistered: u64,
    /// Current number of active handlers
    pub active_handlers: u64,
    /// Peak number of concurrent events processed
    pub peak_concurrent_events: u64,
    /// Current memory usage in bytes (if available)
    pub memory_usage_bytes: Option<u64>,
}

/// Metrics for a specific event type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventTypeMetrics {
    /// Total number of times this event was dispatched
    pub total_dispatched: u64,
    /// Total number of failures for this event type
    pub total_failures: u64,
    /// Total number of times handlers were skipped
    pub total_skipped: u64,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: f64,
    /// Minimum execution time in milliseconds
    pub min_execution_time_ms: f64,
    /// Maximum execution time in milliseconds
    pub max_execution_time_ms: f64,
    /// Number of currently registered handlers
    pub handler_count: u64,
    /// Last dispatch timestamp
    pub last_dispatch_time: u64,
    /// Recent dispatch times for trend analysis (last 100)
    pub recent_execution_times: Vec<f64>,
}

impl EventTypeMetrics {
    /// Update metrics with a new execution time
    pub fn record_execution(&mut self, execution_time_ms: f64, success: bool, skipped: bool) {
        self.total_dispatched += 1;
        self.last_dispatch_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if skipped {
            self.total_skipped += 1;
            return;
        }

        if !success {
            self.total_failures += 1;
        }

        // Update execution time statistics
        if self.total_dispatched == 1 {
            self.avg_execution_time_ms = execution_time_ms;
            self.min_execution_time_ms = execution_time_ms;
            self.max_execution_time_ms = execution_time_ms;
        } else {
            // Update running average
            let total_time = self.avg_execution_time_ms * (self.total_dispatched - 1) as f64;
            self.avg_execution_time_ms = (total_time + execution_time_ms) / self.total_dispatched as f64;
            
            // Update min/max
            self.min_execution_time_ms = self.min_execution_time_ms.min(execution_time_ms);
            self.max_execution_time_ms = self.max_execution_time_ms.max(execution_time_ms);
        }

        // Store recent execution times (keep last 100)
        self.recent_execution_times.push(execution_time_ms);
        if self.recent_execution_times.len() > 100 {
            self.recent_execution_times.remove(0);
        }
    }

    /// Get the trend of recent execution times
    pub fn execution_time_trend(&self) -> ExecutionTimeTrend {
        if self.recent_execution_times.len() < 10 {
            return ExecutionTimeTrend::Stable;
        }

        let recent_count = self.recent_execution_times.len().min(20);
        let older_count = self.recent_execution_times.len().min(40);

        let recent_avg: f64 = self.recent_execution_times
            .iter()
            .rev()
            .take(recent_count)
            .sum::<f64>() / recent_count as f64;

        let older_avg: f64 = self.recent_execution_times
            .iter()
            .rev()
            .skip(recent_count)
            .take(older_count - recent_count)
            .sum::<f64>() / (older_count - recent_count) as f64;

        let change_percent = ((recent_avg - older_avg) / older_avg) * 100.0;

        if change_percent > 20.0 {
            ExecutionTimeTrend::Increasing
        } else if change_percent < -20.0 {
            ExecutionTimeTrend::Decreasing
        } else {
            ExecutionTimeTrend::Stable
        }
    }
}

/// Trend analysis for execution times
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionTimeTrend {
    Increasing,
    Decreasing,
    Stable,
}

/// Metrics for individual handlers
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HandlerMetrics {
    /// Handler ID
    pub handler_id: String,
    /// Handler name
    pub handler_name: String,
    /// Total number of executions
    pub total_executions: u64,
    /// Total number of failures
    pub total_failures: u64,
    /// Total number of times skipped
    pub total_skipped: u64,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: f64,
    /// Total time spent in this handler
    pub total_execution_time_ms: f64,
    /// Number of retries attempted
    pub total_retries: u64,
    /// Last execution timestamp
    pub last_execution_time: u64,
    /// Whether the handler is currently enabled
    pub enabled: bool,
}

impl HandlerMetrics {
    /// Create new handler metrics
    pub fn new(handler_id: String, handler_name: String) -> Self {
        Self {
            handler_id,
            handler_name,
            enabled: true,
            ..Default::default()
        }
    }

    /// Record a handler execution
    pub fn record_execution(&mut self, execution_time_ms: f64, success: bool, skipped: bool, retries: u32) {
        self.total_executions += 1;
        self.last_execution_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if skipped {
            self.total_skipped += 1;
            return;
        }

        if !success {
            self.total_failures += 1;
        }

        self.total_retries += retries as u64;
        self.total_execution_time_ms += execution_time_ms;
        
        if self.total_executions > 0 {
            self.avg_execution_time_ms = self.total_execution_time_ms / self.total_executions as f64;
        }
    }

    /// Get success rate as a percentage
    pub fn success_rate(&self) -> f64 {
        if self.total_executions == 0 {
            return 100.0;
        }
        let successes = self.total_executions - self.total_failures;
        (successes as f64 / self.total_executions as f64) * 100.0
    }
}

/// Performance-related metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Current number of events being processed
    pub current_concurrent_events: u64,
    /// Peak number of concurrent events
    pub peak_concurrent_events: u64,
    /// Average queue depth
    pub avg_queue_depth: f64,
    /// Peak queue depth
    pub peak_queue_depth: u64,
    /// Events processed per second (last minute)
    pub events_per_second: f64,
    /// Average time from event creation to completion
    pub avg_end_to_end_latency_ms: f64,
    /// 95th percentile latency
    pub p95_latency_ms: f64,
    /// 99th percentile latency
    pub p99_latency_ms: f64,
}

/// Error-related metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ErrorMetrics {
    /// Total number of errors
    pub total_errors: u64,
    /// Errors by type
    pub errors_by_type: HashMap<String, u64>,
    /// Recent error messages (last 10)
    pub recent_errors: Vec<ErrorRecord>,
    /// Error rate (errors per minute)
    pub error_rate: f64,
}

/// Record of a specific error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    /// When the error occurred
    pub timestamp: u64,
    /// Error type/category
    pub error_type: String,
    /// Error message
    pub message: String,
    /// Handler ID that caused the error (if applicable)
    pub handler_id: Option<String>,
    /// Event type being processed when error occurred
    pub event_type: Option<String>,
}

/// Collector for event metrics with thread-safe operations
pub struct EventMetricsCollector {
    metrics: Arc<RwLock<EventMetrics>>,
    start_time: SystemTime,
}

impl EventMetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(EventMetrics::new())),
            start_time: SystemTime::now(),
        }
    }

    /// Record a Before event dispatch
    pub fn record_before_event(
        &self,
        event_type: &BeforeEventType,
        execution_time_ms: f64,
        success: bool,
        skipped: bool,
    ) {
        if let Ok(mut metrics) = self.metrics.write() {
            let event_metrics = metrics
                .before_events
                .entry(event_type.name().to_string())
                .or_insert_with(EventTypeMetrics::default);
            
            event_metrics.record_execution(execution_time_ms, success, skipped);
            metrics.last_updated = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
        }
    }

    /// Record an After event dispatch
    pub fn record_after_event(
        &self,
        event_type: &AfterEventType,
        execution_time_ms: f64,
        success: bool,
        skipped: bool,
    ) {
        if let Ok(mut metrics) = self.metrics.write() {
            let event_metrics = metrics
                .after_events
                .entry(event_type.name().to_string())
                .or_insert_with(EventTypeMetrics::default);
            
            event_metrics.record_execution(execution_time_ms, success, skipped);
            metrics.last_updated = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
        }
    }

    /// Record handler execution
    pub fn record_handler_execution(
        &self,
        handler_id: &str,
        handler_name: &str,
        execution_time_ms: f64,
        success: bool,
        skipped: bool,
        retries: u32,
    ) {
        if let Ok(mut metrics) = self.metrics.write() {
            let handler_metrics = metrics
                .handlers
                .entry(handler_id.to_string())
                .or_insert_with(|| HandlerMetrics::new(handler_id.to_string(), handler_name.to_string()));
            
            handler_metrics.record_execution(execution_time_ms, success, skipped, retries);
        }
    }

    /// Record an error
    pub fn record_error(
        &self,
        error_type: String,
        message: String,
        handler_id: Option<String>,
        event_type: Option<String>,
    ) {
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.errors.total_errors += 1;
            
            *metrics.errors.errors_by_type.entry(error_type.clone()).or_insert(0) += 1;
            
            let error_record = ErrorRecord {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                error_type,
                message,
                handler_id,
                event_type,
            };
            
            metrics.errors.recent_errors.push(error_record);
            if metrics.errors.recent_errors.len() > 10 {
                metrics.errors.recent_errors.remove(0);
            }
        }
    }

    /// Update system metrics
    pub fn update_system_metrics(&self, active_handlers: u64, concurrent_events: u64) {
        if let Ok(mut metrics) = self.metrics.write() {
            metrics.system.active_handlers = active_handlers;
            metrics.system.uptime_ms = self.start_time
                .elapsed()
                .unwrap_or_default()
                .as_millis() as u64;
            
            if concurrent_events > metrics.system.peak_concurrent_events {
                metrics.system.peak_concurrent_events = concurrent_events;
            }
        }
    }

    /// Get a snapshot of current metrics
    pub fn snapshot(&self) -> EventMetrics {
        match self.metrics.read() {
            Ok(metrics) => metrics.clone(),
            Err(_) => {
                // Fallback in case of poisoned lock
                EventMetrics::new()
            }
        }
    }

    /// Reset all metrics
    pub fn reset(&self) {
        if let Ok(mut metrics) = self.metrics.write() {
            *metrics = EventMetrics::new();
        }
    }
}

impl Default for EventMetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_metrics() {
        let mut metrics = EventTypeMetrics::default();
        
        // Record some executions
        metrics.record_execution(100.0, true, false);
        metrics.record_execution(200.0, true, false);
        metrics.record_execution(150.0, false, false);
        
        assert_eq!(metrics.total_dispatched, 3);
        assert_eq!(metrics.total_failures, 1);
        assert_eq!(metrics.avg_execution_time_ms, 150.0);
        assert_eq!(metrics.min_execution_time_ms, 100.0);
        assert_eq!(metrics.max_execution_time_ms, 200.0);
    }

    #[test]
    fn test_handler_metrics() {
        let mut metrics = HandlerMetrics::new("test-handler".to_string(), "Test Handler".to_string());
        
        metrics.record_execution(100.0, true, false, 0);
        metrics.record_execution(200.0, false, false, 2);
        
        assert_eq!(metrics.total_executions, 2);
        assert_eq!(metrics.total_failures, 1);
        assert_eq!(metrics.total_retries, 2);
        assert_eq!(metrics.success_rate(), 50.0);
        assert_eq!(metrics.avg_execution_time_ms, 150.0);
    }

    #[test]
    fn test_metrics_collector() {
        let collector = EventMetricsCollector::new();
        
        collector.record_before_event(&BeforeEventType::RecordCreate, 100.0, true, false);
        collector.record_handler_execution("handler-1", "Test Handler", 50.0, true, false, 0);
        collector.record_error(
            "validation_error".to_string(),
            "Invalid data".to_string(),
            Some("handler-1".to_string()),
            Some("BeforeRecordCreate".to_string()),
        );
        
        let snapshot = collector.snapshot();
        assert_eq!(snapshot.total_events(), 1);
        assert_eq!(snapshot.errors.total_errors, 1);
        assert!(snapshot.handlers.contains_key("handler-1"));
    }

    #[test]
    fn test_execution_time_trend() {
        let mut metrics = EventTypeMetrics::default();
        
        // Add some times showing an increasing trend
        for i in 0..50 {
            let time = 100.0 + (i as f64 * 2.0); // Gradually increasing
            metrics.record_execution(time, true, false);
        }
        
        assert_eq!(metrics.execution_time_trend(), ExecutionTimeTrend::Increasing);
    }

    #[test]
    fn test_event_metrics_aggregation() {
        let mut metrics = EventMetrics::new();
        
        // Add some event type metrics
        let mut before_metrics = EventTypeMetrics::default();
        before_metrics.record_execution(100.0, true, false);
        before_metrics.record_execution(200.0, false, false);
        metrics.before_events.insert("BeforeRecordCreate".to_string(), before_metrics);
        
        assert_eq!(metrics.total_events(), 2);
        assert_eq!(metrics.total_failures(), 1);
        assert_eq!(metrics.success_rate(), 50.0);
    }
} 