//! Polynomial in Finite Field.

use crate::{errors::PolynomialError, fields::Field};

/// Polynomial Expression.
#[derive(Debug, Clone, PartialEq)]
pub struct Polynomial<F>
where
    F: Field,
{
    /// Coefficients of the polynomial.
    coefficients: Vec<F::Element>,
}

impl<F: Field> Polynomial<F> {
    /// Creates a new polynomial expression.
    pub fn new(coefficients: Vec<F::Element>) -> Polynomial<F> {
        Polynomial { coefficients }
    }

    /// Add a coefficient to the polynomial.
    pub fn add_coefficient(&mut self, coefficient: F::Element) {
        self.coefficients.push(coefficient);
    }

    /// Remove leading zeros.
    pub fn canonicalize(&mut self) -> Result<(), PolynomialError> {
        while (!self.coefficients.is_empty()) && (*self.last_coefficient()? == F::ZERO) {
            self.coefficients.pop();
        }
        Ok(())
    }

    /// Check if polynomial is empty.
    pub fn is_empty(&self) -> bool {
        self.coefficients.is_empty()
    }

    /// Get coefficients.
    pub fn coefficients(&self) -> &Vec<F::Element> {
        &self.coefficients
    }

    /// Get the degree of the polynomial.
    pub fn degree(&self) -> Result<usize, PolynomialError> {
        if self.coefficients.is_empty() {
            return Ok(0);
        }
        self.coefficients.len().checked_sub(1).ok_or(PolynomialError::IntegerOverflow)
    }

    /// Evaluates the polynomial at a given x using Horner's method.
    pub fn eval_at(&self, x: &F::Inner) -> Result<F::Element, PolynomialError> {
        let mut eval = F::ZERO;
        let x = F::as_element(*x);
        for coefficient in self.coefficients.iter().rev() {
            eval = eval * &x + coefficient;
        }
        Ok(eval)
    }

    /// Evaluates the polynomial at a given x using Horner's method.
    pub fn eval(&self, x: &F::Element) -> Result<F::Element, PolynomialError> {
        let mut eval = F::ZERO;
        for coefficient in self.coefficients.iter().rev() {
            eval = eval * x + coefficient;
        }
        Ok(eval)
    }

    /// Get coefficient at index.
    pub fn get_coefficient(&self, idx: usize) -> Result<&F::Element, PolynomialError> {
        return self.coefficients.get(idx).ok_or(PolynomialError::CoefficientNotFound);
    }

    /// Get mutable coefficient at index.
    pub fn get_coefficient_mut(&mut self, idx: usize) -> Result<&mut F::Element, PolynomialError> {
        return self.coefficients.get_mut(idx).ok_or(PolynomialError::CoefficientNotFound);
    }

    /// Get the last coefficient.
    pub fn last_coefficient(&self) -> Result<&F::Element, PolynomialError> {
        return self.coefficients.last().ok_or(PolynomialError::CoefficientNotFound);
    }

    /// Encode the polynomial into a vector of encoded coefficients
    pub fn encode(&self) -> Vec<F::EncodedElement> {
        F::encode(self.coefficients.iter())
    }

    /// Decode a polynomial from a vector of encoded coefficients
    pub fn try_decode(encoded: Vec<F::EncodedElement>) -> Result<Self, F::DecodeError> {
        Ok(Self { coefficients: F::try_decode(encoded.iter())? })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        fields::PrimeField,
        modular::{ModularNumber, Prime},
        test_prime,
    };
    use crypto_bigint::U64;

    test_prime!(P11, 11u64);

    fn make_polynomial<T: Prime>(coefficients: &[i32]) -> Polynomial<PrimeField<T>> {
        let coefs = coefficients.into_iter().map(|c| ModularNumber::from_u32(*c as u32)).collect();
        Polynomial::new(coefs)
    }

    #[test]
    fn test_evaluator() {
        let polynomial = make_polynomial::<P11>(&[10, 2, 3]);
        let result = polynomial.eval_at(&U64::from_u32(2)).unwrap();
        assert_eq!(result, ModularNumber::from_u32(4));
    }

    #[test]
    fn test_encode_decode() {
        /// We need to check on a prime field, prime test numbers can't be encoded
        type Prime = crate::modular::U64SafePrime;
        let polynomial = make_polynomial::<Prime>(&[10, 2, 3]);
        let encoded = polynomial.encode();
        let decoded = Polynomial::<PrimeField<Prime>>::try_decode(encoded).unwrap();
        assert_eq!(polynomial, decoded);

        let result_a = polynomial.eval_at(&U64::from_u32(2)).unwrap();
        let result_b = decoded.eval_at(&U64::from_u32(2)).unwrap();

        assert_eq!(result_a, result_b);
    }
}
