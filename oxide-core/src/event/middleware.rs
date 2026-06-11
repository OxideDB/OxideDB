//! Event Handler Middleware
//!
//! This module provides middleware functionality for event handlers,
//! including retry logic, timeouts, circuit breakers, and other
//! resilience patterns for production environments.

use crate::AppError;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use tracing::{debug, warn};

use super::context::{AfterEventContext, BeforeEventContext};
use super::handlers::{AfterEventHandler, BeforeEventHandler};

/// Middleware for wrapping Before event handlers with additional functionality
pub trait BeforeHandlerMiddleware: Send + Sync {
    /// Wrap a Before event handler with middleware functionality
    fn wrap(&self, handler: BeforeEventHandler, handler_id: String) -> BeforeEventHandler;
}

/// Middleware for wrapping After event handlers with additional functionality
pub trait AfterHandlerMiddleware: Send + Sync {
    /// Wrap an After event handler with middleware functionality
    fn wrap(&self, handler: AfterEventHandler, handler_id: String) -> AfterEventHandler;
}

/// Timeout middleware that enforces maximum execution time for handlers
pub struct TimeoutMiddleware {
    default_timeout: Duration,
}

impl TimeoutMiddleware {
    /// Create new timeout middleware with default timeout
    pub fn new(default_timeout: Duration) -> Self {
        Self { default_timeout }
    }
}

impl Default for TimeoutMiddleware {
    fn default() -> Self {
        Self::new(Duration::from_secs(5))
    }
}

impl BeforeHandlerMiddleware for TimeoutMiddleware {
    fn wrap(&self, handler: BeforeEventHandler, handler_id: String) -> BeforeEventHandler {
        let timeout_duration = self.default_timeout;
        let handler_id_clone = handler_id.clone();

        Arc::new(move |context: &mut BeforeEventContext| {
            let handler = handler.clone();
            let handler_id = handler_id_clone.clone();
            let timeout_duration = timeout_duration;

            Box::pin(async move {
                let future = handler(context);
                match timeout(timeout_duration, future).await {
                    Ok(result) => result,
                    Err(_) => {
                        warn!(
                            "Handler {} timed out after {:?}",
                            handler_id, timeout_duration
                        );
                        Err(AppError::internal(format!(
                            "Handler {} timed out after {:?}",
                            handler_id, timeout_duration
                        )))
                    }
                }
            })
        })
    }
}

impl AfterHandlerMiddleware for TimeoutMiddleware {
    fn wrap(&self, handler: AfterEventHandler, handler_id: String) -> AfterEventHandler {
        let timeout_duration = self.default_timeout;
        let handler_id_clone = handler_id.clone();

        Arc::new(move |context: &AfterEventContext| {
            let handler = handler.clone();
            let handler_id = handler_id_clone.clone();
            let timeout_duration = timeout_duration;

            Box::pin(async move {
                let future = handler(context);
                match timeout(timeout_duration, future).await {
                    Ok(result) => result,
                    Err(_) => {
                        warn!(
                            "Handler {} timed out after {:?}",
                            handler_id, timeout_duration
                        );
                        Err(AppError::internal(format!(
                            "Handler {} timed out after {:?}",
                            handler_id, timeout_duration
                        )))
                    }
                }
            })
        })
    }
}

/// Retry middleware that automatically retries failed handlers
pub struct RetryMiddleware {
    max_retries: u32,
    initial_delay: Duration,
    max_delay: Duration,
    backoff_multiplier: f64,
}

impl RetryMiddleware {
    /// Create new retry middleware
    pub fn new(
        max_retries: u32,
        initial_delay: Duration,
        max_delay: Duration,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_retries,
            initial_delay,
            max_delay,
            backoff_multiplier,
        }
    }

    /// Create retry middleware with exponential backoff defaults
    pub fn exponential_backoff(max_retries: u32) -> Self {
        Self::new(
            max_retries,
            Duration::from_millis(100),
            Duration::from_secs(30),
            2.0,
        )
    }

    /// Create retry middleware with linear backoff
    pub fn linear_backoff(max_retries: u32, delay: Duration) -> Self {
        Self::new(max_retries, delay, delay, 1.0)
    }

    fn calculate_delay_static(
        attempt: u32,
        initial_delay: Duration,
        max_delay: Duration,
        backoff_multiplier: f64,
    ) -> Duration {
        if attempt == 0 {
            return initial_delay;
        }

        let delay_ms = initial_delay.as_millis() as f64 * backoff_multiplier.powi(attempt as i32);

        let delay = Duration::from_millis(delay_ms as u64);
        std::cmp::min(delay, max_delay)
    }
}

