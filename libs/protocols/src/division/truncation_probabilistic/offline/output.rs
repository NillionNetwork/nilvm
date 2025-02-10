//! Outputs for the PREP-TRUNCPR protocol.

use std::{convert::Infallible, fmt::Display};

use crate::random::{random_bit::BitShare, random_bitwise::BitwiseNumberShares};
use math_lib::modular::{DecodeError, EncodedModularNumber, Modular, ModularNumber};
use serde::{Deserialize, Serialize};

/// The shares produced on a successful PREP-TRUNCPR run.
#[derive(Clone, Debug)]
pub struct PrepTruncPrShares<T: Modular> {
    /// The random bit shares used to compute r' and r''.
    pub ran_bits_r: BitwiseNumberShares<T>,
}

impl<T: Modular> PrepTruncPrShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepTruncPrShares, Infallible> {
        EncodedPrepTruncPrShares::try_from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepTruncPrShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-TRUNCPR shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepTruncPrShares {
    ran_bits_r: Vec<EncodedModularNumber>,
}

impl EncodedPrepTruncPrShares {
    /// Try to decode these shares.
    pub fn try_decode<T: Modular>(&self) -> Result<PrepTruncPrShares<T>, DecodeError> {
        PrepTruncPrShares::try_from(self)
    }
}

impl<T: Modular> TryFrom<&PrepTruncPrShares<T>> for EncodedPrepTruncPrShares {
    type Error = Infallible;

    fn try_from(value: &PrepTruncPrShares<T>) -> Result<EncodedPrepTruncPrShares, Self::Error> {
        Ok(Self {
            ran_bits_r: value.ran_bits_r.shares().iter().map(|bit| ModularNumber::encode(bit.value())).collect(),
        })
    }
}

impl<T: Modular> TryFrom<&EncodedPrepTruncPrShares> for PrepTruncPrShares<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedPrepTruncPrShares) -> Result<Self, Self::Error> {
        let ran_bits_r: Vec<_> = value
            .ran_bits_r
            .iter()
            .map(|mod_bit| ModularNumber::try_from_encoded(mod_bit).map(BitShare::from))
            .collect::<Result<_, _>>()?;
        Ok(Self { ran_bits_r: BitwiseNumberShares::from(ran_bits_r) })
    }
}

/// The PREP-TRUNCPR output.
#[derive(Clone)]
pub enum PrepTruncPrStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Vec<T>,
    },

    /// This or a subprotocol aborted by chance.
    Abort,

    /// RAN was aborted.
    RanAbort,
}

impl<T> Display for PrepTruncPrStateOutput<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success { .. } => write!(f, "Success"),
            Self::Abort => write!(f, "Abort"),
            Self::RanAbort => write!(f, "RanAbort"),
        }
    }
}

impl<T: Modular> PrepTruncPrStateOutput<PrepTruncPrShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(&self) -> Result<PrepTruncPrStateOutput<EncodedPrepTruncPrShares>, Infallible> {
        match self {
            Self::Success { shares } => {
                let shares: Result<Vec<_>, Infallible> = shares.iter().map(PrepTruncPrShares::encode).collect();
                Ok(PrepTruncPrStateOutput::Success { shares: shares? })
            }
            Self::Abort => Ok(PrepTruncPrStateOutput::Abort),
            Self::RanAbort => Ok(PrepTruncPrStateOutput::RanAbort),
        }
    }
}
