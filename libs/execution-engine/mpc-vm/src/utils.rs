//! Useful macros for the MPC implementation

macro_rules! delegate_to_inner {
    ($on:ident, $method:tt $(, $opt:expr)*) => {
        match $on {
                            MPCProtocol::Addition(p) => p.$method($($opt),*),
                            MPCProtocol::Subtraction(p) => p.$method($($opt),*),
                            MPCProtocol::MultiplicationPublic(p) => p.$method($($opt),*),
                            MPCProtocol::MultiplicationShares(p) => p.$method($($opt),*),
                            MPCProtocol::MultiplicationSharePublic(p) => p.$method($($opt),*),
                            MPCProtocol::TruncPr(p) => p.$method($($opt),*),
                            MPCProtocol::Not(p) => p.$method($($opt),*),
                            MPCProtocol::IfElse(p) => p.$method($($opt),*),
                            MPCProtocol::IfElsePublicCond(p) => p.$method($($opt),*),
                            MPCProtocol::IfElsePublicBranches(p) => p.$method($($opt),*),
                            MPCProtocol::RandomInteger(p) => p.$method($($opt),*),
                            MPCProtocol::RandomBoolean(p) => p.$method($($opt),*),
                            MPCProtocol::DivisionIntegerPublic(p) => p.$method($($opt),*),
                            MPCProtocol::DivisionIntegerSecretDividendPublicDivisor(p) => p.$method($($opt),*),
                            MPCProtocol::DivisionIntegerSecretDivisor(p) => p.$method($($opt),*),
                            MPCProtocol::EqualsSecret(p) => p.$method($($opt),*),
                            MPCProtocol::EqualsPublic(p) => p.$method($($opt),*),
                            MPCProtocol::LeftShiftPublic(p) => p.$method($($opt),*),
                            MPCProtocol::LeftShiftShares(p) => p.$method($($opt),*),
                            MPCProtocol::LessThanPublic(p) => p.$method($($opt),*),
                            MPCProtocol::LessThanShares(p) => p.$method($($opt),*),
                            MPCProtocol::ModuloIntegerPublic(p) => p.$method($($opt),*),
                            MPCProtocol::ModuloIntegerSecretDividendPublicDivisor(p) => p.$method($($opt),*),
                            MPCProtocol::ModuloIntegerSecretDivisor(p) => p.$method($($opt),*),
                            MPCProtocol::PowerPublicBasePublicExponent(p) => p.$method($($opt),*),
                            MPCProtocol::RightShiftPublic(p) => p.$method($($opt),*),
                            MPCProtocol::RightShiftShares(p) => p.$method($($opt),*),
                            MPCProtocol::PublicOutputEquality(p) => p.$method($($opt),*),
                            MPCProtocol::Reveal(p) => p.$method($($opt),*),
                            MPCProtocol::PublicKeyDerive(p) => p.$method($($opt),*),
                            MPCProtocol::NewArray(p) => p.$method($($opt),*),
                            MPCProtocol::NewTuple(p) => p.$method($($opt),*),
                            MPCProtocol::InnerProductPublic(p) => p.$method($($opt),*),
                            MPCProtocol::InnerProductSharePublic(p) => p.$method($($opt),*),
                            MPCProtocol::InnerProductShares(p) => p.$method($($opt),*),
                            MPCProtocol::EcdsaSign(p) => p.$method($($opt),*),
                        }
    };
}

macro_rules! into_mpc_protocol {
    ($ty:ident) => {
        impl From<$ty> for $crate::protocols::MPCProtocol {
            fn from(protocol: $ty) -> Self {
                MPCProtocol::$ty(protocol)
            }
        }
    };
}

pub(crate) use delegate_to_inner;
pub(crate) use into_mpc_protocol;