impl BeforeHandlerMiddleware for RetryMiddleware {
    fn wrap(&self, handler: BeforeEventHandler, handler_id: String) -> BeforeEventHandler {
        let max_retries = self.max_retries;
        let initial_delay = self.initial_delay;
        let max_delay = self.max_delay;
        let backoff_multiplier = self.backoff_multiplier;

        Arc::new(move |context: &mut BeforeEventContext| {
            let handler = handler.clone();
            let _handler_id = handler_id.clone();

            Box::pin(async move {
                let mut last_error = None;

                for attempt in 0..=max_retries {
                    match handler(context).await {
                        Ok(result) => return Ok(result),
                        Err(err) => {
                            last_error = Some(err);
                            if attempt < max_retries {
                                let delay = RetryMiddleware::calculate_delay_static(
                                    attempt,
                                    initial_delay,
                                    max_delay,
                                    backoff_multiplier,
                                );
                                tokio::time::sleep(delay).await;
                            }
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| {
                    AppError::internal("Retry failed without error".to_string())
                }))
            })
        })
    }
}

impl AfterHandlerMiddleware for RetryMiddleware {
    fn wrap(&self, handler: AfterEventHandler, handler_id: String) -> AfterEventHandler {
        let max_retries = self.max_retries;
        let initial_delay = self.initial_delay;
        let max_delay = self.max_delay;
        let backoff_multiplier = self.backoff_multiplier;

        Arc::new(move |context: &AfterEventContext| {
            let handler = handler.clone();
            let _handler_id = handler_id.clone();

            Box::pin(async move {
                let mut last_error = None;

                for attempt in 0..=max_retries {
                    match handler(context).await {
                        Ok(result) => return Ok(result),
                        Err(err) => {
                            last_error = Some(err);
                            if attempt < max_retries {
                                let delay = RetryMiddleware::calculate_delay_static(
                                    attempt,
                                    initial_delay,
                                    max_delay,
                                    backoff_multiplier,
                                );
                                tokio::time::sleep(delay).await;
                            }
                        }
                    }
                }

                Err(last_error.unwrap_or_else(|| {
                    AppError::internal("Retry failed without error".to_string())
                }))
            })
        })
    }
}

/// Circuit breaker middleware that prevents cascading failures
pub struct CircuitBreakerMiddleware {
    failure_threshold: u32,
    recovery_timeout: Duration,
    half_open_max_calls: u32,
    state: Arc<CircuitBreakerState>,
}

#[derive(Debug)]
struct CircuitBreakerState {
    failure_count: AtomicU64,
    last_failure_time: std::sync::Mutex<Option<Instant>>,
    state: std::sync::Mutex<CircuitState>,
    half_open_calls: AtomicU64,
}

#[derive(Debug, Clone, PartialEq)]
enum CircuitState {
    Closed,
    Open,
    HalfOpen,
}

impl CircuitBreakerMiddleware {
    /// Create new circuit breaker middleware
    pub fn new(
        failure_threshold: u32,
        recovery_timeout: Duration,
        half_open_max_calls: u32,
    ) -> Self {
        Self {
            failure_threshold,
            recovery_timeout,
            half_open_max_calls,
            state: Arc::new(CircuitBreakerState {
                failure_count: AtomicU64::new(0),
                last_failure_time: std::sync::Mutex::new(None),
                state: std::sync::Mutex::new(CircuitState::Closed),
                half_open_calls: AtomicU64::new(0),
            }),
        }
    }
}

impl Default for CircuitBreakerMiddleware {
    fn default() -> Self {
        Self::new(5, Duration::from_secs(60), 3)
    }
}

impl CircuitBreakerMiddleware {
    async fn execute_with_circuit_breaker<F, Fut, R>(
        &self,
        operation: F,
        handler_id: &str,
    ) -> Result<R, AppError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<R, AppError>>,
    {
        // Check circuit state
        let current_state = {
            let mut state = self.state.state.lock().unwrap();
            let now = Instant::now();

            match *state {
                CircuitState::Open => {
                    if let Some(last_failure) = *self.state.last_failure_time.lock().unwrap() {
                        if now.duration_since(last_failure) >= self.recovery_timeout {
                            *state = CircuitState::HalfOpen;
                            self.state.half_open_calls.store(0, Ordering::SeqCst);
                            debug!("Circuit breaker for {} moved to HALF_OPEN", handler_id);
                        }
                    }
                }
                CircuitState::HalfOpen => {
                    if self.state.half_open_calls.load(Ordering::SeqCst)
                        >= self.half_open_max_calls as u64
                    {
                        return Err(AppError::internal(format!(
                            "Circuit breaker for {} is HALF_OPEN and max calls exceeded",
                            handler_id
                        )));
                    }
                }
                CircuitState::Closed => {}
            }

            state.clone()
        };

        match current_state {
            CircuitState::Open => {
                return Err(AppError::internal(format!(
                    "Circuit breaker for {} is OPEN",
                    handler_id
                )));
            }
            CircuitState::HalfOpen => {
                self.state.half_open_calls.fetch_add(1, Ordering::SeqCst);
            }
            CircuitState::Closed => {}
        }

        // Execute operation
        match operation().await {
            Ok(result) => {
                // Success - reset failure count and close circuit if needed
                self.state.failure_count.store(0, Ordering::SeqCst);

                if current_state == CircuitState::HalfOpen {
                    let mut state = self.state.state.lock().unwrap();
                    *state = CircuitState::Closed;
                    debug!("Circuit breaker for {} moved to CLOSED", handler_id);
                }

                Ok(result)
            }
            Err(err) => {
                // Failure - increment count and potentially open circuit
                let failure_count = self.state.failure_count.fetch_add(1, Ordering::SeqCst) + 1;

                *self.state.last_failure_time.lock().unwrap() = Some(Instant::now());

                if failure_count >= self.failure_threshold as u64 {
                    let mut state = self.state.state.lock().unwrap();
                    *state = CircuitState::Open;
                    warn!(
                        "Circuit breaker for {} moved to OPEN after {} failures",
                        handler_id, failure_count
                    );
                }

                Err(err)
            }
        }
    }
}

impl BeforeHandlerMiddleware for CircuitBreakerMiddleware {
    fn wrap(&self, handler: BeforeEventHandler, handler_id: String) -> BeforeEventHandler {
        let circuit_breaker = Arc::new(CircuitBreakerMiddleware::new(
            self.failure_threshold,
            self.recovery_timeout,
            self.half_open_max_calls,
        ));

        Arc::new(move |context: &mut BeforeEventContext| {
            let handler = handler.clone();
            let handler_id = handler_id.clone();
            let circuit_breaker = circuit_breaker.clone();

            Box::pin(async move {
                circuit_breaker
                    .execute_with_circuit_breaker(|| handler(context), &handler_id)
                    .await
            })
        })
    }
}

impl AfterHandlerMiddleware for CircuitBreakerMiddleware {
    fn wrap(&self, handler: AfterEventHandler, handler_id: String) -> AfterEventHandler {
        let circuit_breaker = Arc::new(CircuitBreakerMiddleware::new(
            self.failure_threshold,
            self.recovery_timeout,
            self.half_open_max_calls,
        ));

        Arc::new(move |context: &AfterEventContext| {
            let handler = handler.clone();
            let handler_id = handler_id.clone();
            let circuit_breaker = circuit_breaker.clone();

            Box::pin(async move {
                circuit_breaker
                    .execute_with_circuit_breaker(|| handler(context), &handler_id)
                    .await
            })
        })
    }
}

/// Composite middleware that chains multiple middleware together
pub struct CompositeBeforeMiddleware {
    middleware: Vec<Box<dyn BeforeHandlerMiddleware>>,
}

impl CompositeBeforeMiddleware {
    /// Create new composite middleware
    pub fn new() -> Self {
        Self {
            middleware: Vec::new(),
        }
    }

