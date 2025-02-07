//! Outputs for the PREP-COMPARE protocol.

use crate::{
    multiplication::multiplication_unbounded::prefix::PrefixMultTuple,
    random::random_quaternary::{QuatShare, QuaternaryShares},
};
use math_lib::modular::{DecodeError, EncodedModularNumber, Modular, ModularNumber};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

/// The shares produced on a successful PREP-COMPARE run.
#[derive(Clone, Debug)]
pub struct PrepCompareShares<T: Modular> {
    /// The bitwise number.
    pub bitwise: ModularNumber<T>,

    /// The quaternary number.
    pub quaternary: QuaternaryShares<T>,

    /// The comparison least significant bit.
    pub comparison_least_bit: ModularNumber<T>,

    /// The comparison most significant bit.
    pub comparison_most_bit: ModularNumber<T>,

    /// The prefix mult tuple.
    pub prefix_mult_tuples: Vec<PrefixMultTuple<T>>,

    /// The zero shares for PUB-MULTs.
    pub zero_shares: Vec<ModularNumber<T>>,
}

impl<T: Modular> PrepCompareShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepCompareShares, Infallible> {
        Ok(EncodedPrepCompareShares::from(self))
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepCompareShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-COMPARE shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepCompareShares {
    bitwise: EncodedModularNumber,
    quaternary: Vec<EncodedModularNumber>,
    comparison_least_bit: EncodedModularNumber,
    comparison_most_bit: EncodedModularNumber,
    prefix_mult_tuples: Vec<EncodedModularNumber>,
    zero_shares: Vec<EncodedModularNumber>,
}

impl EncodedPrepCompareShares {
    /// Try to decode these shares.
    pub fn try_decode<T: Modular>(&self) -> Result<PrepCompareShares<T>, DecodeError> {
        PrepCompareShares::try_from(self)
    }
}

impl<T: Modular> From<&PrepCompareShares<T>> for EncodedPrepCompareShares {
    fn from(value: &PrepCompareShares<T>) -> Self {
        let mut quaternary = Vec::new();
        for quat in value.quaternary.shares().iter() {
            quaternary.push(quat.low().encode());
            quaternary.push(quat.high().encode());
            quaternary.push(quat.cross().encode());
        }
        let mut prefix_mult_tuples = Vec::new();
        for tuple in &value.prefix_mult_tuples {
            prefix_mult_tuples.push(tuple.mask.encode());
            prefix_mult_tuples.push(tuple.domino.encode());
        }
        Self {
            bitwise: value.bitwise.encode(),
            quaternary,
            comparison_least_bit: value.comparison_least_bit.encode(),
            comparison_most_bit: value.comparison_most_bit.encode(),
            prefix_mult_tuples,
            zero_shares: value.zero_shares.iter().map(ModularNumber::encode).collect(),
        }
    }
}

impl<T: Modular> TryFrom<&EncodedPrepCompareShares> for PrepCompareShares<T> {
    type Error = DecodeError;

    #[allow(clippy::indexing_slicing)]
    fn try_from(value: &EncodedPrepCompareShares) -> Result<Self, Self::Error> {
        let mut quaternary = Vec::new();
        for chunk in value.quaternary.chunks(3) {
            if chunk.len() != 3 {
                return Err(DecodeError::ValueLength);
            }
            let low = ModularNumber::try_from_encoded(&chunk[0])?;
            let high = ModularNumber::try_from_encoded(&chunk[1])?;
            let cross = ModularNumber::try_from_encoded(&chunk[2])?;
            let quat = QuatShare::new(low, high, cross);
            quaternary.push(quat);
        }
        let quaternary = QuaternaryShares::from(quaternary);

        let mut prefix_mult_tuples = Vec::new();
        for chunk in value.prefix_mult_tuples.chunks(2) {
            if chunk.len() != 2 {
                return Err(DecodeError::ValueLength);
            }
            let mask = ModularNumber::try_from_encoded(&chunk[0])?;
            let domino = ModularNumber::try_from_encoded(&chunk[1])?;
            let tuple = PrefixMultTuple { mask, domino };
            prefix_mult_tuples.push(tuple);
        }

        Ok(Self {
            bitwise: ModularNumber::try_from_encoded(&value.bitwise)?,
            quaternary,
            comparison_least_bit: ModularNumber::try_from_encoded(&value.comparison_least_bit)?,
            comparison_most_bit: ModularNumber::try_from_encoded(&value.comparison_most_bit)?,
            prefix_mult_tuples,
            zero_shares: value.zero_shares.iter().map(ModularNumber::try_from_encoded).collect::<Result<_, _>>()?,
        })
    }
}

/// The PREP-COMPARE output.
#[derive(Clone)]
pub enum PrepCompareStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Vec<T>,
    },

    /// This or a subprotocol aborted by chance.
    Abort,

    /// RAN bitwise was aborted.
    RanBitwiseAbort,
}

impl<T> std::fmt::Display for PrepCompareStateOutput<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort => write!(f, "Abort"),
            Self::RanBitwiseAbort => write!(f, "RanBitwiseAbort"),
        }
    }
}

impl<T: Modular> PrepCompareStateOutput<PrepCompareShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(&self) -> Result<PrepCompareStateOutput<EncodedPrepCompareShares>, Infallible> {
        match self {
            Self::Success { shares } => {
                let shares: Result<Vec<_>, Infallible> = shares.iter().map(PrepCompareShares::encode).collect();
                Ok(PrepCompareStateOutput::Success { shares: shares? })
            }
            Self::Abort => Ok(PrepCompareStateOutput::Abort),
            Self::RanBitwiseAbort => Ok(PrepCompareStateOutput::RanBitwiseAbort),
        }
    }
}
