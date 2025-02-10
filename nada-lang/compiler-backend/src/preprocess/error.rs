//! MIR Preprocessor errors

use mir_model::{MIRProgramMalformed, OperationId};

/// MIRPreprocessorError
#[derive(Debug, thiserror::Error)]
pub enum MIRPreprocessorError {
    /// Unexpected type
    #[error("unexpected type: {0}")]
    UnexpectedType(String),

    /// Invalid output type
    #[error("invalid output type: {0}")]
    InvalidOutputType(String),

    /// Operation is not supported
    #[error("operation is not supported")]
    OperationNotSupported(&'static str),

    /// Operation was not supported
    #[error("operation was not found")]
    OperationNotFound,

    /// Invalid function argument
    #[error("invalid function argument: {0}")]
    InvalidFunctionArgument(String),

    /// Invalid function argument
    #[error("function call argument {0} does not reference a valid operation")]
    InvalidFunctionCallArgument(i64),

    /// Missing function call argument
    #[error("missing function call argument: {0}")]
    MissingFunctionCallArgument(String),

    /// Cannot set children to operation
    #[error("cannot set children to operation {0}")]
    CannotSetChildren(String),

    /// Missing children
    #[error("expecting operation {0} to have children")]
    MissingChildren(String),

    /// Unexpected number of children
    #[error("unexpected number of children found, expecting {0}, got {1}")]
    UnexpectedChildrenCount(usize, usize),

    /// Unexpected error
    #[error("unexpected error")]
    Unexpected,

    /// Missing function
    #[error("missing function with id {0}")]
    MissingFunction(OperationId),

    /// MIR model is malformed
    #[error("program mir is malformed: {0}")]
    MalformedModel(&'static str),

    /// This error is throw when the preprocessor try to preprocess an operation that is not
    /// preprocessable
    #[error("operation can not be preprocessed")]
    NotPreprocessable,

    /// MIR program is malformed
    #[error(transparent)]
    Malformed(#[from] MIRProgramMalformed),
}
