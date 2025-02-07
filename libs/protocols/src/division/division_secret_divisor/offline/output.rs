//! Outputs for the PREP-DIV-INT-SECRET protocol.

use std::{convert::Infallible, fmt::Display};

use crate::{
    conditionals::less_than::offline::output::{EncodedPrepCompareShares, PrepCompareShares},
    division::{
        modulo2m_public_divisor::offline::{EncodedPrepModulo2mShares, PrepModulo2mShares},
        truncation_probabilistic::offline::{EncodedPrepTruncPrShares, PrepTruncPrShares},
    },
    random::random_bitwise::BitwiseNumberShares,
};
use math_lib::modular::{DecodeError, EncodedModularNumber, Modular, ModularNumber};
use serde::{Deserialize, Serialize};

/// The shares produced on a successful PREP-DIV-INT-SECRET run.
#[derive(Clone, Debug)]
pub struct PrepDivisionIntegerSecretShares<T: Modular> {
    /// The shares for all the comparisons
    pub prep_compare: Vec<PrepCompareShares<T>>,

    /// Pre-processing shares for TRUNCPR and MULTIPLICATION-AND-TRUNCATION protocol
    pub prep_truncpr: Vec<PrepTruncPrShares<T>>,

    /// Pre-processing shares for the TRUNC step
    pub prep_trunc: PrepModulo2mShares<T>,

    /// Pre-processing shares for the BIT-DECOMPOSE step
    pub prep_bit_decompose: BitwiseNumberShares<T>,
}

impl<T: Modular> PrepDivisionIntegerSecretShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepDivisionIntegerSecretShares, Infallible> {
        EncodedPrepDivisionIntegerSecretShares::try_from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepDivisionIntegerSecretShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-DIV-INT-SECRET shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepDivisionIntegerSecretShares {
    prep_compare: Vec<EncodedPrepCompareShares>,
    prep_truncpr: Vec<EncodedPrepTruncPrShares>,
    prep_trunc: EncodedPrepModulo2mShares,
    prep_bit_decompose: Vec<EncodedModularNumber>,
}

impl EncodedPrepDivisionIntegerSecretShares {
    /// Try to decode these shares.
    pub fn try_decode<T: Modular>(&self) -> Result<PrepDivisionIntegerSecretShares<T>, DecodeError> {
        PrepDivisionIntegerSecretShares::try_from(self)
    }
}

impl<T: Modular> TryFrom<&PrepDivisionIntegerSecretShares<T>> for EncodedPrepDivisionIntegerSecretShares {
    type Error = Infallible;

    fn try_from(
        value: &PrepDivisionIntegerSecretShares<T>,
    ) -> Result<EncodedPrepDivisionIntegerSecretShares, Self::Error> {
        let mut bitwise = Vec::new();
        for bit in value.prep_bit_decompose.shares().iter() {
            bitwise.push(bit.value().encode());
        }
        Ok(Self {
            prep_compare: value
                .prep_compare
                .iter()
                .map(|prep_compare_shares| prep_compare_shares.encode())
                .filter_map(Result::ok)
                .collect(),
            prep_truncpr: value
                .prep_truncpr
                .iter()
                .map(|prep_truncpr_shares| prep_truncpr_shares.encode())
                .filter_map(Result::ok)
                .collect(),
            prep_trunc: value.prep_trunc.encode()?,
            prep_bit_decompose: bitwise,
        })
    }
}

impl<T: Modular> TryFrom<&EncodedPrepDivisionIntegerSecretShares> for PrepDivisionIntegerSecretShares<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedPrepDivisionIntegerSecretShares) -> Result<Self, Self::Error> {
        let mut bitwise = Vec::new();
        for encoded_bit in value.prep_bit_decompose.iter() {
            let bit = ModularNumber::try_from_encoded(encoded_bit)?;
            bitwise.push(bit);
        }
        let bitwise = BitwiseNumberShares::from(bitwise);

        Ok(Self {
            prep_compare: value
                .prep_compare
                .iter()
                .map(PrepCompareShares::try_from_encoded)
                .collect::<Result<_, _>>()?,
            prep_truncpr: value
                .prep_truncpr
                .iter()
                .map(PrepTruncPrShares::try_from_encoded)
                .collect::<Result<_, _>>()?,
            prep_trunc: PrepModulo2mShares::try_from_encoded(&value.prep_trunc)?,
            prep_bit_decompose: bitwise,
        })
    }
}

/// The PREP-DIV-INT-SECRET output.
#[derive(Clone)]
pub enum PrepDivisionIntegerSecretStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Vec<T>,
    },

    /// This or a subprotocol aborted by chance.
    Abort,

    /// COMPARE was aborted.
    PrepCompareAbort,

    /// TRUNCPR was aborted.
    PrepTruncPrAbort,

    /// TRUNC was aborted.
    PrepTruncAbort,

    /// RANDOM-BITWISE was aborted.
    RanBitwiseAbort,
}

impl<T> Display for PrepDivisionIntegerSecretStateOutput<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort => write!(f, "Abort"),
            Self::PrepCompareAbort => write!(f, "PrepCompareAbort"),
            Self::PrepTruncPrAbort => write!(f, "PrepTruncPrAbort"),
            Self::PrepTruncAbort => write!(f, "PrepTruncAbort"),
            Self::RanBitwiseAbort => write!(f, "RanBitwiseAbort"),
        }
    }
}

impl<T: Modular> PrepDivisionIntegerSecretStateOutput<PrepDivisionIntegerSecretShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(
        &self,
    ) -> Result<PrepDivisionIntegerSecretStateOutput<EncodedPrepDivisionIntegerSecretShares>, Infallible> {
        match self {
            Self::Success { shares } => {
                let shares: Result<Vec<_>, Infallible> =
                    shares.iter().map(PrepDivisionIntegerSecretShares::encode).collect();
                Ok(PrepDivisionIntegerSecretStateOutput::Success { shares: shares? })
            }
            Self::Abort => Ok(PrepDivisionIntegerSecretStateOutput::Abort),
            Self::PrepCompareAbort => Ok(PrepDivisionIntegerSecretStateOutput::PrepCompareAbort),
            Self::PrepTruncPrAbort => Ok(PrepDivisionIntegerSecretStateOutput::PrepTruncPrAbort),
            Self::PrepTruncAbort => Ok(PrepDivisionIntegerSecretStateOutput::PrepTruncAbort),
            Self::RanBitwiseAbort => Ok(PrepDivisionIntegerSecretStateOutput::RanBitwiseAbort),
        }
    }
}
