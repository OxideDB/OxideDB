//! Bridge implementation that connects oxide-logging to oxide-core abstractions
//!
//! This module provides implementations of the oxide-core logging traits
//! using the concrete oxide-logging service implementations.

use crate::{
    service::LogService,
    models::{LogEntry, CorrelationId},
};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

// Import the oxide-core traits without importing the whole crate
// This allows us to implement the traits while keeping dependencies clean

/// Bridge implementation that adapts LogService to oxide-core's ApplicationLogger trait
pub struct LogServiceBridge {
    /// The underlying log service
    log_service: Arc<LogService>,
}

impl LogServiceBridge {
    /// Create a new bridge from a log service
    pub fn new(log_service: Arc<LogService>) -> Self {
        Self { log_service }
    }

    /// Get the underlying log service
    pub fn inner(&self) -> &Arc<LogService> {
        &self.log_service
    }

    /// Convert oxide-core LogLevel to oxide-logging LogLevel
    fn convert_log_level(level: oxide_core::LogLevel) -> crate::models::LogLevel {
        match level {
            oxide_core::LogLevel::Error => crate::models::LogLevel::Error,
            oxide_core::LogLevel::Warn => crate::models::LogLevel::Warn,
            oxide_core::LogLevel::Info => crate::models::LogLevel::Info,
            oxide_core::LogLevel::Debug => crate::models::LogLevel::Debug,
            oxide_core::LogLevel::Trace => crate::models::LogLevel::Trace,
        }
    }

    /// Convert oxide-core LogContext to oxide-logging LogContext
    fn convert_log_context(context: oxide_core::LogContext) -> crate::models::LogContext {
        crate::models::LogContext {
            user_id: context.user_id,
            session_id: context.session_id,
            collection: context.collection,
            record_id: context.record_id,
            operation: context.operation,
            client_ip: context.client_ip,
            user_agent: context.user_agent,
            metadata: context.metadata,
        }
    }

    /// Convert oxide-logging LoggingMetrics to oxide-core LoggingMetrics
    fn convert_metrics(metrics: crate::models::LogMetrics) -> oxide_core::LoggingMetrics {
        let mut entries_by_level = HashMap::new();
        
        // Convert the log level keys
        for (level, count) in metrics.entries_by_level {
            let core_level = match level {
                crate::models::LogLevel::Error => oxide_core::LogLevel::Error,
                crate::models::LogLevel::Warn => oxide_core::LogLevel::Warn,
                crate::models::LogLevel::Info => oxide_core::LogLevel::Info,
                crate::models::LogLevel::Debug => oxide_core::LogLevel::Debug,
                crate::models::LogLevel::Trace => oxide_core::LogLevel::Trace,
            };
            entries_by_level.insert(core_level, count);
        }

        oxide_core::LoggingMetrics {
            total_entries: metrics.total_entries,
            entries_by_level,
            storage_size_bytes: metrics.storage_size_bytes,
            error_rate_24h: metrics.error_rate_24h,
        }
    }
}

// Implementation of CorrelationIdTrait for our CorrelationId
impl oxide_core::CorrelationIdTrait for CorrelationId {
    fn new() -> Self {
        CorrelationId::new()
    }

    fn from_str(s: &str) -> Result<Self, oxide_core::AppError> {
        CorrelationId::from_str(s).map_err(|e| oxide_core::AppError::internal(format!("Invalid correlation ID: {}", e)))
    }

    fn to_string(&self) -> String {
        ToString::to_string(self)
    }
}

// Implementation of ApplicationLogger trait
impl oxide_core::ApplicationLogger for LogServiceBridge {
    type CorrelationId = CorrelationId;

