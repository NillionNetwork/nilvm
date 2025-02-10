//! Share that contains a bit value.

use math_lib::modular::{CheckedSub, DecodeError, EncodedModularNumber, Modular, ModularNumber};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

/// Represents a share where the secret behind it can only contain the value 0 or 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitShare<T: Modular>(ModularNumber<T>);

impl<T: Modular> BitShare<T> {
    /// Gets a reference to the underlying modular number.
    pub fn value(&self) -> &ModularNumber<T> {
        &self.0
    }

    /// This computes: a + (1 − 2a) · [b]
    pub fn xor_mask(&self, mask: bool) -> BitShare<T> {
        let bit_share = ModularNumber::from(self.clone());
        let left_term = ModularNumber::from_u32(mask as u32);
        let right_term = match mask {
            // SAFETY: We know modulo is greater than 0.
            true => T::MODULO.checked_sub(&T::Normal::from(1)).unwrap(),
            false => T::Normal::from(1),
        };
        let right_term = ModularNumber::<T>::new(right_term);
        let right_term = bit_share * &right_term;
        let result = left_term + &right_term;
        BitShare::from(result)
    }

    /// Encode this share.
    pub fn encode(&self) -> EncodedBitShare {
        EncodedBitShare::from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedBitShare) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

impl<T: Modular> From<ModularNumber<T>> for BitShare<T> {
    fn from(value: ModularNumber<T>) -> Self {
        Self(value)
    }
}

impl<T: Modular> From<BitShare<T>> for ModularNumber<T> {
    fn from(value: BitShare<T>) -> Self {
        value.0
    }
}

/// An encoded version of the LAMBDA output shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedBitShare {
    modular_number: EncodedModularNumber,
}

impl EncodedBitShare {
    /// Try to decode these shares.
    pub fn try_decode<T: Modular>(&self) -> Result<BitShare<T>, DecodeError> {
        BitShare::try_from(self)
    }
}

impl<T: Modular> From<&BitShare<T>> for EncodedBitShare {
    fn from(value: &BitShare<T>) -> Self {
        let modular_number = value.0.encode();
        Self { modular_number }
    }
}

impl<T: Modular> TryFrom<&EncodedBitShare> for BitShare<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedBitShare) -> Result<Self, Self::Error> {
        let modular_number = ModularNumber::try_from_encoded(&value.modular_number)?;
        Ok(Self(modular_number))
    }
}

/// An error for Bit Share Types.
#[derive(Debug, thiserror::Error)]
pub enum BitShareError {
    /// An integer overflow error.
    #[error("integer overflow")]
    IntegerOverflow,

    /// No elements found.
    #[error("no elements found")]
    NoElements,
}

/// The output of the RandomBoolean (RandomBit) protocol.
pub enum RandomBitStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output bit shares.
        shares: Vec<T>,
    },

    /// The protocol failed.
    ///
    /// This can happen if at least one of the random numbers generated by RAN was zero.
    Abort,
}

impl<T> Display for RandomBitStateOutput<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort => write!(f, "Random Boolean Aborted"),
        }
    }
}

impl<T: Modular> RandomBitStateOutput<BitShare<T>> {
    /// Encodes the shares in this output.
    pub fn encode(&self) -> RandomBitStateOutput<EncodedBitShare> {
        match self {
            Self::Success { shares } => {
                let shares: Vec<_> = shares.iter().map(BitShare::encode).collect();
                RandomBitStateOutput::Success { shares }
            }
            Self::Abort => RandomBitStateOutput::Abort,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use basic_types::PartyId;
    use math_lib::modular::U64SafePrime;
    use rstest::*;
    use shamir_sharing::{
        protocol::PolyDegree,
        secret_sharer::{PartyShares, SecretSharer, ShamirSecretSharer},
    };

    #[rstest]
    #[case(false, false, 0)]
    #[case(false, true, 1)]
    #[case(true, false, 1)]
    #[case(true, true, 0)]
    fn xoring(#[case] left: bool, #[case] right: bool, #[case] expected: u32) {
        let parties: Vec<_> = (1..6).map(PartyId::from).collect();
        let secret_sharer = ShamirSecretSharer::new(parties[0].clone(), 2, parties).unwrap();
        let secret = ModularNumber::from_u32(right as u32);
        let shares: PartyShares<ModularNumber<U64SafePrime>> =
            secret_sharer.generate_shares(&secret, PolyDegree::T).expect("generate shares failed");
        let mut result_shares = PartyShares::default();
        for (party_id, share) in shares {
            let share = BitShare::from(share);
            let result = share.xor_mask(left);
            result_shares.insert(party_id, ModularNumber::from(result));
        }
        let result: ModularNumber<U64SafePrime> = secret_sharer.recover(result_shares).expect("recover failed");
        assert_eq!(result, ModularNumber::from_u32(expected));
    }
}
