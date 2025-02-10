//! Outputs for the POLY EVAL protocol.  
use math_lib::modular::{Modular, ModularNumber};

/// The output of this state machine.
pub enum PolyEvalStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output of the protocol.
        outputs: Vec<T>,
    },
}

#[derive(Debug, Clone)]
/// The output of the POLY EVAL protocol.
pub struct PolyEvalShares<T: Modular> {
    /// The given polynomial evaluated at the given abscissa.
    pub poly_x: ModularNumber<T>,
}
