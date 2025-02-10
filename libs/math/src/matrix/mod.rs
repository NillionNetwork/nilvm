//! Matrix operations.

pub mod matrix;
pub mod ops;

pub use matrix::{Matrix, MatrixError};
#[allow(unused_imports)]
pub use ops::*;
