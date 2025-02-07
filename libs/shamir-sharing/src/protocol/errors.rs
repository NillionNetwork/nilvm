//! Secret Sharing Scheme errors.

use crate::party::TooManyParties;
use math_lib::{
    decoders::ECCError,
    errors::{InterpolationError, PolynomialError},
    matrix::MatrixError,
};
use thiserror::Error;

/// Share generation failure.
#[derive(Error, Debug)]
pub enum ShareGenerationError {
    /// Failed to evaluate polynomial.
    #[error("failed evaluating polynomial")]
    PolynomialEvaluation(#[from] PolynomialError),
}

/// Secret recovery failure.
#[derive(Error, Debug)]
pub enum RecoverSecretError {
    /// The polynomial interpolation failed.
    #[error(transparent)]
    Interpolation(#[from] InterpolationError),

    /// The error correction failed.
    #[error(transparent)]
    ECC(#[from] ECCError),

    /// The polynomial operation failed.
    #[error(transparent)]
    Polynomial(#[from] PolynomialError),

    /// A provided party id was not found.
    #[error("party not found")]
    PartyNotFound,
}

/// Hyper map failure.
#[derive(Error, Debug)]
pub enum HyperMapError {
    /// Integer overflow or underflow.
    #[error("interger overflow/underflow")]
    Arithmetic,

    /// The polynomial interpolation failed.
    #[error(transparent)]
    Interpolation(#[from] InterpolationError),

    /// A provided party id was not found.
    #[error("party not found")]
    PartyNotFound,

    /// Error construction hyper-invertible matrix.
    #[error("failed constructing matrix: {0}")]
    Matrix(#[from] MatrixError),
}

/// Secret encode failure.
#[derive(Error, Debug)]
pub enum EncoderError {
    /// Integer overflow error.
    #[error("integer overflow")]
    IntegerOverflow,

    /// Locale size doesn't match polynomial degree.
    #[error("locale degree has to be polynomial degree + 1")]
    LocaleMismatch,

    /// Too many secrets tried to be packed.
    #[error("too many secrets")]
    TooManySecrets,

    /// The polynomial interpolation failed.
    #[error(transparent)]
    Interpolation(#[from] InterpolationError),

    /// Too many parties were provided during the mapper initialization.
    #[error(transparent)]
    TooManyParties(#[from] TooManyParties),

    /// Mismatched secret count tried to be packed.
    #[error("mismatched secret count")]
    SecretCountMismatch,
}

/// Secret encode failure.
#[derive(Error, Debug)]
pub enum ShamirError {
    /// The polynomial interpolation failed.
    #[error(transparent)]
    Interpolation(#[from] InterpolationError),

    /// Too many parties were provided during the mapper initialization.
    #[error(transparent)]
    TooManyParties(#[from] TooManyParties),

    /// Polynomial degree too high for given number of parties.
    #[error("polynomial degree too high for provided parties")]
    TooHighDegree,

    /// Error construction hyper-invertible matrix.
    #[error("failed constructing matrix: {0}")]
    MatrixBuild(#[from] MatrixError),

    /// Integer overflow or underflow.
    #[error("interger overflow/underflow")]
    Arithmetic,
}
