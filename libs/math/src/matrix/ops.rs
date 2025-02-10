//! Matrix Operations

use crate::{
    fields::{Field, Inv},
    matrix::{Matrix, MatrixError},
};
use std::ops::Mul;

impl<F: Field> Mul<&Matrix<F>> for Matrix<F> {
    type Output = Result<Matrix<F>, MatrixError>;

    /// Naive matrix multiplication, A: MxK * B: KxN -> C: KxN, O(KMN).
    fn mul(self, other: &Matrix<F>) -> Result<Matrix<F>, MatrixError> {
        if self.ncols() != other.nrows() {
            return Err(MatrixError::Arithmetic);
        }
        let mut out = Matrix::<F>::zero(self.nrows(), other.ncols());
        for row in 0..self.nrows() {
            for col in 0..other.ncols() {
                let oi = out.entry_mut(row, col)?;
                for i in 0..self.ncols() {
                    let li = self.entry(row, i)?;
                    let ri = other.entry(i, col)?;
                    *oi = *oi + &(*ri * li);
                }
            }
        }
        Ok(out)
    }
}

impl<F: Field> Inv for Matrix<F> {
    type Output = Result<Matrix<F>, MatrixError>;

    /// Inverse of the matrix using Guassian elimination, consumes the matrix, O(N^3).
    fn inv(mut self) -> Result<Matrix<F>, MatrixError> {
        let n = self.nrows();
        if n != self.ncols() {
            return Err(MatrixError::Singular);
        }
        let mut m = Matrix::<F>::identity(n)?;

        for i in 0..n {
            let v_ii = *self.entry(i, i)?;
            if v_ii == F::ZERO {
                for j in i + 1..n {
                    let v_ji = *self.entry(j, i)?;
                    if v_ji != F::ZERO {
                        for k in 0..n {
                            let v_jk = *self.entry(j, k)?;
                            let v_ik = self.entry_mut(i, k)?;
                            let temp = *v_ik;
                            *v_ik = v_jk;
                            let v_jk = self.entry_mut(j, k)?;
                            *v_jk = temp;
                            let m_jk = *m.entry(j, k)?;
                            let m_ik = m.entry_mut(i, k)?;
                            let temp = *m_ik;
                            *m_ik = m_jk;
                            let m_jk = m.entry_mut(j, k)?;
                            *m_jk = temp;
                        }
                        break;
                    }
                }
            }
            let v_ii = *self.entry(i, i)?;
            if v_ii != F::ONE {
                let v_ii_inv = v_ii.inv()?;
                for k in 0..n {
                    let v_ik = self.entry_mut(i, k)?;
                    *v_ik = *v_ik * &v_ii_inv;
                    let m_ik = m.entry_mut(i, k)?;
                    *m_ik = *m_ik * &v_ii_inv;
                }
            }
            for j in 0..n {
                if i == j {
                    continue;
                }
                let v_ji = *self.entry(j, i)?;
                if v_ji != F::ZERO {
                    for k in 0..n {
                        let v_ik = *self.entry(i, k)?;
                        let v_jk = self.entry_mut(j, k)?;
                        *v_jk = *v_jk - &(v_ik * &v_ji);
                        let m_ik = *m.entry(i, k)?;
                        let m_jk = m.entry_mut(j, k)?;
                        *m_jk = *m_jk - &(m_ik * &v_ji);
                    }
                }
            }
        }
        Ok(m)
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
    fn multiplication() {
        let left = make_matrix(3, &[1, 1, 1, 1, 2, 4, 1, 3, 9]);
        let right = make_matrix(3, &[3, 10, 1, 4, 4, 5, 7, 12, 7]);
        let result = (left * &right).unwrap();
        let expected = Matrix::<Field>::identity(3).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn inverse() {
        let matrix = make_matrix(3, &[1, 1, 1, 1, 2, 4, 1, 3, 9]);
        let result = matrix.inv().unwrap();
        let expected = make_matrix(3, &[3, 10, 1, 4, 4, 5, 7, 12, 7]);
        assert_eq!(result, expected);
    }

    #[test]
    fn inverse2() {
        let matrix = make_matrix(2, &[0, 1, 1, 0]);
        let result = matrix.inv().unwrap();
        let expected = make_matrix(2, &[0, 1, 1, 0]);
        assert_eq!(result, expected);
    }
}
