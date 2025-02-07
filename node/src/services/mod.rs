//! Services in the Node.
//!
//! A service is a stateless entity that serves a specific purpose. Services can use IO (e.g. repositories) but as
//! opposed to actors they should not store any state within their instance.

pub(crate) mod auxiliary_material;
pub(crate) mod blob;
pub(crate) mod nonce;
pub(crate) mod offsets;
pub(crate) mod payments;
pub(crate) mod preprocessing;
pub(crate) mod programs;
pub(crate) mod receipts;
pub(crate) mod results;
pub(crate) mod runtime_elements;
pub(crate) mod scheduling;
pub(crate) mod time;
pub(crate) mod user_values;
pub(crate) mod uuid;
