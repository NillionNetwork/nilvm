//! The DIVISION protocol

pub mod state;
mod utils;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

#[cfg(test)]
mod test;
