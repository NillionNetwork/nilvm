//! Outputs for the PREP-PUBLIC-OUTPUT-EQUALITY protocol.

use math_lib::modular::{DecodeError, EncodedModularNumber, ModularNumber, SafePrime};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

/// The shares produced on a successful PREP-PUBLIC-OUTPUT-EQUALITY run.
#[derive(Clone, Debug, Default)]
pub struct PrepPublicOutputEqualityShares<T: SafePrime> {
    /// Zero 2T
    pub zero_two_t: ModularNumber<T>,

    /// Rand
    pub ran: ModularNumber<T>,
}

impl<T: SafePrime> PrepPublicOutputEqualityShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepPublicOutputEqualityShares, Infallible> {
        EncodedPrepPublicOutputEqualityShares::try_from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepPublicOutputEqualityShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-PUBLIC-OUTPUT-EQUALITY shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepPublicOutputEqualityShares {
    ran: EncodedModularNumber,
    zero_two_t: EncodedModularNumber,
}

impl EncodedPrepPublicOutputEqualityShares {
    /// Try to decode these shares.
    pub fn try_decode<T: SafePrime>(&self) -> Result<PrepPublicOutputEqualityShares<T>, DecodeError> {
        PrepPublicOutputEqualityShares::try_from(self)
    }
}

impl<T: SafePrime> TryFrom<&PrepPublicOutputEqualityShares<T>> for EncodedPrepPublicOutputEqualityShares {
    type Error = Infallible;

    fn try_from(
        value: &PrepPublicOutputEqualityShares<T>,
    ) -> Result<EncodedPrepPublicOutputEqualityShares, Self::Error> {
        Ok(Self { ran: value.ran.encode(), zero_two_t: value.zero_two_t.encode() })
    }
}

impl<T: SafePrime> TryFrom<&EncodedPrepPublicOutputEqualityShares> for PrepPublicOutputEqualityShares<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedPrepPublicOutputEqualityShares) -> Result<Self, Self::Error> {
        Ok(Self {
            ran: ModularNumber::try_from_encoded(&value.ran)?,
            zero_two_t: ModularNumber::try_from_encoded(&value.zero_two_t)?,
        })
    }
}

/// The PREP-PUBLIC-OUTPUT-EQUALITY output.
#[derive(Clone)]
pub enum PrepPublicOutputEqualityStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output shares.
        shares: Vec<T>,
    },
}

impl<T: SafePrime> PrepPublicOutputEqualityStateOutput<PrepPublicOutputEqualityShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(
        &self,
    ) -> Result<PrepPublicOutputEqualityStateOutput<EncodedPrepPublicOutputEqualityShares>, Infallible> {
        match self {
            Self::Success { shares } => {
                let shares: Result<Vec<_>, Infallible> =
                    shares.iter().map(PrepPublicOutputEqualityShares::encode).collect();
                Ok(PrepPublicOutputEqualityStateOutput::Success { shares: shares? })
            }
        }
    }
}
