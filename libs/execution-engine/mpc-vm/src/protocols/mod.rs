//! MPC protocols implementation

pub(crate) mod addition;
pub(crate) mod division;
pub(crate) mod ecdsa_sign;
pub(crate) mod equals;
pub(crate) mod if_else;
pub(crate) mod inner_product;
pub(crate) mod left_shift;
pub(crate) mod less_than;
pub(crate) mod modulo;
pub(crate) mod multiplication;
pub(crate) mod new;
pub(crate) mod not;

pub(crate) mod power;
pub(crate) mod random;
pub(crate) mod reveal;
pub(crate) mod right_shift;

pub(crate) mod subtraction;
pub(crate) mod trunc_pr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::{
    protocols::{
        addition::Addition,
        division::{DivisionIntegerPublic, DivisionIntegerSecretDividendPublicDivisor, DivisionIntegerSecretDivisor},
        ecdsa_sign::EcdsaSign,
        equals::{EqualsPublic, EqualsSecret, PublicOutputEquality},
        if_else::{IfElse, IfElsePublicBranches, IfElsePublicCond},
        inner_product::{InnerProductPublic, InnerProductSharePublic, InnerProductShares},
        left_shift::{LeftShiftPublic, LeftShiftShares},
        less_than::{LessThanPublic, LessThanShares},
        modulo::{ModuloIntegerPublic, ModuloIntegerSecretDividendPublicDivisor, ModuloIntegerSecretDivisor},
        multiplication::{MultiplicationPublic, MultiplicationSharePublic, MultiplicationShares},
        new::{NewArray, NewTuple},
        not::Not,
        power::PowerPublicBasePublicExponent,
        random::{RandomBoolean, RandomInteger},
        reveal::Reveal,
        right_shift::{RightShiftPublic, RightShiftShares},
        subtraction::Subtraction,
        trunc_pr::TruncPr,
    },
    requirements::RuntimeRequirementType,
    utils::delegate_to_inner,
};
use jit_compiler::models::{
    protocols::{memory::ProtocolAddress, ExecutionLine, Protocol, ProtocolDependencies},
    SourceRefIndex,
};
use nada_value::NadaType;
use std::fmt::{Display, Formatter};
use strum::VariantNames;

/// A circuit protocol.
///
/// This models the interconnection between different circuits.
#[derive(Debug, Clone, VariantNames)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u8)]
pub enum MPCProtocol {
    /// Addition operation protocol
    Addition(Addition) = 0,
    /// Subtraction operation protocol
    Subtraction(Subtraction) = 1,
    /// Public Multiplication operation protocol
    MultiplicationPublic(MultiplicationPublic) = 2,
    /// Shares Multiplication operation protocol
    MultiplicationShares(MultiplicationShares) = 5,
    /// Share / Public multiplication protocol
    MultiplicationSharePublic(MultiplicationSharePublic) = 6,
    /// TruncPr operation protocol
    TruncPr(TruncPr) = 7,
    /// Protocol that implements the not operation
    Not(Not) = 8,
    /// If else operation protocol
    IfElse(IfElse) = 12,
    /// If else operation protocol with public condition
    IfElsePublicCond(IfElsePublicCond) = 39,
    /// If else operation protocol with public branches
    IfElsePublicBranches(IfElsePublicBranches) = 40,

    /// Random Integer protocol
    RandomInteger(RandomInteger) = 17,
    /// Protocol for Integer division with public elements
    DivisionIntegerPublic(DivisionIntegerPublic) = 18,
    /// Protocol for Integer division with public divisor
    DivisionIntegerSecretDividendPublicDivisor(DivisionIntegerSecretDividendPublicDivisor) = 19,
    /// Protocol for Integer division with secret divisor
    DivisionIntegerSecretDivisor(DivisionIntegerSecretDivisor) = 20,
    /// Protocol for Integer Secret Equals
    EqualsSecret(EqualsSecret) = 21,
    /// Protocol for Integer Public Equals
    EqualsPublic(EqualsPublic) = 22,
    /// Protocol for public left shift
    LeftShiftPublic(LeftShiftPublic) = 23,
    /// Protocol for shares left shift
    LeftShiftShares(LeftShiftShares) = 24,
    /// Protocol for public less than
    LessThanPublic(LessThanPublic) = 26,
    /// Protocol for shares less than
    LessThanShares(LessThanShares) = 27,
    /// Protocol for Integer modulo with public elements
    ModuloIntegerPublic(ModuloIntegerPublic) = 28,
    /// Protocol for Integer modulo with public divisor
    ModuloIntegerSecretDividendPublicDivisor(ModuloIntegerSecretDividendPublicDivisor) = 29,
    /// Protocol for Integer modulo with secret divisor
    ModuloIntegerSecretDivisor(ModuloIntegerSecretDivisor) = 30,
    /// Protocol for public base and public exponent power
    PowerPublicBasePublicExponent(PowerPublicBasePublicExponent) = 31,
    /// Protocol for public right shift
    RightShiftPublic(RightShiftPublic) = 33,
    /// Protocol for shares right shift
    RightShiftShares(RightShiftShares) = 34,
    /// Protocol for shares equals that returns a public result
    PublicOutputEquality(PublicOutputEquality) = 35,
    /// Protocol that reveals the value of a share
    Reveal(Reveal) = 36,
    /// New array protocol variant
    NewArray(NewArray) = 37,
    /// New tuple protocol variant
    NewTuple(NewTuple) = 38,
    /// Inner product shares protocol variant
    InnerProductShares(InnerProductShares) = 42,
    /// Inner product share public protocol variant
    InnerProductSharePublic(InnerProductSharePublic) = 43,
    /// Inner product public protocol variant
    InnerProductPublic(InnerProductPublic) = 44,
    /// Random Boolean protocol
    RandomBoolean(RandomBoolean) = 45,
    /// Ecdsa sign protocol
    EcdsaSign(EcdsaSign) = 46,
}

