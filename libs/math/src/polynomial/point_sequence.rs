//! Point Sequence.

use crate::{
    errors::{InterpolationError, PointSequenceNotFound, ShareNotFound},
    fields::Field,
    polynomial::point::Point,
};
use std::collections::HashSet;

/// Point sequence.
#[derive(Clone)]
pub struct PointSequence<F>
where
    F: Field,
{
    points: Vec<Point<F>>,
}

impl<F: Field> Default for PointSequence<F> {
    fn default() -> Self {
        Self { points: Vec::new() }
    }
}

impl<F: Field> PointSequence<F> {
    /// Get the points in the sequence.
    pub fn points(&self) -> &Vec<Point<F>> {
        &self.points
    }

    /// Consume the point sequence and return the points in it.
    pub fn into_points(self) -> Vec<Point<F>> {
        self.points
    }

    /// Check if points is empty.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Checks if there are any duplicated abscissas.
    pub fn has_duplicates(&self) -> bool {
        let mut x_set = HashSet::new();
        for p in self.points.iter() {
            x_set.insert(&p.x);
        }
        if x_set.len() == self.points.len() {
            return false;
        }
        true
    }

    /// Add a point to the point sequence.
    pub fn push(&mut self, point: Point<F>) {
        self.points.push(point)
    }

    /// Get the X coordinates of the sequence.
    fn get_x_s(&self) -> Vec<&F::Inner> {
        self.points.iter().map(|share| &share.x).collect()
    }

    /// Get the Y coordinates of the sequence.
    fn get_y_s(&self) -> Vec<&F::Element> {
        self.points.iter().map(|share| &share.y).collect()
    }

    /// Get generated share with an index.
    pub fn get_share(&self, index: usize) -> Result<F::Element, ShareNotFound> {
        Ok(self.points.get(index).ok_or(ShareNotFound)?.y)
    }

    /// Get the unzipped vectors for X and Y coordinates.
    pub fn unzip(&self) -> (Vec<&F::Inner>, Vec<&F::Element>) {
        (self.get_x_s(), self.get_y_s())
    }

    /// Lagrange interpolation for Point Sequence at Zero.
    pub fn lagrange_interpolate(&self) -> Result<F::Element, InterpolationError> {
        if self.points.is_empty() {
            return Err(InterpolationError::EmptySequence);
        }

        let mut res: F::Element = F::ZERO;

        for (i, pi) in self.points().iter().enumerate() {
            let mut den = F::ONE;
            let mut num = F::ONE;
            for (j, pj) in self.points().iter().enumerate() {
                if j != i {
                    let xi = F::as_element(pi.x);
                    let xj = F::as_element(pj.x);
                    den = den * &(xi - &xj);
                    num = num * &-xj;
                }
            }
            res = res + &((num / &den)? * &pi.y);
        }
        Ok(res)
    }

    /// Get initial part of Point Sequence till count.
    pub fn take(&self, count: u64) -> Result<PointSequence<F>, PointSequenceNotFound> {
        let mut point_sequence: PointSequence<F> = PointSequence::default();
        for point_index in 0..count as usize {
            point_sequence.push(self.points.get(point_index).ok_or(PointSequenceNotFound)?.clone());
        }
        Ok(point_sequence)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{fields::PrimeField, modular::ModularNumber, test_prime};

    test_prime!(P13, 13u64);

    #[test]
    fn test_lagrange_interpolation() {
        type Field = PrimeField<P13>;
        let coordinates: Vec<(u32, u32)> = vec![(2, 10), (8, 5), (3, 10)];
        let mut point_sequence = PointSequence::<Field>::default();
        for (x, y) in coordinates {
            point_sequence.push(Point::new(x.into(), ModularNumber::from_u32(y)));
        }
        let result = point_sequence.lagrange_interpolate().unwrap();
        assert_eq!(result, ModularNumber::from_u32(9));
    }
}
