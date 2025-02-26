//! MPC virtual machine implementation
pub mod plan;
#[cfg(any(test, feature = "simulator"))]
pub mod simulator;
#[cfg(test)]
mod tests;

use crate::{
    protocols::{ecdsa_sign::EcdsaSign, eddsa_sign::EddsaSign, MPCProtocol},
    vm::plan::MPCProtocolPreprocessingElements,
};
use anyhow::Error;
use basic_types::PartyMessage;
use execution_engine_vm::vm::{
    instructions::{
        DefaultInstructionStateMachine, Instruction, InstructionResult, InstructionRouter, InstructionStateMachine,
    },
    sm::{ExecutionContext, VmStateMessage},
};
pub use execution_engine_vm::{
    metrics::ExecutionMetricsConfig,
    vm::{
        config::ExecutionVmConfig,
        instructions::{get_statistic_k, STATISTIC_KAPPA},
        ExecutionVm, VmYield,
    },
};
use math_lib::{
    fields::PrimeField,
    modular::{EncodedModularNumber, SafePrime},
};
use protocols::{
    conditionals::{
        equality::{PrivateOutputEqualityState, PrivateOutputEqualityStateMessage},
        equality_public_output::{PublicOutputEqualityState, PublicOutputEqualityStateMessage},
        if_else::{IfElseState, IfElseStateMessage},
        less_than::{CompareState, CompareStateMessage},
    },
    division::{
        division_public_divisor::{DivisionIntegerPublicDivisorState, DivisionIntegerPublicDivisorStateMessage},
        division_secret_divisor::{DivisionIntegerSecretDivisorState, DivisionIntegerSecretDivisorStateMessage},
        modulo2m_public_divisor::{Modulo2mState, Modulo2mStateMessage},
        modulo_public_divisor::{ModuloState, ModuloStateMessage},
        modulo_secret_divisor::{ModuloIntegerSecretDivisorState, ModuloIntegerSecretDivisorStateMessage},
        truncation_probabilistic::{TruncPrState, TruncPrStateMessage},
    },
    multiplication::multiplication_shares::{MultState, MultStateMessage},
    reveal::{RevealState, RevealStateMessage},
    threshold_ecdsa::signing::{EcdsaSignState, EcdsaSignStateMessage},
    threshold_eddsa::{EddsaSignState, EddsaSignStateMessage},
};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

/// A message for the execution VM.
pub type MPCExecutionVmMessage = VmStateMessage<MPCMessages>;

impl<T> Instruction<T> for MPCProtocol
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
    type Router = MPCInstructionRouter<T>;
    type Message = MPCMessages;

    fn run<F>(
        self,
        context: &mut ExecutionContext<F, T>,
        share_elements: Self::PreprocessingElement,
    ) -> Result<InstructionResult<Self::Router, T>, Error>
    where
        F: Instruction<T>,
    {
        use MPCProtocol::*;
        match self {
            Not(p) => p.run(context, share_elements),
            RandomInteger(p) => p.run(context, share_elements),
            RandomBoolean(p) => p.run(context, share_elements),
            TruncPr(p) => p.run(context, share_elements),
            IfElse(p) => p.run(context, share_elements),
            IfElsePublicCond(p) => p.run(context, share_elements),
            IfElsePublicBranches(p) => p.run(context, share_elements),
            Addition(p) => p.run(context, share_elements),
            Subtraction(p) => p.run(context, share_elements),
            MultiplicationPublic(p) => p.run(context, share_elements),
            MultiplicationSharePublic(p) => p.run(context, share_elements),
            MultiplicationShares(p) => p.run(context, share_elements),
            NewArray(p) => p.run(context, share_elements),
            NewTuple(p) => p.run(context, share_elements),
            DivisionIntegerPublic(p) => p.run(context, share_elements),
            DivisionIntegerSecretDividendPublicDivisor(p) => p.run(context, share_elements),
            DivisionIntegerSecretDivisor(p) => p.run(context, share_elements),
            EqualsPublic(p) => p.run(context, share_elements),
            EqualsSecret(p) => p.run(context, share_elements),
            LeftShiftPublic(p) => p.run(context, share_elements),
            LeftShiftShares(p) => p.run(context, share_elements),
            LessThanPublic(p) => p.run(context, share_elements),
            LessThanShares(p) => p.run(context, share_elements),
            ModuloIntegerPublic(p) => p.run(context, share_elements),
            ModuloIntegerSecretDividendPublicDivisor(p) => p.run(context, share_elements),
            ModuloIntegerSecretDivisor(p) => p.run(context, share_elements),
            PowerPublicBasePublicExponent(p) => p.run(context, share_elements),
            RightShiftPublic(p) => p.run(context, share_elements),
            RightShiftShares(p) => p.run(context, share_elements),
            PublicOutputEquality(p) => p.run(context, share_elements),
            Reveal(p) => p.run(context, share_elements),
            PublicKeyDerive(p) => p.run(context, share_elements),
            InnerProductShares(p) => p.run(context, share_elements),
            InnerProductPublic(p) => p.run(context, share_elements),
            InnerProductSharePublic(p) => p.run(context, share_elements),
            EcdsaSign(p) => p.run(context, share_elements),
            EddsaSign(p) => p.run(context, share_elements),
        }
    }
}