impl Display for MPCProtocol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use MPCProtocol::*;
        match self {
            Addition(p) => write!(f, "{}", p),
            Subtraction(p) => write!(f, "{}", p),
            MultiplicationPublic(p) => write!(f, "{}", p),

            MultiplicationShares(p) => write!(f, "{}", p),
            MultiplicationSharePublic(p) => write!(f, "{}", p),
            Not(p) => write!(f, "{}", p),
            IfElse(p) => write!(f, "{}", p),
            IfElsePublicCond(p) => write!(f, "{}", p),
            IfElsePublicBranches(p) => write!(f, "{}", p),
            RandomInteger(p) => write!(f, "{}", p),
            RandomBoolean(p) => write!(f, "{}", p),
            TruncPr(p) => write!(f, "{}", p),

            DivisionIntegerPublic(p) => write!(f, "{}", p),
            DivisionIntegerSecretDividendPublicDivisor(p) => write!(f, "{}", p),
            DivisionIntegerSecretDivisor(p) => write!(f, "{}", p),
            EqualsPublic(p) => write!(f, "{}", p),
            EqualsSecret(p) => write!(f, "{}", p),
            LeftShiftPublic(p) => write!(f, "{}", p),
            LeftShiftShares(p) => write!(f, "{}", p),

            LessThanPublic(p) => write!(f, "{}", p),
            LessThanShares(p) => write!(f, "{}", p),
            ModuloIntegerPublic(p) => write!(f, "{}", p),
            ModuloIntegerSecretDividendPublicDivisor(p) => write!(f, "{}", p),
            ModuloIntegerSecretDivisor(p) => write!(f, "{}", p),
            PowerPublicBasePublicExponent(p) => write!(f, "{}", p),

            RightShiftPublic(p) => write!(f, "{}", p),
            RightShiftShares(p) => write!(f, "{}", p),
            PublicOutputEquality(p) => write!(f, "{}", p),
            Reveal(p) => write!(f, "{}", p),
            NewArray(p) => write!(f, "{}", p),
            NewTuple(p) => write!(f, "{}", p),
            InnerProductShares(p) => write!(f, "{}", p),
            InnerProductSharePublic(p) => write!(f, "{}", p),
            InnerProductPublic(p) => write!(f, "{}", p),
            EcdsaSign(p) => write!(f, "{}", p),
        }
    }
}

impl Protocol for MPCProtocol {
    type RequirementType = RuntimeRequirementType;

    fn ty(&self) -> &NadaType {
        delegate_to_inner!(self, ty)
    }

    fn name(&self) -> &'static str {
        delegate_to_inner!(self, name)
    }

    fn with_address(&mut self, address: ProtocolAddress) {
        delegate_to_inner!(self, with_address, address);
    }
    fn address(&self) -> ProtocolAddress {
        delegate_to_inner!(self, address)
    }

    fn runtime_requirements(&self) -> &[(Self::RequirementType, usize)] {
        delegate_to_inner!(self, runtime_requirements)
    }

    fn execution_line(&self) -> ExecutionLine {
        delegate_to_inner!(self, execution_line)
    }

    fn source_ref_index(&self) -> &SourceRefIndex {
        delegate_to_inner!(self, source_ref_index)
    }
}

impl ProtocolDependencies for MPCProtocol {
    fn dependencies(&self) -> Vec<ProtocolAddress> {
        delegate_to_inner!(self, dependencies)
    }
}

impl MPCProtocol {
    /// Generates a list with the names of all the protocols
    pub fn list() -> &'static [&'static str] {
        MPCProtocol::VARIANTS
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::compile_protocols;
    use anyhow::Error;
    use rstest::rstest;
    #[rstest]
    #[case("simple", 7)]
    #[case("simple_literals", 8)]
    #[case("sum", 10)]
    #[case("array_new_2_dimensional", 13)]
    #[case("inner_product", 15)]
    #[case("zip_simple", 21)]
    #[case("unzip_simple", 19)]
    #[case("cardio_risk_factor", 43)]
    fn memory_size(#[case] program_name: &str, #[case] expected_size: usize) -> Result<(), Error> {
        let program = compile_protocols(program_name)?;
        assert_eq!(program.memory_size(), expected_size);
        Ok(())
    }
}
