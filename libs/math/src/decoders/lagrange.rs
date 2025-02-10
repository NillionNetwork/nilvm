//! Lagrange in Finite Field.

use crate::{
    errors::InterpolationError,
    fields::{Field, Inv},
    polynomial::{point_sequence::PointSequence, Polynomial},
};
use std::collections::HashMap;

/// Lagrange Polynomial.
#[derive(Debug, Clone)]
pub struct Lagrange<F>
where
    F: Field,
{
    /// Abscissas of the Lagrange polynomial.
    abscissas: Vec<F::Element>,

    /// Barycentric weights.
    weights: HashMap<F::Inner, F::Element>,

    /// Lagrange coefficients for evaluation at Zero.
    coefficients: HashMap<F::Inner, F::Element>,
}

impl<F: Field> Lagrange<F> {
    /// Creates a new Lagrange polynomial expression, O(n^2).
    pub fn new(abscissas: Vec<F::Element>) -> Result<Lagrange<F>, InterpolationError> {
        let mut coefs = Vec::new();
        let mut weights = HashMap::new();
        let mut w = F::ZERO;
        for (i, xi) in abscissas.iter().enumerate() {
            let xi_inv = -xi.inv()?;
            let mut wi = F::ONE;
            for (j, xj) in abscissas.iter().enumerate() {
                if j != i {
                    wi = wi * &(*xi - xj);
                }
            }
            wi = wi.inv()?;
            let ci = xi_inv * &wi;
            w = w + &ci;
            coefs.push(ci);
            weights.insert(F::as_inner(*xi), wi);
        }
        let mut coefficients = HashMap::new();
        w = w.inv()?;
        for (&c, x) in coefs.iter().zip(abscissas.iter()) {
            let c = c * &w;
            coefficients.insert(F::as_inner(*x), c);
        }
        Ok(Lagrange { abscissas, weights, coefficients })
    }

    /// Return the abscissas of the Lagrange polynomial.
    pub fn abscissas(&self) -> &Vec<F::Element> {
        &self.abscissas
    }

    /// Lagrange interpolation at Zero, O(n).
    pub fn interpolate(&self, sequence: &PointSequence<F>) -> Result<F::Element, InterpolationError> {
        if sequence.points().len() != self.abscissas.len() {
            return Err(InterpolationError::MismatchedAbscissas);
        }

        let mut res = F::ZERO;

        for pi in sequence.points().iter() {
            let ci = self.coefficients.get(&pi.x).ok_or(InterpolationError::MismatchedAbscissas)?;
            res = res + &(*ci * &pi.y);
        }
        Ok(res)
    }

    /// Partial Lagrange interpolation at Zero, only producing one of the factors in the sum.
    pub fn partial(&self, x: &F::Inner, y: &F::Element) -> Result<F::Element, InterpolationError> {
        let ci = self.coefficients.get(x).ok_or(InterpolationError::MismatchedAbscissas)?;
        let result = *ci * y;
        Ok(result)
    }

    /// Evaluate lagrange polynomial at x, O(n).
    pub fn eval(&self, sequence: &PointSequence<F>, x: &F::Element) -> Result<F::Element, InterpolationError> {
        if sequence.points().len() != self.abscissas.len() {
            return Err(InterpolationError::MismatchedAbscissas);
        }

        let mut top = F::ZERO;
        let mut bot = F::ZERO;

        for pi in sequence.points().iter() {
            let ci = F::as_element(pi.x) - x;
            if ci == F::ZERO {
                return Ok(pi.y);
            }
            let wi = self.weights.get(&pi.x).ok_or(InterpolationError::CoefficientNotFound)?;
            let ci = (-*wi / &ci)?;
            bot = bot + &ci;
            top = top + &(ci * &pi.y);
        }
        let res = (top / &bot)?;
        Ok(res)
    }
}

impl<F, F2> TryFrom<&Lagrange<F>> for Lagrange<F2>
where
    F: Field,
    F2: Field<Inner = F::Inner>,
{
    type Error = InterpolationError;

    fn try_from(original: &Lagrange<F>) -> Result<Self, Self::Error> {
        let mut abscissas = Vec::new();
        for x in &original.abscissas {
            abscissas.push(F2::as_element(F::as_inner(*x)));
        }
        let lagrange = Lagrange::new(abscissas)?;
        Ok(lagrange)
    }
}

/// Construct a new lagrange polynomial from point sequence, O(n^3).
pub fn lagrange_polynomial<F: Field>(sequence: &PointSequence<F>) -> Result<Polynomial<F>, InterpolationError> {
    let mut res = F::polynomial(Vec::new());
    for (i, pi) in sequence.points().iter().enumerate() {
        let mut den = F::ONE;
        let mut num = F::polynomial(vec![F::ONE]);
        for (j, pj) in sequence.points().iter().enumerate() {
            if j != i {
                let xi = F::as_element(pi.x);
                let xj = F::as_element(pj.x);
                den = den * &(xi - &xj);
                let px = F::polynomial(vec![-xj, F::ONE]);
                num = (num * &px)?;
            }
        }
        let fac = (pi.y / &den)?;
        let f = F::polynomial(vec![fac]);
        num = (num * &f)?;
        res = res + &num;
    }
    res.canonicalize()?;
    Ok(res)
}

