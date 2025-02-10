//! PREP-MODULO protocol.
//!
//! This protocol produces shares of elements that can then be used to run the MODULO protocol.

use anyhow::anyhow;

pub mod output;
pub mod state;

#[cfg(any(test, feature = "bench"))]
pub mod protocol;

#[cfg(test)]
pub(crate) mod test;

#[cfg(any(test, feature = "validation"))]
pub mod validation;

use self::state::PrepModuloState;
pub use output::{EncodedPrepModuloShares, PrepModuloShares, PrepModuloStateOutput};

state_machine_macros::define_encoded_dyn_state_machine!(
    PrepModuloState,
    PrepModuloStateOutput<EncodedPrepModuloShares>
);
