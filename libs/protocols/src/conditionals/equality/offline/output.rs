//! Outputs for the PREP PRIVATE OUTPUT EQUALITY protocol.
use math_lib::{
    fields::PrimeField,
    modular::{DecodeError, EncodedModularNumber, SafePrime},
    polynomial::Polynomial,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;

use crate::{
    conditionals::poly_eval::offline::output::{EncodedPrepPolyEvalShares, PrepPolyEvalShares},
    random::{random_bit::BitShare, random_bitwise::BitwiseNumberShares},
};

#[derive(Debug, Clone)]
/// The output of the PRIVATE OUTPUT EQUALITY protocol.
pub struct PrepPrivateOutputEqualityShares<T>
where
    T: SafePrime,
{
    /// Bitwise Number Shares
    pub bitwise_number_shares: BitwiseNumberShares<T>,
    /// The lagrange polynomial
    pub lagrange_polynomial: Polynomial<PrimeField<T>>,
    /// The outputs of running the prep_eval_poly preprocessing protocol. It contains:
    /// - The invertible number
    /// - The powers of the invertible number
    /// - The shares of zero
    pub prep_poly_eval: PrepPolyEvalShares<T>,
}

/// The output of this state machine.
#[derive(Debug, Clone)]
pub enum PrepPrivateOutputEqualityStateOutput<T> {
    /// The protocol was successful.
    Success {
        /// The output of the protocol.
        shares: Vec<T>,
    },

    /// RAN BITWISE failed.
    RanBitwiseAbort,

    /// PREP POLY EVAL failed.
    PrepPolyEvalAbort,
}

impl<T: SafePrime> PrepPrivateOutputEqualityShares<T> {
    /// Encode this share.
    pub fn encode(&self) -> Result<EncodedPrepPrivateOutputEqualityShares, Infallible> {
        EncodedPrepPrivateOutputEqualityShares::try_from(self)
    }

    /// Try to decode an encoded share.
    pub fn try_from_encoded(encoded: &EncodedPrepPrivateOutputEqualityShares) -> Result<Self, DecodeError> {
        encoded.try_into()
    }
}

/// An encoded version of the PREP-POLY-EVAL shares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodedPrepPrivateOutputEqualityShares {
    bitwise_number_shares: Vec<EncodedModularNumber>,
    lagrange_polynomial: Vec<EncodedModularNumber>,
    prep_poly_eval: EncodedPrepPolyEvalShares,
}

impl EncodedPrepPrivateOutputEqualityShares {
    /// Try to decode these shares.
    pub fn try_decode<T: SafePrime>(&self) -> Result<PrepPrivateOutputEqualityShares<T>, DecodeError> {
        PrepPrivateOutputEqualityShares::try_from(self)
    }
}

impl<T: SafePrime> TryFrom<&PrepPrivateOutputEqualityShares<T>> for EncodedPrepPrivateOutputEqualityShares {
    type Error = Infallible;

    fn try_from(
        value: &PrepPrivateOutputEqualityShares<T>,
    ) -> Result<EncodedPrepPrivateOutputEqualityShares, Self::Error> {
        Ok(Self {
            bitwise_number_shares: value.bitwise_number_shares.shares().iter().map(|x| x.value().encode()).collect(),
            lagrange_polynomial: value.lagrange_polynomial.encode(),
            prep_poly_eval: value.prep_poly_eval.encode()?,
        })
    }
}

impl<T: SafePrime> TryFrom<&EncodedPrepPrivateOutputEqualityShares> for PrepPrivateOutputEqualityShares<T> {
    type Error = DecodeError;

    fn try_from(value: &EncodedPrepPrivateOutputEqualityShares) -> Result<Self, Self::Error> {
        let bitwise_number_shares = BitwiseNumberShares::<T>::from(
            value
                .bitwise_number_shares
                .iter()
                .map(|x| x.try_decode())
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(BitShare::from)
                .collect::<Vec<_>>(),
        );

        let lagrange_polynomial = Polynomial::<PrimeField<T>>::try_decode(value.lagrange_polynomial.clone())?;
        Ok(Self { bitwise_number_shares, lagrange_polynomial, prep_poly_eval: value.prep_poly_eval.try_decode()? })
    }
}

impl<T: SafePrime> PrepPrivateOutputEqualityStateOutput<PrepPrivateOutputEqualityShares<T>> {
    /// Encode the shares in this output.
    pub fn encode(
        &self,
    ) -> Result<PrepPrivateOutputEqualityStateOutput<EncodedPrepPrivateOutputEqualityShares>, Infallible> {
        match self {
            Self::Success { shares: outputs } => {
                let outputs: Result<Vec<_>, Infallible> =
                    outputs.iter().map(PrepPrivateOutputEqualityShares::encode).collect();
                Ok(PrepPrivateOutputEqualityStateOutput::Success { shares: outputs? })
            }
            Self::RanBitwiseAbort => Ok(PrepPrivateOutputEqualityStateOutput::RanBitwiseAbort),
            Self::PrepPolyEvalAbort => Ok(PrepPrivateOutputEqualityStateOutput::PrepPolyEvalAbort),
        }
    }
}
