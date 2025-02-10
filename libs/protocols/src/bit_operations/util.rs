//! Utils for bit decomposition.

use math_lib::modular::{AsBits, Modular};

/// Whether we have reached the final round.
pub fn is_final_round<T: Modular>(round_id: usize) -> bool {
    (1 << round_id) >= T::MODULO.bits()
}
