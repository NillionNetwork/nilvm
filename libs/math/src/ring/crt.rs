//! Implementation of the Chinese Remainder Theorem.

use crate::{
    modular::{ModularNumber, SophiePrime},
    ring::RingTuple,
};
use crypto_bigint::Integer;

/// Performs the Chinese Remainder Theorem to reconstruct the secret from its parts in each field.
pub fn crt<T>(ring_tuple: RingTuple<T>) -> ModularNumber<T::SemiPrime>
where
    T: SophiePrime,
{
    let (prime_element, binary_ext_element) = ring_tuple.into_parts();

    let prime_element = prime_element.into_value();
    let is_binary_ext_odd = binary_ext_element.value() & 1 == 1;
    let is_prime_odd = prime_element.is_odd().unwrap_u8() == 1;

    let q = ModularNumber::<T::SemiPrime>::new(T::MODULO);
    let a_q = ModularNumber::<T::SemiPrime>::new(prime_element);

    // TODO: try to use `crypto_bigint::Choice` here.
    let left_term = if is_binary_ext_odd { q } else { ModularNumber::ZERO };
    let q = if is_prime_odd { q } else { ModularNumber::ZERO };
    let right_term = q + &a_q;
    left_term + &right_term
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{galois::GF256, test_safe_prime};
    use crypto_bigint::U64;

    test_safe_prime!(P23, P11, P22, U64, U64::from_u32(23), U64::from_u32(2));
    // Note: this not a prime but the sophie germaine prime ends up being the 7th mersenne prime
    // as that's what was being used in this test previously.
    test_safe_prime!(Prime9th, Prime9thSophie, Prime9thSemi, U64, U64::from_u32(1048575), U64::from_u32(2));

    #[test]
    fn crt_simple() {
        // Decompose the input (15) into a part mod 11 and one mod 2^8. The latter just has to have the LSB set,
        // anything else is random.
        let ring_tuple = RingTuple::new(ModularNumber::<P11>::from_u32(4), GF256::new(241));
        assert_eq!(crt(ring_tuple), ModularNumber::<P22>::from_u32(15));
    }

    #[test]
    fn crt_complex() {
        // Input is 910292.
        let ring_tuple = RingTuple::new(ModularNumber::<Prime9thSophie>::from_u32(386005), GF256::new(68));
        assert_eq!(crt(ring_tuple), ModularNumber::<Prime9thSemi>::from_u32(910292));
    }
}
