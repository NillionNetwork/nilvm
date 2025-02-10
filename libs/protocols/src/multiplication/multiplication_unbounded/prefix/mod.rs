//! Prefix multiplication protocol.

pub mod state;

pub use state::{PrefixMultTuple, PrepPrefixMultState, PrepPrefixMultStateMessage, PrepPrefixMultStateOutput};

#[cfg(test)]
mod test;