/// Specific instruction message content
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MPCMessages {
    /// Message that is sent to run a division integer with public divisor protocol
    DivisionIntegerPublicDivisor(DivisionIntegerPublicDivisorStateMessage),
    /// Message that is sent to run a division integer with secret divisor protocol
    DivisionIntegerSecretDivisor(DivisionIntegerSecretDivisorStateMessage),
    /// Message is sent to run an if else protocol
    IfElse(IfElseStateMessage),
    /// Message is sent to run a less than protocol
    LessThan(CompareStateMessage),
    /// Message that is sent to run a modulo protocol
    Modulo(ModuloStateMessage),
    /// Message that is sent to run a modulo protocol
    ModuloIntegerSecretDivisor(ModuloIntegerSecretDivisorStateMessage),
    /// Message is sent to run a shares multiplication protocol
    Multiplication(MultStateMessage),
    /// Message that is sent to run a public output equality protocol
    PublicOutputEquality(PublicOutputEqualityStateMessage),
    /// Message is sent to run a reveal protocol
    Reveal(RevealStateMessage<EncodedModularNumber>),
    /// Message is sent to run a right shift (truncation) protocol
    RightShift(Modulo2mStateMessage),
    /// Message is sent to run a probabilistic truncation protocol
    TruncPr(TruncPrStateMessage),
    /// Message is sent to run a private output equality protocol
    EqualsIntegerSecret(PrivateOutputEqualityStateMessage),
    /// Message is sent to run an ecdsa sign protocol
    EcdsaSign(EcdsaSignStateMessage),
    /// Message is sent to run an eddsa sign protocol
    EddsaSign(EddsaSignStateMessage),
}

