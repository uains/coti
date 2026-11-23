//! Logging initialization for the application boundary.

use tracing_subscriber::EnvFilter;

/// Initializes the global tracing subscriber.
///
/// The filter is read from `RUST_LOG` when present and defaults to `info`.
/// Repeated or conflicting initialization is ignored so embedders and tests can
/// call this safely.
pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}