    /// Add middleware to the chain
    pub fn with_middleware(mut self, middleware: Box<dyn BeforeHandlerMiddleware>) -> Self {
        self.middleware.push(middleware);
        self
    }

    /// Create a production-ready middleware chain
    pub fn production() -> Self {
        Self::new()
            .with_middleware(Box::new(TimeoutMiddleware::new(Duration::from_secs(10))))
            .with_middleware(Box::new(RetryMiddleware::exponential_backoff(3)))
            .with_middleware(Box::new(CircuitBreakerMiddleware::default()))
    }

    /// Create a development middleware chain
    pub fn development() -> Self {
        Self::new().with_middleware(Box::new(TimeoutMiddleware::new(Duration::from_secs(30))))
    }
}

impl Default for CompositeBeforeMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl BeforeHandlerMiddleware for CompositeBeforeMiddleware {
    fn wrap(&self, mut handler: BeforeEventHandler, handler_id: String) -> BeforeEventHandler {
        // Apply middleware in reverse order so the first added is outermost
        for middleware in self.middleware.iter().rev() {
            handler = middleware.wrap(handler, handler_id.clone());
        }
        handler
    }
}

/// Composite middleware for After handlers
pub struct CompositeAfterMiddleware {
    middleware: Vec<Box<dyn AfterHandlerMiddleware>>,
}

impl CompositeAfterMiddleware {
    /// Create new composite middleware
    pub fn new() -> Self {
        Self {
            middleware: Vec::new(),
        }
    }

