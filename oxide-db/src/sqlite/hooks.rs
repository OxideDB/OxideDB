//! Shared helpers for enforcing SQLite event hook outcomes.

use oxide_core::{event::handlers::HandlerExecutionResult, AppError};

pub(super) fn ensure_before_handlers_succeeded(
    results: &[HandlerExecutionResult],
) -> Result<(), AppError> {
    if let Some(result) = results
        .iter()
        .find(|result| !result.success && !result.skipped)
    {
        let error = result
            .error
            .clone()
            .unwrap_or_else(|| "Unknown handler error".to_string());

        if let Some(plugin_error) = plugin_error_from_handler_error(&error) {
            return Err(plugin_error);
        }

        return Err(AppError::internal(format!(
            "Before event handler {} failed: {}",
            result.handler_id, error
        )));
    }

    Ok(())
}

fn plugin_error_from_handler_error(error: &str) -> Option<AppError> {
    let rest = error.strip_prefix("Plugin error: ")?;
    let (plugin_name, message) = rest.split_once(" - ")?;

    Some(AppError::plugin(
        plugin_name.to_string(),
        message.to_string(),
    ))
}
