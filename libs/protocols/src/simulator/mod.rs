//! Simulators used to test protocol.
//!
//! Simulator must only be used for tests. Do not include in anything that is not a test.

#[cfg(any(test, feature = "validation"))]
pub mod symmetric;
