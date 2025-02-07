//! The TRUNCPR protocol

pub mod state;

#[cfg(any(test, feature = "validation"))]
pub mod protocol;

#[cfg(test)]
mod test;