#[cfg(any(test, feature = "bench"))]
#[allow(clippy::unwrap_used, clippy::arithmetic_side_effects, dead_code, unused_imports)]
pub mod lagrange_test {
    //! Lagrange tests.

    use super::*;
    use crate::{
        fields::PrimeField,
        modular::{Modular, ModularNumber, Prime},
        polynomial::{point::Point, Polynomial},
        test_prime,
    };
    use crypto_bigint::U64;
    use std::sync::Arc;

    test_prime!(Fft64, 18446744072637906947u64);
    test_prime!(P13, 13u64);

    fn make_polynomial<T: Prime>(coefficients: &[i32]) -> Polynomial<PrimeField<T>> {
        let coefs = coefficients.into_iter().map(|c| ModularNumber::from_u32(*c as u32)).collect();
        Polynomial::new(coefs)
    }

    #[test]
    fn lagrange_interpolate() {
        type Field = PrimeField<P13>;
        let coordinates: Vec<(u32, u32)> = vec![(2, 10), (8, 5), (3, 10)];
        let mut point_sequence = PointSequence::default();
        let mut abscissas = Vec::new();
        for (x, y) in coordinates {
            abscissas.push(ModularNumber::from_u32(x));
            point_sequence.push(Point::new(x.into(), ModularNumber::from_u32(y)));
        }
        let lagrange = Lagrange::<Field>::new(abscissas).unwrap();
        let result = lagrange.interpolate(&point_sequence).unwrap();
        assert_eq!(result, ModularNumber::from_u32(9));
    }

    #[test]
    fn lagrange_eval() {
        type Field = PrimeField<P13>;
        let coordinates: Vec<(u32, u32)> = vec![(2, 10), (8, 5), (3, 10)];
        let mut point_sequence = PointSequence::<Field>::default();
        let mut abscissas = Vec::new();
        for (x, y) in coordinates {
            abscissas.push(ModularNumber::from_u32(x));
            point_sequence.push(Point::new(x.into(), ModularNumber::from_u32(y)));
        }
        let lagrange = Lagrange::new(abscissas).unwrap();
        let result = lagrange.eval(&point_sequence, &ModularNumber::from_u32(4)).unwrap();
        assert_eq!(result, ModularNumber::ONE);
    }

    #[test]
    fn lagrange_polynomial_constructs() {
        type Field = PrimeField<P13>;
        let coordinates: Vec<(u32, u32)> = vec![(1, 1), (2, 4), (3, 9)];
        let mut point_sequence = PointSequence::<Field>::default();
        for (x, y) in coordinates {
            point_sequence.push(Point::new(U64::from_u32(x), ModularNumber::from_u32(y)));
        }
        let result = lagrange_polynomial(&point_sequence).unwrap();
        let expect = make_polynomial(&[0, 0, 1]);
        assert_eq!(result, expect);
    }

    fn to_modular<T: Modular>(values: Vec<u64>) -> Vec<ModularNumber<T>> {
        values.into_iter().map(|x: u64| ModularNumber::from_u64(x)).collect()
    }

    /// Lagrange benchmarking.
    pub fn lagrange_bench(m: usize) {
        // Parameters.
        type Field = PrimeField<Fft64>;
        let polynomial_degree = 32u64;
        let n = 100u64;
        let abscissas = to_modular::<Fft64>((1..=n).collect());

        // Setup.
        let xs = to_modular((1..=(polynomial_degree + 1)).collect());
        let lagrange = Lagrange::new(xs).unwrap();

        let mut secrets = Vec::new();
        let mut results = Vec::new();
        for _ in 0..m {
            let secret = ModularNumber::gen_random();
            secrets.push(secret.clone());

            // Create polynomial.
            let mut polynomial = Polynomial::<Field>::new(Vec::new());
            polynomial.add_coefficient(secret);
            for _ in 0..polynomial_degree {
                let coefficient = Field::gen_random_element(&mut rand::thread_rng());
                polynomial.add_coefficient(coefficient);
            }

            // Evaluate polynomial.
            let mut values = Vec::new();
            let mut point_sequence = PointSequence::<Field>::default();
            for x in &abscissas {
                let y = polynomial.eval(x).unwrap();
                point_sequence.push(Point::new(x.into_value(), y));
                values.push(y);
            }

            // Interpolate polynomial.
            let points = point_sequence.take(polynomial_degree + 1).unwrap();
            let result = lagrange.interpolate(&points).unwrap();
            results.push(result);
        }
        assert_eq!(secrets, results, "Secret recovery failed!");
    }

    #[test]
    fn bench() {
        lagrange_bench(10);
    }
}