    /// Add middleware to the chain
    pub fn with_middleware(mut self, middleware: Box<dyn AfterHandlerMiddleware>) -> Self {
        self.middleware.push(middleware);
        self
    }

    /// Create a production-ready middleware chain
    pub fn production() -> Self {
        Self::new()
            .with_middleware(Box::new(TimeoutMiddleware::new(Duration::from_secs(10))))
            .with_middleware(Box::new(RetryMiddleware::exponential_backoff(3)))
            .with_middleware(Box::new(CircuitBreakerMiddleware::default()))
    }

    /// Create a development middleware chain
    pub fn development() -> Self {
        Self::new().with_middleware(Box::new(TimeoutMiddleware::new(Duration::from_secs(30))))
    }
}

impl Default for CompositeAfterMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

impl AfterHandlerMiddleware for CompositeAfterMiddleware {
    fn wrap(&self, mut handler: AfterEventHandler, handler_id: String) -> AfterEventHandler {
        // Apply middleware in reverse order so the first added is outermost
        for middleware in self.middleware.iter().rev() {
            handler = middleware.wrap(handler, handler_id.clone());
        }
        handler
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_timeout_middleware() {
        let middleware = TimeoutMiddleware::new(Duration::from_millis(100));
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let handler: BeforeEventHandler = Arc::new(move |_context: &mut BeforeEventContext| {
            let counter = call_count_clone.clone();
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(200)).await; // Longer than timeout
                Ok(())
            }) as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let wrapped =
            BeforeHandlerMiddleware::wrap(&middleware, handler, "test-handler".to_string());
        let mut context = BeforeEventContext::new_create("test".to_string(), serde_json::json!({}));

        let result = wrapped(&mut context).await;
        assert!(result.is_err());
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_middleware() {
        let middleware =
            RetryMiddleware::new(2, Duration::from_millis(10), Duration::from_millis(10), 1.0);
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let handler: BeforeEventHandler = Arc::new(move |_context: &mut BeforeEventContext| {
            let counter = call_count_clone.clone();
            Box::pin(async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    Err(AppError::internal("Simulated failure"))
                } else {
                    Ok(())
                }
            }) as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let wrapped =
            BeforeHandlerMiddleware::wrap(&middleware, handler, "test-handler".to_string());
        let mut context = BeforeEventContext::new_create("test".to_string(), serde_json::json!({}));

        let result = wrapped(&mut context).await;
        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 3); // Original + 2 retries
    }

    #[tokio::test]
    async fn test_circuit_breaker_middleware() {
        let middleware = CircuitBreakerMiddleware::new(2, Duration::from_millis(50), 1);
        let handler: BeforeEventHandler = Arc::new(|_context: &mut BeforeEventContext| {
            Box::pin(async move { Err(AppError::internal("Always fails")) })
                as Pin<Box<dyn Future<Output = Result<(), AppError>> + Send + '_>>
        });

        let wrapped =
            BeforeHandlerMiddleware::wrap(&middleware, handler, "test-handler".to_string());

        // First two calls should execute and fail
        for _ in 0..2 {
            let mut context =
                BeforeEventContext::new_create("test".to_string(), serde_json::json!({}));
            let result = wrapped(&mut context).await;
            assert!(result.is_err());
        }

        // Third call should be blocked by circuit breaker
        let mut context = BeforeEventContext::new_create("test".to_string(), serde_json::json!({}));
        let result = wrapped(&mut context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circuit breaker"));
    }

    #[tokio::test]
    async fn test_composite_middleware() {
        let middleware = CompositeBeforeMiddleware::new()
            .with_middleware(Box::new(TimeoutMiddleware::new(Duration::from_millis(500))))
            .with_middleware(Box::new(RetryMiddleware::linear_backoff(
                1,
                Duration::from_millis(10),
            )));

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let handler: BeforeEventHandler = Arc::new(move |_context: &mut BeforeEventContext| {
            let counter = call_count_clone.clone();
            Box::pin(async move {
                let count = counter.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    Err(AppError::internal("First call fails"))
                } else {
                    Ok(())
                }
            })
        });

        let wrapped =
            BeforeHandlerMiddleware::wrap(&middleware, handler, "test-handler".to_string());
        let mut context = BeforeEventContext::new_create("test".to_string(), serde_json::json!({}));

        let result = wrapped(&mut context).await;
        assert!(result.is_ok());
        assert_eq!(call_count.load(Ordering::SeqCst), 2); // Original + 1 retry
    }
}
