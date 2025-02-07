//! Matrix.

use crate::{
    errors::DivByZero,
    fields::{field::Inv, Field},
};
use thiserror::Error;

/// Matrix Expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Matrix<F: Field> {
    /// Matrix.
    data: Vec<F::Element>,

    /// Number of rows.
    nrows: u16,

    /// Number of columns.
    ncols: u16,
}

impl<F: Field> Matrix<F> {
    /// New matrix.
    pub fn new(data: Vec<F::Element>, nrows: u16, ncols: u16) -> Result<Matrix<F>, MatrixError> {
        let n = usize::try_from(u32::from(nrows).checked_mul(u32::from(ncols)).ok_or(MatrixError::Arithmetic)?)
            .map_err(|_| MatrixError::Arithmetic)?;
        if n != data.len() {
            return Err(MatrixError::Build(data.len(), n));
        }
        Ok(Matrix { data, nrows, ncols })
    }

    /// Returns the reference to data.
    pub fn data(&self) -> &Vec<F::Element> {
        &self.data
    }

    /// Returns the data as a Vec consuming the matrix.
    pub fn to_vec(self) -> Vec<F::Element> {
        self.data
    }

    /// Number of rows.
    pub fn nrows(&self) -> u16 {
        self.nrows
    }

    /// Number of columns.
    pub fn ncols(&self) -> u16 {
        self.ncols
    }

    /// Get the matrix entry `M[row,col]`.
    pub fn entry(&self, row: u16, col: u16) -> Result<&F::Element, MatrixError> {
        let index = usize::try_from(
            u64::from(row)
                .checked_mul(u64::from(self.ncols))
                .ok_or(MatrixError::Arithmetic)?
                .checked_add(u64::from(col))
                .ok_or(MatrixError::Arithmetic)?,
        )
        .map_err(|_| MatrixError::Arithmetic)?;
        self.data.get(index).ok_or(MatrixError::IndexNotFound)
    }

    /// Get the matrix entry `M[row,col]`.
    pub fn entry_mut(&mut self, row: u16, col: u16) -> Result<&mut F::Element, MatrixError> {
        let index = usize::try_from(
            u64::from(row)
                .checked_mul(u64::from(self.ncols))
                .ok_or(MatrixError::Arithmetic)?
                .checked_add(u64::from(col))
                .ok_or(MatrixError::Arithmetic)?,
        )
        .map_err(|_| MatrixError::Arithmetic)?;
        self.data.get_mut(index).ok_or(MatrixError::IndexNotFound)
    }

    /// Zero matrix.
    pub fn zero(nrows: u16, ncols: u16) -> Matrix<F> {
        let data = vec![vec![F::ZERO; ncols.into()]; nrows.into()].into_iter().flatten().collect();
        Matrix { data, nrows, ncols }
    }

    /// One matrix.
    pub fn one(nrows: u16, ncols: u16) -> Matrix<F> {
        let data = vec![vec![F::ONE; ncols.into()]; nrows.into()].into_iter().flatten().collect();
        Matrix { data, nrows, ncols }
    }

    /// Identity matrix.
    pub fn identity(n: u16) -> Result<Matrix<F>, MatrixError> {
        let mut m = Matrix::zero(n, n);
        for i in 0..n {
            *m.entry_mut(i, i)? = F::ONE;
        }
        Ok(m)
    }

    /// Vandermonde matrix from abscissas.
    pub fn vandermonde(abscissas: &[F::Element], ncols: u16) -> Result<Matrix<F>, MatrixError> {
        let size = usize::from(ncols);
        if abscissas.len() < size {
            return Err(MatrixError::Vandermonde(size, abscissas.len()));
        }
        let mut v = Vec::new();
        for a in abscissas {
            let mut b = F::ONE;
            for _ in 0..size {
                v.push(b);
                b = b * a;
            }
        }
        let nrows = u16::try_from(abscissas.len()).map_err(|_| MatrixError::Arithmetic)?;
        let v = Matrix::new(v, nrows, ncols)?;
        Ok(v)
    }

    /// LU decompose the matrix using Guassian elimination, consumes the matrix, O(N^3).
    /// TODO: Check edge cases.
    pub fn lu_decompose(mut self) -> Result<(Matrix<F>, Matrix<F>), MatrixError> {
        let n = self.nrows();
        if n != self.ncols() {
            return Err(MatrixError::Singular);
        }
        let mut m = Matrix::identity(n)?;

        for i in 0..n - 1 {
            let v_ii_inv = self.entry(i, i)?.inv()?;
            for j in i + 1..n {
                let v_ji = *self.entry(j, i)?;
                let r = v_ji * &v_ii_inv;
                if r != F::ZERO {
                    for k in 0..n {
                        let v_ik = *self.entry(i, k)?;
                        let v_jk = self.entry_mut(j, k)?;
                        *v_jk = *v_jk - &(v_ik * &r);
                    }
                    let m_ji = m.entry_mut(j, i)?;
                    *m_ji = r;
                }
            }
        }
        Ok((m, self))
    }

