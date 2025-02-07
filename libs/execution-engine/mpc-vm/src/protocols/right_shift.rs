//! Implementation of the MPC protocols for the right-shift operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::RightShift as BytecodeRightShift, protocols::ExecutionLine},
    public_shift_protocol, share_shift_protocol,
};

pub(crate) struct RightShift;

impl RightShift {
    /// Transforms a bytecode right shift into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeRightShift,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        // Check types
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        if !right_type.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "The amount of shift should be public. {} not supported",
                right_type
            )));
        }

        let left_type = context.bytecode.memory_element_type(operation.left)?;

        if left_type.is_public() {
            RightShiftPublic::public_protocol(context, operation)
        } else {
            RightShiftShares::share_protocol(context, operation)
        }
    }
}

// Right shift with public inputs.
binary_protocol!(RightShiftPublic, "RSHFTC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(RightShiftPublic);
impl RightShiftPublic {
    public_shift_protocol!(BytecodeRightShift);
}

// Right shift with share inputs.
binary_protocol!(
    RightShiftShares,
    "RSHFTS",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::Trunc, 1)]
);
into_mpc_protocol!(RightShiftShares);
impl RightShiftShares {
    share_shift_protocol!(BytecodeRightShift);
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{RightShiftPublic, RightShiftShares},
        vm::{plan::MPCProtocolPreprocessingElements, MPCInstructionRouter, MPCMessages},
    };
    use anyhow::{anyhow, Error};
    use execution_engine_vm::vm::{
        errors::EvaluationError,
        instructions::{
            get_statistic_k, into_instruction_messages, DefaultInstructionStateMachine, Instruction, InstructionResult,
            STATISTIC_KAPPA,
        },
        memory::MemoryValue,
        sm::ExecutionContext,
    };
    use math_lib::modular::{FloorMod, ModularNumber, ModularPow, SafePrime};
    use nada_value::NadaValue;
    use protocols::division::modulo2m_public_divisor::{
        states::Mod2mTruncVariant, Modulo2mShares, Modulo2mState, Modulo2mStateMessage,
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::cmp::Ordering;

    impl<T> Instruction<T> for RightShiftPublic
    where
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        type PreprocessingElement = MPCProtocolPreprocessingElements<T>;
        type Router = MPCInstructionRouter<T>;
        type Message = MPCMessages;

        #[allow(clippy::arithmetic_side_effects)]
        fn run<F>(
            self,
            context: &mut ExecutionContext<F, T>,
            _: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            use nada_value::NadaType::*;

            let left = context.read(self.left)?;
            let shift_amount = context.read(self.right)?;

            let left_type = left.to_type();
            let shift_amount_type = shift_amount.to_type();

            let left = left.try_into_value()?;
            let shift_amount = shift_amount.try_into_value()?;
            let shift_amount = ModularNumber::two().exp_mod(&shift_amount.into_value());

            match shift_amount.cmp(&ModularNumber::ZERO) {
                Ordering::Less => Err(EvaluationError::NegativeShift)?,
                Ordering::Equal => match (left_type, shift_amount_type) {
                    (Integer, UnsignedInteger) => Ok(InstructionResult::Value { value: NadaValue::new_integer(left) }),
                    (UnsignedInteger, UnsignedInteger) => {
                        Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(left) })
                    }
                    (left, right) => {
                        Err(anyhow!("unsupported operands for right shift protocol: {left:?} >> {right:?}"))
                    }
                },
                Ordering::Greater => {
                    // Shift right in the clear for public variables
                    match (left_type, shift_amount_type) {
                        (Integer, UnsignedInteger) => {
                            let remainder = left.fmod(&shift_amount)?;
                            let left = left - &remainder;
                            let result = (left / &shift_amount)?;
                            Ok(InstructionResult::Value { value: NadaValue::new_integer(result) })
                        }
                        (UnsignedInteger, UnsignedInteger) => {
                            let remainder = (left % &shift_amount)?;
                            let left = left - &remainder;
                            let result = (left / &shift_amount)?;
                            Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                        }
                        (left, right) => {
                            Err(anyhow!("unsupported operands for right shift protocol: {left:?} >> {right:?}"))
                        }
                    }
                }
            }
        }
    }

    impl<T> Instruction<T> for RightShiftShares
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
            mut share_elements: Self::PreprocessingElement,
        ) -> Result<InstructionResult<Self::Router, T>, Error>
        where
            F: Instruction<T>,
        {
            let right = context.read(self.right)?;
            let left = context.read(self.left)?;

            use nada_value::NadaType::*;
            let (left_type, left) = (left.to_type(), left.try_into_value()?);
            let (shift_amount_type, shift_amount) = (right.to_type(), right.try_into_value()?);

            match shift_amount.cmp(&ModularNumber::ZERO) {
                Ordering::Less => Err(EvaluationError::NegativeShift)?,
                Ordering::Equal => match (left_type, shift_amount_type) {
                    (ShamirShareInteger, UnsignedInteger) => {
                        Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(left) })
                    }
                    (ShamirShareUnsignedInteger, UnsignedInteger) => {
                        Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(left) })
                    }
                    (left, right) => {
                        Err(anyhow!("unsupported operands for right shift protocol: {left:?} >> {right:?}"))
                    }
                },
                Ordering::Greater => match (left_type, shift_amount_type) {
                    (ty @ ShamirShareInteger, UnsignedInteger) | (ty @ ShamirShareUnsignedInteger, UnsignedInteger) => {
                        let prep_elements = share_elements.trunc.pop().ok_or_else(|| {
                            anyhow!("truncation prep element shares not found for right shift operation")
                        })?;
                        let shares = Modulo2mShares { dividend: left, divisors_exp_m: shift_amount, prep_elements };

                        let (initial_state, messages) = Modulo2mState::new(
                            vec![shares],
                            context.secret_sharer(),
                            STATISTIC_KAPPA,
                            get_statistic_k::<T>(),
                            Mod2mTruncVariant::Trunc,
                        )?;

                        Ok(InstructionResult::StateMachine {
                            state_machine: MPCInstructionRouter::RightShift(DefaultInstructionStateMachine::new(
                                initial_state,
                                ty,
                            )),
                            messages: into_instruction_messages(messages),
                        })
                    }
                    (left, right) => {
                        Err(anyhow!("unsupported operands for right shift protocol: {left:?} >> {right:?}"))
                    }
                },
            }
        }
    }

    impl From<Modulo2mStateMessage> for MPCMessages {
        fn from(message: Modulo2mStateMessage) -> Self {
            MPCMessages::RightShift(message)
        }
    }

    impl TryFrom<MPCMessages> for Modulo2mStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::RightShift(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}
