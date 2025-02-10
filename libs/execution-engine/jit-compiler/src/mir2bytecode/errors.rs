//! Errors during the Bytecode compilation of a program.
use crate::models::{bytecode::memory::BytecodeMemoryError, memory::AddressCountError};
use nada_compiler_backend::{literal_value::LiteralValueError, mir::OperationId};

/// An MIR2BytecodeError during the Bytecode compilation of a program.
#[derive(Debug, thiserror::Error)]
pub enum MIR2BytecodeError {
    /// A party was not found
    #[error("party {0} was not found")]
    PartyNotFound(String),

    /// A literal was not found
    #[error("literal {0} was not found")]
    LiteralNotFound(String),

    /// An input was not found
    #[error("input {0} was not found")]
    InputNotFound(String),

    /// The program contains references to inputs that are not defined
    #[error("inputs do not exist: {0:?}")]
    InputsNotExist(Vec<String>),

    /// The input is not used
    #[error("inputs is not used: {0}")]
    InputNotReferenced(String),

    /// MIR operation was not found
    #[error("operation {0} was not found")]
    OperationNotFound(OperationId),

    /// Allocation in memory fail
    #[error("allocation in memory fail")]
    MemoryAllocation(#[from] BytecodeMemoryError),

    /// Literal value parsing failed
    #[error("failed parsing literal")]
    LiteralValueParsingFailed(#[from] LiteralValueError),

    /// Address is not available for operation
    #[error("address is not available for {0}")]
    AddressNotAvailable(String),

    /// Error while operation construction
    #[error("construction of {0} failed: {1}")]
    BytecodeElementNotCreated(String, String),

    /// Operation is not supported
    #[error("operation is not supported")]
    OperationNotSupported(&'static str),

    /// This error is thrown where the address count for a type fails.
    #[error(transparent)]
    AddressCount(#[from] AddressCountError),

    /// This error is thrown when the offset count for an array accessor failed.
    #[error("offset calculation for an array accessor failed")]
    ArrayAccessorOffset,

    /// This error is thrown when the offset count for a tuple accessor failed.
    #[error("offset calculation for a tuple accessor failed")]
    TupleAccessorOffset,

    /// This error is thrown when the transformation found and array accessor over source element
    /// that is not a tuple.
    #[error("incompatible sopurce type with a tuple accessor")]
    TupleAccessorIncompatibleType,

    /// This error is thrown when an input is load multiple times.
    #[error("input {0} is loaded multiple times")]
    RedundantLoad(String),

    /// This error is thrown when the program defines an input of an unsupported type
    #[error("input type is not supported: {0}")]
    UnsupportedInputType(&'static str),
}
