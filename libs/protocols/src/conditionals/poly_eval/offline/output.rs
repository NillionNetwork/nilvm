//! Outputs for the POLY EVAL protocol.  
use math_lib::modular::{DecodeError, EncodedModularNumber, Modular, ModularNumber};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

#[derive(Debug, Clone)]
/// The output of the POLY EVAL protocol.
pub struct PrepPolyEvalShares<T: Modular> {
    /// The invertible elements generated in the first phase.
    pub invertible_number: ModularNumber<T>,
    /// The powers of the number.
    pub powers: Vec<ModularNumber<T>>,
    /// The shares of zero.
    pub zero_share: ModularNumber<T>,
}

/// The output of this state machine.
pub enum PrepPolyEvalStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output of the protocol.
        outputs: Vec<T>,
    },

    /// INV-RAN was aborted.
    InvRanAbort,
}

impl<T: Modular> PrepPolyEvalShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepPolyEvalShares, Infallible> {
        EncodedPrepPolyEvalShares::try_from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepPolyEvalShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-POLY-EVAL shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepPolyEvalShares {
    invertible_number: EncodedModularNumber,
    powers: Vec<EncodedModularNumber>,
    zero_share: EncodedModularNumber,
}

impl EncodedPrepPolyEvalShares {
    /// Try to decode these shares.
    pub fn try_decode<T: Modular>(&self) -> Result<PrepPolyEvalShares<T>, DecodeError> {
        PrepPolyEvalShares::try_from(self)
    }
}

impl<T: Modular> TryFrom<&PrepPolyEvalShares<T>> for EncodedPrepPolyEvalShares {
    type Error = Infallible;

    fn try_from(value: &PrepPolyEvalShares<T>) -> Result<EncodedPrepPolyEvalShares, Self::Error> {
        Ok(Self {
            invertible_number: value.invertible_number.encode(),
            powers: value.powers.iter().map(|x| x.encode()).collect(),
            zero_share: value.zero_share.encode(),
        })
    }
}

impl<T: Modular> TryFrom<&EncodedPrepPolyEvalShares> for PrepPolyEvalShares<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedPrepPolyEvalShares) -> Result<Self, Self::Error> {
        Ok(Self {
            invertible_number: ModularNumber::try_from_encoded(&value.invertible_number)?,
            powers: value.powers.iter().map(|x| ModularNumber::try_from_encoded(x)).collect::<Result<Vec<_>, _>>()?,
            zero_share: ModularNumber::try_from_encoded(&value.zero_share)?,
        })
    }
}

impl<T: Modular> PrepPolyEvalStateOutput<PrepPolyEvalShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(&self) -> Result<PrepPolyEvalStateOutput<EncodedPrepPolyEvalShares>, Infallible> {
        match self {
            Self::Success { outputs } => {
                let outputs: Result<Vec<_>, Infallible> = outputs.iter().map(PrepPolyEvalShares::encode).collect();
                Ok(PrepPolyEvalStateOutput::Success { outputs: outputs? })
            }
            Self::InvRanAbort => Ok(PrepPolyEvalStateOutput::InvRanAbort),
        }
    }
}
