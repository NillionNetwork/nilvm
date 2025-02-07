//! The MODULO protocol

pub mod state;

#[cfg(any(test, feature = "bench", feature = "testing"))]
pub mod protocol;

#[cfg(test)]
pub mod test;
