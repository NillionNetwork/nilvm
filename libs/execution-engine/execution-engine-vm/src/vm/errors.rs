//! The errors returned in the evaluation stage.

use crate::vm::memory::RuntimeMemoryError;
use anyhow::Error;
use jit_compiler::models::memory::AddressCountError;
use math_lib::modular::Overflow;

/// An error during the evaluation of a program.
#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    /// An overflow during conversion between `BigInt` and `ModularNumber`.
    #[error("overflow: {0}")]
    Overflow(#[from] Overflow),

    /// An error during runtime memory accessor.
    #[error("runtime memory: {0}")]
    RuntimeMemory(#[from] RuntimeMemoryError),

    /// Not implemented.
    #[error("not implemented: {0}")]
    Unimplemented(String),

    /// Address count failed
    #[error("address count failed: {0}")]
    AddressCount(#[from] AddressCountError),

    /// Division by Zero
    #[error("division by zero")]
    DivByZero,

    /// Negative shift amount
    #[error("negative shift amount")]
    NegativeShift,

    /// Party output adapter
    #[error(transparent)]
    PartyOutputAdapter(#[from] Error),

    /// This error is thrown when an output can not be retrieved.
    #[error("{0} can not be retrieve: {1}")]
    OutputRetrieveError(String, String),
}
