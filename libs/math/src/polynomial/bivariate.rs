//! Bivariate Polynomial in Finite Field.

use crate::{errors::PolynomialError, fields::Field, polynomial::Polynomial};

/// Bivariate Expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bivariate<F: Field> {
    /// Field of the bivariate.
    field: F,

    /// Coefficients of the bivariate.
    coefficients: Vec<Vec<F::Element>>,
}

impl<F: Field> Bivariate<F> {
    /// Creates a new bivariate expression.
    pub fn new(field: F, coefficients: Vec<Vec<F::Element>>) -> Bivariate<F> {
        Bivariate { field, coefficients }
    }

    /// Check if bivariate is empty.
    pub fn is_empty(&self) -> bool {
        self.coefficients.is_empty()
    }

    /// Get the field of the bivariate.
    pub fn field(&self) -> &F {
        &self.field
    }

    /// Get coefficients.
    pub fn coefficients(&self) -> &Vec<Vec<F::Element>> {
        &self.coefficients
    }

    /// Get coefficient at index.
    pub fn get_coefficient(&self, x_idx: usize, y_idx: usize) -> Result<&F::Element, PolynomialError> {
        self.coefficients
            .get(y_idx)
            .ok_or(PolynomialError::CoefficientNotFound)?
            .get(x_idx)
            .ok_or(PolynomialError::CoefficientNotFound)
    }

    /// Get the x degree of the bivariate.
    pub fn degree_x(&self) -> Result<usize, PolynomialError> {
        if self.coefficients.is_empty() {
            return Ok(0);
        }
        let mut max_degree = 0;
        for row in &self.coefficients {
            let degree = row.len().checked_sub(1).ok_or(PolynomialError::IntegerOverflow)?;
            if degree > max_degree {
                max_degree = degree;
            }
        }
        Ok(max_degree)
    }

    /// Get the y degree of the bivariate.
    pub fn degree_y(&self) -> Result<usize, PolynomialError> {
        if self.coefficients.is_empty() {
            return Ok(0);
        }
        self.coefficients.len().checked_sub(1).ok_or(PolynomialError::IntegerOverflow)
    }

    /// Get the polynomial when evaluated at x.
    pub fn reduce_x(&self, x: F::Element) -> Result<Polynomial<F>, PolynomialError> {
        let mut coefs = Vec::new();
        for row in &self.coefficients {
            let mut eval = F::ZERO;
            for coefficient in row.iter().rev() {
                eval = eval * &x + coefficient;
            }
            coefs.push(eval);
        }
        Ok(Polynomial::new(coefs))
    }

    /// Get the polynomial when evaluated at y.
    pub fn reduce_y(&self, y: F::Element) -> Result<Polynomial<F>, PolynomialError> {
        let mut coefs = Vec::new();
        for _ in 0..=self.degree_y()? {
            coefs.push(F::ZERO);
        }
        for row in self.coefficients.iter().rev() {
            for (i, coefficient) in row.iter().enumerate() {
                let eval = coefs.get_mut(i).ok_or(PolynomialError::CoefficientNotFound)?;
                *eval = *eval * &y + coefficient;
            }
        }
        Ok(Polynomial::new(coefs))
    }
}

#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        fields::PrimeField,
        modular::{ModularNumber, Prime},
        test_prime,
    };

    test_prime!(P13, 13u64);

    fn make_polynomial<T: Prime>(coefficients: &[i32]) -> Polynomial<PrimeField<T>> {
        let coefs = coefficients.into_iter().map(|c| ModularNumber::from_u32(*c as u32)).collect();
        Polynomial::new(coefs)
    }

    fn make_bivariate<T: Prime>(n: usize, values: &[i32]) -> Bivariate<PrimeField<T>> {
        let mut bivariate = Vec::new();
        for i in 0..n {
            bivariate.push(Vec::new());
            for j in 0..n {
                let num = ModularNumber::from_u32(values[i * n + j] as u32);
                bivariate[i].push(num);
            }
        }
        Bivariate::new(PrimeField::default(), bivariate)
    }

    #[test]
    fn test_reduce_x() {
        let bivariate = make_bivariate::<P13>(3, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let x = ModularNumber::two();
        let result_poly = bivariate.reduce_x(x).unwrap();
        // 1+2*2+3*2^2=1+4+12=4, 4+5*2+6*2^2=4+10+11=12, 7+8*2+9*4=7+3+10=7
        let coefs = vec![4, 12, 7];
        let expected_poly = make_polynomial(&coefs);
        assert_eq!(result_poly, expected_poly);
    }

    #[test]
    fn test_reduce_y() {
        let bivariate = make_bivariate::<P13>(3, &[1, 2, 3, 4, 5, 6, 7, 8, 9]);
        let y = ModularNumber::two();
        let result_poly = bivariate.reduce_y(y).unwrap();
        // 1+4*2+7*2^2=1+8+2=11, 2+5*2+8*2^2=2+10+6=5, 3+6*2+9*2^2=3+12+10=12
        let coefs = vec![11, 5, 12];
        let expected_poly = make_polynomial(&coefs);
        assert_eq!(result_poly, expected_poly);
    }
}
