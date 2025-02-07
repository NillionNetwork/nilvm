//! `Polynomial<F>` Operations

use crate::{
    errors::PolynomialError,
    fields::{Field, Inv},
    polynomial::Polynomial,
};
use std::ops::{Add, Div, Mul, Neg, Sub};

impl<F: Field> Sub<&Polynomial<F>> for Polynomial<F> {
    type Output = Polynomial<F>;

    fn sub(self, other: &Self) -> Polynomial<F> {
        let mut coefficients: Vec<F::Element> = self.coefficients().clone();

        let mut other_iter = other.coefficients().iter();
        for (coef, other_coef) in coefficients.iter_mut().zip(other_iter.by_ref()) {
            *coef = *coef - other_coef;
        }
        for remaining_coef in other_iter {
            coefficients.push(-*remaining_coef);
        }

        Polynomial::<F>::new(coefficients)
    }
}

impl<F: Field> Add<&Polynomial<F>> for Polynomial<F> {
    type Output = Polynomial<F>;

    fn add(self, other: &Self) -> Polynomial<F> {
        if self.coefficients().is_empty() {
            return Polynomial::new(other.coefficients().clone());
        }
        let mut coefficients: Vec<F::Element> = self.coefficients().clone();

        let mut other_iter = other.coefficients().iter();
        for (coef, other_coef) in coefficients.iter_mut().zip(other_iter.by_ref()) {
            *coef = *coef + other_coef;
        }
        for remaining_coef in other_iter {
            coefficients.push(*remaining_coef);
        }

        Polynomial::<F>::new(coefficients)
    }
}

impl<F: Field> Neg for &Polynomial<F> {
    type Output = Polynomial<F>;

    fn neg(self) -> Self::Output {
        let mut coefficients: Vec<F::Element> = self.coefficients().clone();
        for c in self.coefficients().iter() {
            coefficients.push(-*c);
        }
        Polynomial::<F>::new(coefficients)
    }
}

impl<F: Field> Mul<&Polynomial<F>> for Polynomial<F> {
    type Output = Result<Polynomial<F>, PolynomialError>;

    fn mul(self, other: &Self) -> Result<Polynomial<F>, PolynomialError> {
        if self.coefficients().is_empty() || other.coefficients().is_empty() {
            return Ok(Polynomial::new(Vec::new()));
        }
        let len = self.coefficients().len().checked_add(other.degree()?).ok_or(PolynomialError::IntegerOverflow)?;
        let mut product = Polynomial::new(vec![F::ZERO; len]);
        for (i, c) in self.coefficients().iter().enumerate() {
            for (j, o) in other.coefficients().iter().enumerate() {
                let k = i.checked_add(j).ok_or(PolynomialError::IntegerOverflow)?;
                let coef_k = product.get_coefficient_mut(k)?;
                *coef_k = *coef_k + &(*c * o);
            }
        }
        Ok(product)
    }
}

impl<F: Field> Div<&Polynomial<F>> for Polynomial<F> {
    type Output = Result<(Polynomial<F>, Polynomial<F>), PolynomialError>;