    fn info(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            log_service.info(message, module).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn warn(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            log_service.warn(message, module).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn error(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            log_service.error(message, module).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn debug(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            let entry = LogEntry::new(crate::models::LogLevel::Debug, message, module);
            log_service.log(entry).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn trace(&self, message: String, module: String) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            let entry = LogEntry::new(crate::models::LogLevel::Trace, message, module);
            log_service.log(entry).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn log_with_context(
        &self,
        level: oxide_core::LogLevel,
        message: String,
        module: String,
        context: oxide_core::LogContext,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        let level = Self::convert_log_level(level);
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            log_service.log_with_context(level, message, module, context).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn log_with_correlation(
        &self,
        level: oxide_core::LogLevel,
        message: String,
        module: String,
        correlation_id: Self::CorrelationId,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        let level = Self::convert_log_level(level);
        
        Box::pin(async move {
            log_service.log_with_correlation(level, message, module, correlation_id).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn log_with_context_and_correlation(
        &self,
        level: oxide_core::LogLevel,
        message: String,
        module: String,
        context: oxide_core::LogContext,
        correlation_id: Self::CorrelationId,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        let level = Self::convert_log_level(level);
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            let entry = LogEntry::new(level, message, module)
                .with_context(context)
                .with_correlation_id(correlation_id);
            log_service.log(entry).await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }

    fn flush(&self) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            log_service.flush().await
                .map_err(|e| oxide_core::AppError::internal(format!("Logging error: {}", e)))
        })
    }
}

// Implementation of SecurityAuditor trait
impl oxide_core::SecurityAuditor for LogServiceBridge {
    type CorrelationId = CorrelationId;

    fn log_authentication(
        &self,
        actor: String,
        action: String,
        result: String,
        context: oxide_core::LogContext,
        risk_score: Option<u8>,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_authentication(actor, action, result, context, risk_score).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }

    fn log_authorization(
        &self,
        actor: String,
        target: String,
        action: String,
        result: String,
        context: oxide_core::LogContext,
        risk_score: Option<u8>,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_authorization(actor, target, action, result, context, risk_score).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }

    fn log_data_access(
        &self,
        actor: String,
        target: String,
        action: String,
        context: oxide_core::LogContext,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_data_access(actor, target, action, context).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }

    fn log_data_modification(
        &self,
        actor: String,
        target: String,
        action: String,
        context: oxide_core::LogContext,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_data_modification(actor, target, action, context).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }

    fn log_configuration_change(
        &self,
        actor: String,
        target: String,
        action: String,
        context: oxide_core::LogContext,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_configuration_change(actor, target, action, context).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }

    fn log_security_violation(
        &self,
        actor: String,
        violation_type: String,
        description: String,
        context: oxide_core::LogContext,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_security_violation(actor, violation_type, description, context).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }

    fn log_plugin_event(
        &self,
        plugin_name: String,
        action: String,
        result: String,
        context: oxide_core::LogContext,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_plugin_event(plugin_name, action, result, context).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }

    fn log_system_event(
        &self,
        event_type: String,
        description: String,
        context: oxide_core::LogContext,
    ) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<Uuid>> + Send + '_>> {
        let audit_service = self.log_service.audit_service();
        let context = Self::convert_log_context(context);
        
        Box::pin(async move {
            audit_service.log_system_event(event_type, description, context).await
                .map_err(|e| oxide_core::AppError::internal(format!("Audit error: {}", e)))
        })
    }
}

// Implementation of LoggingService trait
impl oxide_core::LoggingService for LogServiceBridge {
    fn get_metrics(&self) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<oxide_core::LoggingMetrics>> + Send + '_>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            let metrics = log_service.get_metrics().await
                .map_err(|e| oxide_core::AppError::internal(format!("Metrics error: {}", e)))?;
            Ok(Self::convert_metrics(metrics))
        })
    }

    fn shutdown(self: Arc<Self>) -> Pin<Box<dyn Future<Output = oxide_core::LoggingResult<()>> + Send>> {
        let log_service = Arc::clone(&self.log_service);
        
        Box::pin(async move {
            // Reference to the log service for shutdown
            drop(log_service); // Drop our reference
            
            // Note: This assumes LogService has a method to extract itself from the Arc
            // In practice, you might need to modify LogService to support this pattern
            // For now, we'll just return Ok since the service will be dropped
            Ok(())
        })
    }
} 