//! Outputs for the PREP-MOD2M protocol.

use std::{convert::Infallible, fmt::Display};

use crate::{
    conditionals::less_than::offline::output::{EncodedPrepCompareShares, PrepCompareShares},
    random::{random_bit::BitShare, random_bitwise::BitwiseNumberShares},
};
use math_lib::modular::{DecodeError, EncodedModularNumber, Modular, ModularNumber};
use serde::{Deserialize, Serialize};

/// The shares produced on a successful PREP-MOD2M run.
#[derive(Clone, Debug)]
pub struct PrepModulo2mShares<T: Modular> {
    /// The random bit shares used to compute r' and r''.
    pub ran_bits_r: BitwiseNumberShares<T>,

    /// The prep for the one comparison in the MOD2M protocol.
    pub prep_compare: Vec<PrepCompareShares<T>>,
}

impl<T: Modular> PrepModulo2mShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepModulo2mShares, Infallible> {
        EncodedPrepModulo2mShares::try_from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepModulo2mShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-MOD2M shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepModulo2mShares {
    ran_bits_r: Vec<EncodedModularNumber>,
    prep_compare: Vec<EncodedPrepCompareShares>,
}

impl EncodedPrepModulo2mShares {
    /// Try to decode these shares.
    pub fn try_decode<T: Modular>(&self) -> Result<PrepModulo2mShares<T>, DecodeError> {
        PrepModulo2mShares::try_from(self)
    }
}

impl<T: Modular> TryFrom<&PrepModulo2mShares<T>> for EncodedPrepModulo2mShares {
    type Error = Infallible;

    fn try_from(value: &PrepModulo2mShares<T>) -> Result<EncodedPrepModulo2mShares, Self::Error> {
        Ok(Self {
            ran_bits_r: value.ran_bits_r.shares().iter().map(|bit| ModularNumber::encode(bit.value())).collect(),
            prep_compare: value
                .prep_compare
                .iter()
                .map(|perp_compare_shares| perp_compare_shares.encode())
                .filter_map(Result::ok)
                .collect(),
        })
    }
}

impl<T: Modular> TryFrom<&EncodedPrepModulo2mShares> for PrepModulo2mShares<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedPrepModulo2mShares) -> Result<Self, Self::Error> {
        let ran_bits_r: Vec<_> = value
            .ran_bits_r
            .iter()
            .map(|mod_bit| ModularNumber::try_from_encoded(mod_bit).map(BitShare::from))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            ran_bits_r: BitwiseNumberShares::from(ran_bits_r),
            prep_compare: value
                .prep_compare
                .iter()
                .map(PrepCompareShares::try_from_encoded)
                .collect::<Result<_, _>>()?,
        })
    }
}

/// The PREP-MOD2M output.
#[derive(Clone, Debug)]
pub enum PrepModulo2mStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Vec<T>,
    },

    /// This or a subprotocol aborted by chance.
    Abort,

    /// RAN was aborted.
    RanAbort,

    /// COMPARE was aborted.
    PrepCompareAbort,
}

impl<T> Display for PrepModulo2mStateOutput<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort => write!(f, "Abort"),
            Self::RanAbort => write!(f, "RanAbort"),
            Self::PrepCompareAbort => write!(f, "PrepCompareAbort"),
        }
    }
}

impl<T: Modular> PrepModulo2mStateOutput<PrepModulo2mShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(&self) -> Result<PrepModulo2mStateOutput<EncodedPrepModulo2mShares>, Infallible> {
        match self {
            Self::Success { shares } => {
                let shares: Result<Vec<_>, Infallible> = shares.iter().map(PrepModulo2mShares::encode).collect();
                Ok(PrepModulo2mStateOutput::Success { shares: shares? })
            }
            Self::Abort => Ok(PrepModulo2mStateOutput::Abort),
            Self::RanAbort => Ok(PrepModulo2mStateOutput::RanAbort),
            Self::PrepCompareAbort => Ok(PrepModulo2mStateOutput::PrepCompareAbort),
        }
    }
}
