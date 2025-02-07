//! Gao decoder.

use crate::{
    decoders::lagrange_polynomial,
    errors::{InterpolationError, PolynomialError},
    fields::Field,
    polynomial::{point_sequence::PointSequence, Polynomial},
};
use thiserror::Error;

/// Decode polynomial from Point Sequence using Gao error correction.
pub fn gao_decode<F: Field>(
    sequence: &PointSequence<F>,
    degree: usize,
    max_error: usize,
) -> Result<(Polynomial<F>, Polynomial<F>), ECCError> {
    if sequence.is_empty() {
        return Err(ECCError::EmptySequence);
    }
    if sequence.has_duplicates() {
        return Err(ECCError::HasDuplicates);
    }
    let max_degree = degree.checked_add(1).ok_or(ECCError::IntegerOverflow)?;
    let min_rem_degree = max_degree.checked_add(max_error).ok_or(ECCError::IntegerOverflow)?;
    let min_sequence_len = min_rem_degree.checked_add(max_error).ok_or(ECCError::IntegerOverflow)?;
    if sequence.points().len() < min_sequence_len {
        return Err(ECCError::Unrecoverable);
    }

    let faulty_poly = lagrange_polynomial(sequence)?;

    let mut encode_poly = F::polynomial(vec![F::ONE]);
    for pi in sequence.points().iter() {
        let xi = F::as_element(pi.x);
        let poly_i = F::polynomial(vec![-xi, F::ONE]);
        encode_poly = (encode_poly * &poly_i)?;
    }

    let mut r0 = encode_poly;
    let mut r1 = faulty_poly;
    let mut s0 = F::polynomial(vec![F::ONE]);
    let mut s1 = F::polynomial(Vec::new());
    let mut t0 = F::polynomial(Vec::new());
    let mut t1 = F::polynomial(vec![F::ONE]);

    loop {
        let (q, r2) = (r0.clone() / &r1)?;

        if r0.degree()? < min_rem_degree {
            let (g, leftover) = (r0 / &t0)?;

            if leftover.is_empty() {
                let decoded_poly = g;
                let error_locator = t0;
                return Ok((decoded_poly, error_locator));
            } else {
                return Err(ECCError::Unrecoverable);
            };
        }
        let s1_old = s1.clone();
        let t1_old = t1.clone();
        s1 = s0 - &(s1 * &q)?;
        t1 = t0 - &(t1 * &q)?;
        r0 = r1;
        s0 = s1_old;
        t0 = t1_old;
        r1 = r2;
    }
}

/// ECC Error.
#[derive(Debug, Eq, Error, PartialEq)]
pub enum ECCError {
    /// Unrecoverable from sequence, too many errors.
    #[error("unrecoverable: too many errors to recover")]
    Unrecoverable,

    /// Empty sequence error.
    #[error("empty sequence")]
    EmptySequence,

    /// Has duplicate abscissas.
    #[error("has duplicate abscissas")]
    HasDuplicates,

    /// Integer arithmetic error.
    #[error("integer overflow")]
    IntegerOverflow,

    /// Failed interpolation.
    #[error("interpolation failed: {0}")]
    FailedInterpolation(#[from] InterpolationError),

    /// Polynomial Error.
    #[error("polynomial operation error: {0}")]
    PolyError(#[from] PolynomialError),
}

#[cfg(test)]
mod test {
    use crypto_bigint::U64;

    use super::*;
    use crate::{
        fields::PrimeField,
        modular::{ModularNumber, Prime},
        polynomial::point::Point,
        test_prime,
    };

    test_prime!(P433, 433u64);

    fn make_polynomial<T: Prime>(coefficients: &[i32]) -> Polynomial<PrimeField<T>> {
        let coefs = coefficients.into_iter().map(|c: &i32| ModularNumber::from_u32(*c as u32)).collect();
        Polynomial::new(coefs)
    }

    #[test]
    fn test_gao_decode() {
        type Field = PrimeField<P433>;
        // y = 5 + 68x, first coordinate corrupted
        let coordinates: Vec<(u32, u32)> = vec![(1, 130), (2, 141), (3, 209), (4, 277)];
        let mut point_sequence = PointSequence::<Field>::default();
        for (x, y) in coordinates {
            point_sequence.push(Point::new(U64::from_u32(x), ModularNumber::from_u32(y)));
        }
        let out = gao_decode(&point_sequence, 1, 1);
        let (decoded_poly, error_locator) = out.unwrap();
        let expected_poly = make_polynomial(&[5, 68]);
        assert_eq!(decoded_poly, expected_poly);
        let expected_error_locator = make_polynomial(&[205, 228]);
        assert_eq!(error_locator, expected_error_locator);
    }

    #[test]
    fn test_gao_decode_unrecoverable() {
        type Field = PrimeField<P433>;
        // y = 5 + 68x, first two coordinates corrupted
        let coordinates: Vec<(u32, u32)> = vec![(1, 60), (2, 253), (3, 209), (4, 277)];
        let mut point_sequence = PointSequence::<Field>::default();
        for (x, y) in coordinates {
            point_sequence.push(Point::new(x.into(), ModularNumber::from_u32(y)));
        }
        let out = gao_decode(&point_sequence, 1, 1);
        let expected: Result<(), ECCError> = Err(ECCError::Unrecoverable);
        assert_eq!(out.err(), expected.err());
    }
}
