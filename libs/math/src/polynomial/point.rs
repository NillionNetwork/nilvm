//! Point
use std::fmt::Debug;

use crate::fields::Field;

/// Point
#[derive(Clone)]
pub struct Point<F>
where
    F: Field,
{
    pub(crate) x: F::Inner,
    pub(crate) y: F::Element,
}

impl<F> Point<F>
where
    F: Field,
{
    /// Creates a new point.
    pub fn new(x: F::Inner, y: F::Element) -> Point<F> {
        Point { x, y }
    }

    /// Consumes the point and returns the (x, y) coordinates in it.
    pub fn into_coordinates(self) -> (F::Inner, F::Element) {
        (self.x, self.y)
    }
}

impl<F: Field> Debug for Point<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Point").field("x", &self.x).field("y", &self.y).finish()
    }
}
