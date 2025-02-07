//! Errors during the Protocols compilation of a program.
use crate::models::{
    bytecode::memory::{BytecodeAddress, BytecodeMemoryError},
    memory::AddressCountError,
    protocols::memory::ProtocolMemoryError,
};
use nada_type::TypeError;
use std::borrow::Cow;

/// A Bytecode2ProtocolError during the Protocols compilation of a program.
#[derive(Debug, thiserror::Error)]
pub enum Bytecode2ProtocolError {
    /// This error is thrown when we try to transform an operation that has been transformed
    /// previously
    #[error("operation had transformed previously")]
    DuplicateTransformation,

    /// This error is thrown when we find a compound type that is not supported yet
    #[error("unsupported compound type")]
    UnsupportedCompoundType,

    /// This error is thrown when we try to refer to the result of transforming a bytecode operation,
    /// but this one has not been transformed yet
    #[error("bytecode operation has not been transformed")]
    ResultantProtocolNotFound,

    /// This error is thrown when a protocol result can not be adapted for any of the existing adapters.
    #[error("protocol result cannot be adapted for any adapter")]
    AdapterNotFound,

    /// An operation is not supported.
    #[error("operation {0} is not supported")]
    OperationNotSupported(String),

    /// Bytecode memory overflow
    #[error(transparent)]
    BytecodeMemoryOverflow(#[from] BytecodeMemoryError),

    /// Protocol memory overflow
    #[error(transparent)]
    ProtocolMemoryOverflow(#[from] ProtocolMemoryError),

    /// Error reading memory sizeof
    #[error("reading sizeof failed: {0}")]
    SizeOf(#[from] AddressCountError),

    /// No compound type.
    #[error("no compound type")]
    NoCompoundType,

    /// Unimplemented feature error.
    #[error(transparent)]
    TypeError(#[from] TypeError),

    /// A logic error during the compilation.
    #[error("logic error: {0}")]
    Logic(Cow<'static, str>),

    /// Bytecode operation not found
    #[error("bytecode operation not found {0}")]
    OperationNotFound(BytecodeAddress),
}

impl Bytecode2ProtocolError {
    /// Creates a new logic error
    pub fn logic<T: Into<Cow<'static, str>>>(message: T) -> Self {
        Self::Logic(message.into())
    }
}
