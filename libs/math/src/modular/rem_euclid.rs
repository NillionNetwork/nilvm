//! Euclidean Remainder

/// Euclidean Remainder trait
pub trait RemEuclid<Rhs = Self> {
    /// Output type of the Euclidean Remainder Operation
    type Output;

    /// function to do the Euclidean Remainder Operation
    fn rem_euclid(self, rhs: Rhs) -> Self::Output;
}
