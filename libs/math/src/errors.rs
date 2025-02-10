//! Crate errors.

use crate::matrix::MatrixError;
use thiserror::Error;

/// Share not found error.
#[derive(Error, Debug)]
#[error("share not found")]
pub struct ShareNotFound;

/// Point sequence not found.
#[derive(Error, Debug)]
#[error("point sequence not found")]
pub struct PointSequenceNotFound;

/// Failed Interpolation Error
#[derive(Error, Debug, Eq, PartialEq)]
pub enum InterpolationError {
    /// Division by zero.
    #[error("division by zero")]
    DivByZero,

    /// Polynomial error.
    #[error("polynomial error: {0}")]
    Polynomial(#[from] PolynomialError),

    /// Empty point sequence.
    #[error("empty point sequence")]
    EmptySequence,

    /// Coefficient not found.
    #[error("lagrange polynomial coefficient not found")]
    CoefficientNotFound,

    /// The point sequence has duplicate abscissas.
    #[error("point sequence has duplicate abscissas")]
    DuplicateAbscissas,

    /// The point sequence abscissas do not match interpolator.
    #[error("point sequence has mismatched abscissas")]
    MismatchedAbscissas,

    /// Matrix error.
    #[error("error constructing matrix")]
    MatrixError(#[from] MatrixError),
}

impl From<DivByZero> for InterpolationError {
    fn from(_: DivByZero) -> Self {
        Self::DivByZero
    }
}

/// Polynomial error.
#[derive(Error, Debug, Eq, PartialEq)]
pub enum PolynomialError {
    /// Division by zero.
    #[error("division by zero")]
    DivByZero,

    /// Coefficient not found.
    #[error("polynomial coefficient not found")]
    CoefficientNotFound,

    /// Integer overflow error.
    #[error("integer overflow")]
    IntegerOverflow,
}

impl From<DivByZero> for PolynomialError {
    fn from(_: DivByZero) -> Self {
        Self::DivByZero
    }
}

/// Division by zero.
#[derive(Error, Debug, Eq, PartialEq)]
#[error("division by zero")]
pub struct DivByZero;