    /// Matrix determinant using LU decompose, consumes the matrix.
    /// TODO: Add case where the matrix is not LU decomposable.
    pub fn determinant(self) -> Result<F::Element, MatrixError> {
        let (_, upper) = self.lu_decompose()?;
        let mut determinant = F::ONE;
        let n = upper.nrows();
        for i in 0..n {
            let v_ii = upper.entry(i, i)?;
            determinant = determinant * v_ii;
        }
        Ok(determinant)
    }
}

/// Matrix Error.
#[derive(Error, Debug, Eq, PartialEq)]
pub enum MatrixError {
    /// Operation error.
    #[error("operation error: {0}")]
    OperationError(#[from] DivByZero),

    /// Index not found error.
    #[error("index not found")]
    IndexNotFound,

    /// Integer overflow or underflow.
    #[error("interger overflow/underflow")]
    Arithmetic,

    /// Error building matrix.
    #[error("error building matrix, given data has {0} entries which does not match nrows x ncols = {1}")]
    Build(usize, usize),

    /// Error building vandermonde matrix.
    #[error("error building vandermonde matrix, given nrows {0} larger than number of abscissas {1}")]
    Vandermonde(usize, usize),

    /// Non-invertible, singular matrix.
    #[error("singular matrix can't be inverted")]
    Singular,
}

impl<F, F2> TryFrom<&Matrix<F>> for Matrix<F2>
where
    F: Field,
    F2: Field<Inner = F::Inner>,
{
    type Error = MatrixError;

    fn try_from(original: &Matrix<F>) -> Result<Self, Self::Error> {
        let mut data = Vec::new();
        for x in &original.data {
            data.push(F2::as_element(F::as_inner(*x)));
        }
        let lagrange = Matrix::new(data, original.nrows, original.ncols)?;
        Ok(lagrange)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{fields::PrimeField, modular::ModularNumber, test_prime};

    type Field = PrimeField<P13>;

    test_prime!(P13, 13u64);

    fn make_vector(values: &[u64]) -> Vec<ModularNumber<P13>> {
        values.into_iter().map(|val| ModularNumber::from_u64(*val)).collect()
    }

    fn make_matrix(n: usize, values: &[u64]) -> Matrix<Field> {
        Matrix::new(make_vector(values), n as u16, n as u16).unwrap()
    }

    #[test]
    fn identity() {
        let result = Matrix::<Field>::identity(3).unwrap();
        let expected = make_matrix(3, &[1, 0, 0, 0, 1, 0, 0, 0, 1]);
        assert_eq!(result, expected);
    }

    #[test]
    fn vandermonde() {
        let abscissas = make_vector(&[1, 2, 3]);
        let result = Matrix::vandermonde(&abscissas, 3).unwrap();
        let expected = make_matrix(3, &[1, 1, 1, 1, 2, 4, 1, 3, 9]);
        assert_eq!(result, expected);
    }

    #[test]
    fn lower_upper_decompose() {
        let matrix = make_matrix(3, &[1, 4, 10, 11, 8, 5, 3, 4, 7]);
        let (left, right) = matrix.clone().lu_decompose().unwrap();
        let expected_left = make_matrix(3, &[1, 0, 0, 11, 1, 0, 3, 6, 1]);
        let expected_right = make_matrix(3, &[1, 4, 10, 0, 3, 12, 0, 0, 9]);
        assert_eq!((left.clone(), right.clone()), (expected_left, expected_right));
        let result = (left * &right).unwrap();
        assert_eq!(result, matrix);
    }

    #[test]
    fn decompose_false() {
        let matrix = make_matrix(2, &[0, 1, 1, 0]);
        let result = matrix.clone().lu_decompose().err().unwrap();
        assert_eq!(result, MatrixError::OperationError(DivByZero));
    }

    #[test]
    fn determinant() {
        let matrix = make_matrix(3, &[1, 4, 10, 11, 8, 5, 3, 4, 7]);
        let result = matrix.determinant().unwrap();
        let expected = ModularNumber::ONE;
        assert_eq!(result, expected);
    }
}
