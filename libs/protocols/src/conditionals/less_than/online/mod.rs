//! The COMPARE protocol.

pub mod state;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

#[cfg(test)]
mod test;
