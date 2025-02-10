//! PRIVATE OUTPUT EQUALITY protocol.

/// The polynomial degree used for the evaluation of the private equality protocol.
/// The source establishes that: deg(P(X)) > 2.
pub const POLY_EVAL_DEGREE: u64 = 64;

/// The PREP protocol for PRIVATE OUTPUT EQUALITY
pub mod offline;

/// The private equality protocol
pub mod online;
pub use online::state::*;
