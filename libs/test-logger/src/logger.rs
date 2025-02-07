//! Logger for tests.
//!
//! Provides a convenience logger for integration tests

use once_cell::sync::Lazy;

pub static LOGGER_INIT: Lazy<fn()> = Lazy::new(logger_init);

fn logger_init() -> fn() {
    #[cfg(feature = "tracing")]
    tracing_subscriber::fmt::init();
    #[cfg(feature = "log")]
    env_logger::init();
    || ()
}