    fn div(self, other: &Self) -> Result<(Polynomial<F>, Polynomial<F>), PolynomialError> {
        let mut dividend = self;
        dividend.canonicalize()?;

        let mut divisor = other.clone();
        divisor.canonicalize()?;
        if divisor.is_empty() {
            return Err(PolynomialError::DivByZero);
        }
        let divisor_degree = divisor.degree()?;
        let divisor_lead = divisor.last_coefficient()?.inv()?;

        let mut remainder = Polynomial::<F>::new(dividend.coefficients().clone());
        let mut quotient = Polynomial::<F>::new(vec![F::ZERO; remainder.coefficients().len()]);

        let n = remainder.coefficients().len().checked_sub(divisor_degree).ok_or(PolynomialError::IntegerOverflow)?;
        for i in (0..n).rev() {
            let idx = i.checked_add(divisor_degree).ok_or(PolynomialError::IntegerOverflow)?;
            let coef = remainder.get_coefficient(idx)?;
            let monomial_lead = *coef * &divisor_lead;
            let quotient_i = quotient.get_coefficient_mut(i)?;
            *quotient_i = monomial_lead;
            for j in 0..=divisor_degree {
                let ij = i.checked_add(j).ok_or(PolynomialError::IntegerOverflow)?;
                let r_ij = remainder.get_coefficient_mut(ij)?;
                let divisor_j = divisor.get_coefficient(j)?;
                *r_ij = *r_ij - &(monomial_lead * divisor_j);
            }
        }
        quotient.canonicalize()?;
        remainder.canonicalize()?;
        Ok((quotient, remainder))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub mod test {
    use super::*;
    use crate::{
        fields::PrimeField,
        modular::{ModularNumber, Prime},
        test_prime,
    };

    test_prime!(P11, 11u64);

    fn make_polynomial<T: Prime>(coefficients: &[i32]) -> Polynomial<PrimeField<T>> {
        let coefs = coefficients.into_iter().map(|c| ModularNumber::from_u32(*c as u32)).collect();
        Polynomial::new(coefs)
    }

    #[test]
    fn test_poly_add() {
        let polynomial1 = make_polynomial::<P11>(&[2, 3, 10]);
        let polynomial2 = make_polynomial(&[2, 4]);
        let result = polynomial1.clone() + &polynomial2;
        let expected = make_polynomial(&[4, 7, 10]);
        assert_eq!(result.coefficients(), expected.coefficients());
        let result2 = polynomial2 + &polynomial1;
        assert_eq!(result2.coefficients(), expected.coefficients());
    }

    #[test]
    fn test_poly_sub() {
        let polynomial1 = make_polynomial::<P11>(&[2, 3, 10]);
        let polynomial2 = make_polynomial(&[2, 4]);
        let result = polynomial1.clone() - &polynomial2;
        let expected = make_polynomial(&[0, 10, 10]);
        assert_eq!(result.coefficients(), expected.coefficients());
        let result2 = polynomial2 - &polynomial1;
        let expected2 = make_polynomial(&[0, 1, 1]);
        assert_eq!(result2.coefficients(), expected2.coefficients());
    }

    #[test]
    fn test_poly_mul() {
        let polynomial1 = make_polynomial::<P11>(&[2, 3, 10]);
        let polynomial2 = make_polynomial(&[2, 4]);
        let result = (polynomial1.clone() * &polynomial2).unwrap();
        let expected = make_polynomial(&[4, 3, 10, 7]);
        assert_eq!(result.coefficients(), expected.coefficients());
        let result2 = (polynomial2 * &polynomial1).unwrap();
        assert_eq!(result2.coefficients(), expected.coefficients());
    }

    #[test]
    fn test_poly_div() {
        let dividend = make_polynomial::<P11>(&[2, 3, 10]);
        let divisor = make_polynomial(&[4, 3]);
        let (quotient, remainder) = (dividend / &divisor).unwrap();
        let expected_quotient = make_polynomial(&[10, 7]);
        assert_eq!(expected_quotient.coefficients(), quotient.coefficients());
        let expected_remainder = make_polynomial(&[6]);
        assert_eq!(expected_remainder.coefficients(), remainder.coefficients());
    }

    #[test]
    fn test_poly_div_by_zero() {
        let dividend = make_polynomial::<P11>(&[2, 3, 10]);
        let divisor = make_polynomial(&[0]);
        let result = dividend / &divisor;
        let expected: Result<(), PolynomialError> = Err(PolynomialError::DivByZero);
        assert_eq!(result.err(), expected.err());
    }

    #[test]
    fn test_poly_div_by_scalar() {
        let dividend = make_polynomial::<P11>(&[2, 3, 7]);
        let divisor = make_polynomial(&[2]);
        let (quotient, remainder) = (dividend / &divisor).unwrap();
        let expected_quotient = make_polynomial(&[12, 7, 9]);
        assert_eq!(quotient.coefficients(), expected_quotient.coefficients());
        let expected_remainder = make_polynomial(&[]);
        assert_eq!(remainder.coefficients(), expected_remainder.coefficients());
    }
}