/// MPC Protocols state machine
pub enum MPCInstructionRouter<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// State machine for the protocol `DivisionIntegerPublicDivisor`
    DivisionIntegerPublicDivisor(DefaultInstructionStateMachine<MPCMessages, DivisionIntegerPublicDivisorState<T>>),
    /// State machine for the protocol `DivisionIntegerSecretDivisor`
    DivisionIntegerSecretDivisor(DefaultInstructionStateMachine<MPCMessages, DivisionIntegerSecretDivisorState<T>>),
    /// State machine for the protocol `IfElse`
    IfElse(DefaultInstructionStateMachine<MPCMessages, IfElseState<T>>),
    /// State machine for the protocol `LessThan`
    LessThan(DefaultInstructionStateMachine<MPCMessages, CompareState<T>>),
    /// State machine for the protocol `Modulo`
    Modulo(DefaultInstructionStateMachine<MPCMessages, ModuloState<T>>),
    /// State machine for the protocol `ModuloIntegerSecretDivisor`
    ModuloIntegerSecretDivisor(DefaultInstructionStateMachine<MPCMessages, ModuloIntegerSecretDivisorState<T>>),
    /// State machine for the protocol `Multiplication`
    Multiplication(DefaultInstructionStateMachine<MPCMessages, MultState<T>>),
    /// State machine for the protocol `PublicOutputEquality`
    PublicOutputEquality(DefaultInstructionStateMachine<MPCMessages, PublicOutputEqualityState<T>>),
    /// State machine for the protocol `Reveal`
    Reveal(DefaultInstructionStateMachine<MPCMessages, RevealState<PrimeField<T>, ShamirSecretSharer<T>>>),
    /// State machine for the protocol `RightShift`
    RightShift(DefaultInstructionStateMachine<MPCMessages, Modulo2mState<T>>),
    /// State machine for the protocol `TruncPr`
    TruncPr(DefaultInstructionStateMachine<MPCMessages, TruncPrState<T>>),
    /// State machine for the protocol `EqualsIntegerSecret`
    EqualsIntegerSecret(DefaultInstructionStateMachine<MPCMessages, PrivateOutputEqualityState<T>>),
    /// State machine for the protocol `EcdsaSign`
    EcdsaSign(DefaultInstructionStateMachine<MPCMessages, EcdsaSignState>),
    /// State machine for the protocol `EddsaSign`
    EddsaSign(DefaultInstructionStateMachine<MPCMessages, EddsaSignState>),
}

impl<T> InstructionStateMachine<T> for MPCInstructionRouter<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type Message = MPCMessages;

    /// Check if the state machine is finished
    fn is_finished(&self) -> bool {
        match self {
            MPCInstructionRouter::DivisionIntegerPublicDivisor(sm) => sm.is_finished(),
            MPCInstructionRouter::DivisionIntegerSecretDivisor(sm) => sm.is_finished(),
            MPCInstructionRouter::IfElse(sm) => sm.is_finished(),
            MPCInstructionRouter::LessThan(sm) => sm.is_finished(),
            MPCInstructionRouter::Modulo(sm) => sm.is_finished(),
            MPCInstructionRouter::ModuloIntegerSecretDivisor(sm) => sm.is_finished(),
            MPCInstructionRouter::Multiplication(sm) => sm.is_finished(),
            MPCInstructionRouter::PublicOutputEquality(sm) => sm.is_finished(),
            MPCInstructionRouter::Reveal(sm) => sm.is_finished(),
            MPCInstructionRouter::RightShift(sm) => sm.is_finished(),
            MPCInstructionRouter::TruncPr(sm) => sm.is_finished(),
            MPCInstructionRouter::EqualsIntegerSecret(sm) => sm.is_finished(),
            MPCInstructionRouter::EcdsaSign(sm) => sm.sm.is_finished(),
            MPCInstructionRouter::EddsaSign(sm) => sm.sm.is_finished(),
        }
    }

    /// Delegates the handling of the protocol message to the protocol state machine
    fn handle_message<I>(
        &mut self,
        context: &mut ExecutionContext<I, T>,
        message: PartyMessage<Self::Message>,
    ) -> Result<InstructionResult<I::Router, T>, Error>
    where
        I: Instruction<T, Message = Self::Message>,
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        match self {
            MPCInstructionRouter::Reveal(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::RightShift(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::TruncPr(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::IfElse(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::LessThan(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::Modulo(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::ModuloIntegerSecretDivisor(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::PublicOutputEquality(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::DivisionIntegerPublicDivisor(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::DivisionIntegerSecretDivisor(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::Multiplication(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::EqualsIntegerSecret(sm) => sm.handle_message(context, message),
            MPCInstructionRouter::EcdsaSign(sm) => EcdsaSign::handle_message::<I, T>(sm, message),
            MPCInstructionRouter::EddsaSign(sm) => EddsaSign::handle_message::<I, T>(sm, message),
        }
    }
}

impl<T> InstructionRouter<T> for MPCInstructionRouter<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
}
