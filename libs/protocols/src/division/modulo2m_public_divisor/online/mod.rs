//! The MOD2M protocol

pub mod state;

#[cfg(any(test, feature = "bench", feature = "testing"))]
pub mod protocol;

#[cfg(test)]
mod test;
