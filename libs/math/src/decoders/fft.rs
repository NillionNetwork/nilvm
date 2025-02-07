//! Prime Field Fast Fourier Transform.

use crate::{
    errors::DivByZero,
    modular::{ModularInverse, ModularNumber, ModularPow, Prime},
};

/// 2-radix FFT using root of unity w.
pub fn fft2<T: Prime>(values: Vec<ModularNumber<T>>, w: &ModularNumber<T>) -> Result<Vec<ModularNumber<T>>, FFTError> {
    // TODO: debug why are the abscissas not aligned?
    let values = fft2_rearrange(values)?;
    let values = fft2_compute(values, w)?;
    Ok(values)
}

/// 2-radix Inverse FFT using root of unity w.
pub fn fft2_inverse<T: Prime>(
    values: Vec<ModularNumber<T>>,
    w: &ModularNumber<T>,
) -> Result<Vec<ModularNumber<T>>, FFTError> {
    let w_inv = w.inverse();
    let l = ModularNumber::from_u64(values.len() as u64);
    let l_inv = l.inverse();
    let mut values = fft2(values, &w_inv)?;
    for x in values.iter_mut() {
        *x = *x * &l_inv;
    }
    Ok(values)
}

/// Rearrange input values for FFT.
fn fft2_rearrange<T: Prime>(mut values: Vec<ModularNumber<T>>) -> Result<Vec<ModularNumber<T>>, FFTError> {
    let mut target = 0;
    for pos in 0..values.len() {
        if target > pos {
            values.swap(target, pos)
        }
        let mut mask = values.len().checked_shr(1).ok_or(FFTError::IntegerArithmetic)?;
        while target & mask != 0 {
            target &= !mask;
            mask = mask.checked_shr(1).ok_or(FFTError::IntegerArithmetic)?;
        }
        target |= mask;
    }
    Ok(values)
}

/// Compute FFT in place.
fn fft2_compute<T: Prime>(
    mut values: Vec<ModularNumber<T>>,
    w: &ModularNumber<T>,
) -> Result<Vec<ModularNumber<T>>, FFTError> {
    let mut depth = 0u32;
    loop {
        let step = 1usize.checked_shl(depth).ok_or(FFTError::IntegerArithmetic)?;
        if step >= values.len() {
            break;
        }
        let jump = step.checked_mul(2).ok_or(FFTError::IntegerArithmetic)?;
        let exp = values.len().checked_div(jump).ok_or(FFTError::IntegerArithmetic)?;
        let factor_stride = w.exp_mod(&T::Normal::from(exp as u64));
        let mut factor = ModularNumber::ONE;
        for group in 0..step {
            let mut pair = group;
            while pair < values.len() {
                let pair_step = pair.checked_add(step).ok_or(FFTError::IntegerArithmetic)?;
                let x = *values.get(pair).ok_or(FFTError::IndexNotFound)?;
                let z = values.get_mut(pair_step).ok_or(FFTError::IndexNotFound)?;
                let y = *z * &factor;
                *z = x - &y;

                let x = values.get_mut(pair).ok_or(FFTError::IndexNotFound)?;
                *x = *x + &y;

                pair = pair.checked_add(jump).ok_or(FFTError::IntegerArithmetic)?;
            }
            factor = factor * &factor_stride;
        }
        depth = depth.checked_add(1).ok_or(FFTError::IntegerArithmetic)?;
    }
    Ok(values)
}

/// Fast Fourier Transform Error.
#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum FFTError {
    /// Operation error.
    #[error("operation error")]
    OperationError(#[from] DivByZero),

    /// Integer arithmetic error.
    #[error("integer arithmetic error")]
    IntegerArithmetic,

    /// Index not found error.
    #[error("index not found")]
    IndexNotFound,
}

#[cfg(any(test, feature = "bench"))]
#[allow(dead_code, clippy::arithmetic_side_effects, clippy::indexing_slicing, clippy::unwrap_used)]
pub mod fft_test {
    //! FFT tests.

    use super::*;
    use crate::{
        fields::{Field, PrimeField},
        modular::UintModulo,
        polynomial::Polynomial,
        test_prime,
    };

    test_prime!(P433, 433u64);
    test_prime!(P354, 354u64);
    test_prime!(Fft64, 18446744072637906947u64);
    test_prime!(OmegaFft, 4264170360572300374u64);
    test_prime!(P5038849, 5038849u64);

    fn to_modular<T: Prime>(values: Vec<i32>) -> Vec<ModularNumber<T>> {
        values.into_iter().map(|x: i32| ModularNumber::new(T::Normal::from(x as u64))).collect()
    }

    #[test]
    fn test_fft2() {
        // Field is Z_433 in which 354 is an 8th root of unity.
        let w = ModularNumber::<P433>::from_u32(354);

        let values = to_modular(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        let result = fft2(values, &w).unwrap();
        let exp_vec = vec![36, 303, 146, 3, 429, 422, 279, 122];
        assert_eq!(result, to_modular(exp_vec));
    }

    #[test]
    fn test_fft2_inverse() {
        // Field is Z_433 in which 354 is an 8th root of unity.
        let w = ModularNumber::<P433>::from_u32(354);

        let values = to_modular(vec![36, 303, 146, 3, 429, 422, 279, 122]);
        let result = fft2_inverse(values, &w).unwrap();
        let expected = vec![1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(result, to_modular(expected));
    }

    #[test]
    fn test_fft2_big() {
        let w = ModularNumber::<P5038849>::from_u32(4318906);

        let values = to_modular((1234000..1234256).collect());
        let forward = fft2(values, &w).unwrap();
        let result = fft2_inverse(forward, &w).unwrap();
        let expected: Vec<i32> = (1234000..1234256).collect();
        assert_eq!(result, to_modular(expected));
    }

    /// FFT benchmarking.
    pub fn fft_bench(m: usize) {
        // Parameters.
        type Field = PrimeField<Fft64>;
        let w = Field::as_element(OmegaFft::MODULO);
        let polynomial_degree = 32u64;
        let n = 128u64;

        // Setup.
        let mut secrets = Vec::new();
        let mut results = Vec::new();
        for _ in 0..m {
            let secret = ModularNumber::<Fft64>::gen_random();
            secrets.push(secret.clone());

            // Create polynomial.
            let mut polynomial = Polynomial::<Field>::new(Vec::new());
            polynomial.add_coefficient(secret);
            for _ in 0..polynomial_degree {
                let coefficient = Field::gen_random_element(&mut rand::thread_rng());
                polynomial.add_coefficient(coefficient);
            }
            // Add extra zeros for FFT.
            for _ in (polynomial_degree + 1)..n {
                polynomial.add_coefficient(PrimeField::<Fft64>::ZERO);
            }
            let values = polynomial.coefficients().clone();

            // Evaluate polynomial.
            let values = fft2(values, &w).unwrap();

            // Interpolate polynomial.
            let result = fft2_inverse(values, &w).unwrap().get(0).unwrap().clone();
            results.push(result);
        }
        assert_eq!(secrets, results, "Secret recovery failed!");
    }

    #[test]
    fn test_bench() {
        fft_bench(10);
    }
}
