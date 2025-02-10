//! Outputs for the PREP-MODULO protocol.

use std::{convert::Infallible, fmt::Display};

use crate::{
    conditionals::less_than::offline::output::{EncodedPrepCompareShares, PrepCompareShares},
    random::{random_bit::BitShare, random_bitwise::BitwiseNumberShares},
};
use math_lib::modular::{DecodeError, EncodedModularNumber, Modular, ModularNumber};
use serde::{Deserialize, Serialize};

/// The shares produced on a successful PREP-MODULO run.
#[derive(Clone, Debug)]
pub struct PrepModuloShares<T: Modular> {
    /// The random bit shares used to compute r' and r''.
    pub ran_bits_r: BitwiseNumberShares<T>,

    /// The prep for the two comparisons in the MODULO protocol.
    pub prep_compare: Vec<PrepCompareShares<T>>,
}

impl<T: Modular> PrepModuloShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepModuloShares, Infallible> {
        EncodedPrepModuloShares::try_from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepModuloShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-MODULO shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepModuloShares {
    ran_bits_r: Vec<EncodedModularNumber>,
    prep_compare: Vec<EncodedPrepCompareShares>,
}

impl EncodedPrepModuloShares {
    /// Try to decode these shares.
    pub fn try_decode<T: Modular>(&self) -> Result<PrepModuloShares<T>, DecodeError> {
        PrepModuloShares::try_from(self)
    }
}

impl<T: Modular> TryFrom<&PrepModuloShares<T>> for EncodedPrepModuloShares {
    type Error = Infallible;

    fn try_from(value: &PrepModuloShares<T>) -> Result<EncodedPrepModuloShares, Self::Error> {
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

impl<T: Modular> TryFrom<&EncodedPrepModuloShares> for PrepModuloShares<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedPrepModuloShares) -> Result<Self, Self::Error> {
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

/// The PREP-MODULO output.
#[derive(Clone)]
pub enum PrepModuloStateOutput<T> {
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

impl<T> Display for PrepModuloStateOutput<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort => write!(f, "Abort"),
            Self::RanAbort => write!(f, "RanAbort"),
            Self::PrepCompareAbort => write!(f, "PrepCompareAbort"),
        }
    }
}

impl<T: Modular> PrepModuloStateOutput<PrepModuloShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(&self) -> Result<PrepModuloStateOutput<EncodedPrepModuloShares>, Infallible> {
        match self {
            Self::Success { shares } => {
                let shares: Result<Vec<_>, Infallible> = shares.iter().map(PrepModuloShares::encode).collect();
                Ok(PrepModuloStateOutput::Success { shares: shares? })
            }
            Self::Abort => Ok(PrepModuloStateOutput::Abort),
            Self::RanAbort => Ok(PrepModuloStateOutput::RanAbort),
            Self::PrepCompareAbort => Ok(PrepModuloStateOutput::PrepCompareAbort),
        }
    }
}
