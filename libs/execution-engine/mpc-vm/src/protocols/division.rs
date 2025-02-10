//! Implementation of the MPC protocols for the division operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::Division as BytecodeDivision, protocols::ExecutionLine},
    public_binary_protocol, share_binary_protocol,
};

pub(crate) struct Division;

impl Division {
    /// Transforms a bytecode division into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeDivision,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        // Check types
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        use nada_value::NadaType::*;
        match (left_type, right_type) {
            (Integer, Integer) | (UnsignedInteger, UnsignedInteger) => {
                DivisionIntegerPublic::public_protocol(context, operation)
            }
            (SecretInteger, Integer) | (SecretUnsignedInteger, UnsignedInteger) => {
                DivisionIntegerSecretDividendPublicDivisor::share_protocol(context, operation)
            }
            (&Integer, &SecretInteger)
            | (&SecretInteger, &SecretInteger)
            | (&UnsignedInteger, &SecretUnsignedInteger)
            | (&SecretUnsignedInteger, &SecretUnsignedInteger) => {
                DivisionIntegerSecretDivisor::share_protocol(context, operation)
            }
            _ => {
                let msg = format!("type {left_type} / {right_type} not supported");
                Err(Bytecode2ProtocolError::OperationNotSupported(msg))
            }
        }
    }
}

binary_protocol!(DivisionIntegerPublic, "DIVC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(DivisionIntegerPublic);
impl DivisionIntegerPublic {
    public_binary_protocol!(BytecodeDivision);
}

binary_protocol!(
    DivisionIntegerSecretDividendPublicDivisor,
    "DIVM",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::Modulo, 1)]
);
into_mpc_protocol!(DivisionIntegerSecretDividendPublicDivisor);
impl DivisionIntegerSecretDividendPublicDivisor {
    share_binary_protocol!(BytecodeDivision);
}

binary_protocol!(
    DivisionIntegerSecretDivisor,
    "DIVS",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::DivisionIntegerSecret, 1)]
);
into_mpc_protocol!(DivisionIntegerSecretDivisor);
impl DivisionIntegerSecretDivisor {
    share_binary_protocol!(BytecodeDivision);
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{DivisionIntegerPublic, DivisionIntegerSecretDividendPublicDivisor, DivisionIntegerSecretDivisor},
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
    use math_lib::modular::{FloorMod, ModularNumber, SafePrime};
    use nada_value::NadaValue;
    use protocols::division::{
        division_public_divisor::{
            DivisionIntegerPublicDivisorShares, DivisionIntegerPublicDivisorState,
            DivisionIntegerPublicDivisorStateMessage,
        },
        division_secret_divisor::{
            DivisionIntegerSecretDivisorShares, DivisionIntegerSecretDivisorState,
            DivisionIntegerSecretDivisorStateMessage,
        },
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for DivisionIntegerPublic
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

            let dividend = context.read(self.left)?;
            let divisor = context.read(self.right)?;

            let dividend_type = dividend.to_type();
            let divisor_type = divisor.to_type();

            let dividend = dividend.try_into_value()?;
            let divisor = divisor.try_into_value()?;
            if divisor == ModularNumber::ZERO {
                return Err(EvaluationError::DivByZero)?;
            }

            // Division in the clear for public variables
            match (dividend_type, divisor_type) {
                (Integer, Integer) => {
                    let remainder = dividend.fmod(&divisor)?;
                    let dividend = dividend - &remainder;
                    let result = (dividend / &divisor)?;
                    Ok(InstructionResult::Value { value: NadaValue::new_integer(result) })
                }
                (UnsignedInteger, UnsignedInteger) => {
                    let remainder = (dividend % &divisor)?;
                    let dividend = dividend - &remainder;
                    let result = (dividend / &divisor)?;
                    Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for division public protocol: {left:?} / {right:?}"))
                }
            }
        }
    }

    impl<T> Instruction<T> for DivisionIntegerSecretDividendPublicDivisor
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
            use nada_value::NadaType::*;

            let dividend = context.read(self.left)?;
            let divisor = context.read(self.right)?;

            match (dividend.to_type(), divisor.to_type()) {
                (ty @ ShamirShareInteger, Integer) | (ty @ ShamirShareUnsignedInteger, UnsignedInteger) => {
                    let dividend = dividend.try_into_value()?;
                    let divisor = divisor.try_into_value()?;
                    if divisor == ModularNumber::ZERO {
                        return Err(EvaluationError::DivByZero)?;
                    }
                    // Use Division with Integer Public divisor protocol for secret dividends.
                    let prep_elements = share_elements
                        .modulo
                        .pop()
                        .ok_or_else(|| anyhow!("shares not found for division integer with public divisor"))?;
                    let division_shares = DivisionIntegerPublicDivisorShares { dividend, divisor, prep_elements };

                    let (initial_state, messages) = DivisionIntegerPublicDivisorState::new(
                        vec![division_shares],
                        context.secret_sharer(),
                        STATISTIC_KAPPA,
                        get_statistic_k::<T>(),
                    )?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::DivisionIntegerPublicDivisor(
                            DefaultInstructionStateMachine::new(initial_state, ty),
                        ),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => Err(anyhow!(
                    "unsupported operands for secret division with public divisor protocol: {left:?} / {right:?}"
                )),
            }
        }
    }

    impl From<DivisionIntegerPublicDivisorStateMessage> for MPCMessages {
        fn from(message: DivisionIntegerPublicDivisorStateMessage) -> Self {
            MPCMessages::DivisionIntegerPublicDivisor(message)
        }
    }

    impl TryFrom<MPCMessages> for DivisionIntegerPublicDivisorStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::DivisionIntegerPublicDivisor(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }

    impl<T> Instruction<T> for DivisionIntegerSecretDivisor
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
            use nada_value::NadaType::*;

            let dividend = context.read(self.left)?;
            let divisor = context.read(self.right)?;

            match (dividend.to_type(), divisor.to_type()) {
                (ShamirShareInteger, ty @ ShamirShareInteger)
                | (Integer, ty @ ShamirShareInteger)
                | (ShamirShareUnsignedInteger, ty @ ShamirShareUnsignedInteger)
                | (UnsignedInteger, ty @ ShamirShareUnsignedInteger) => {
                    let dividend = dividend.try_into_value()?;
                    let divisor = divisor.try_into_value()?;
                    // Use Division with Integer Secret divisor protocol for secret dividends.
                    let prep_elements = share_elements
                        .division_integer_secret
                        .pop()
                        .ok_or_else(|| anyhow!("shares not found for division integer with secret divisor"))?;
                    let division_shares = DivisionIntegerSecretDivisorShares { dividend, divisor, prep_elements };
                    let (initial_state, messages) = DivisionIntegerSecretDivisorState::new(
                        vec![division_shares],
                        context.secret_sharer(),
                        STATISTIC_KAPPA,
                        get_statistic_k::<T>(),
                    )?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::DivisionIntegerSecretDivisor(
                            DefaultInstructionStateMachine::new(initial_state, ty),
                        ),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => Err(anyhow!(
                    "unsupported operands for secret division with secret divisor protocol: {left:?} / {right:?}"
                )),
            }
        }
    }

    impl From<DivisionIntegerSecretDivisorStateMessage> for MPCMessages {
        fn from(message: DivisionIntegerSecretDivisorStateMessage) -> Self {
            MPCMessages::DivisionIntegerSecretDivisor(message)
        }
    }

    impl TryFrom<MPCMessages> for DivisionIntegerSecretDivisorStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::DivisionIntegerSecretDivisor(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}
