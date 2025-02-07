//! Implementation of the MPC protocols for the addition operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::Modulo as BytecodeModulo, protocols::ExecutionLine},
    public_binary_protocol, share_binary_protocol,
};
use nada_value::NadaType::{Integer, SecretInteger, SecretUnsignedInteger, UnsignedInteger};

pub(crate) struct Modulo;

impl Modulo {
    /// Transforms a bytecode modulo into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeModulo,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        // Check types
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        match (left_type, right_type) {
            (Integer, Integer) | (UnsignedInteger, UnsignedInteger) => {
                ModuloIntegerPublic::public_protocol(context, operation)
            }
            (SecretInteger, Integer) | (SecretUnsignedInteger, UnsignedInteger) => {
                ModuloIntegerSecretDividendPublicDivisor::share_protocol(context, operation)
            }
            (&Integer, &SecretInteger)
            | (&SecretInteger, &SecretInteger)
            | (&UnsignedInteger, &SecretUnsignedInteger)
            | (&SecretUnsignedInteger, &SecretUnsignedInteger) => {
                ModuloIntegerSecretDivisor::share_protocol(context, operation)
            }
            _ => {
                let msg = format!("type {left_type} mod {right_type} not supported");
                Err(Bytecode2ProtocolError::OperationNotSupported(msg))
            }
        }
    }
}

binary_protocol!(ModuloIntegerPublic, "MODC", ExecutionLine::Local, RuntimeRequirementType);
into_mpc_protocol!(ModuloIntegerPublic);
impl ModuloIntegerPublic {
    public_binary_protocol!(BytecodeModulo);
}

binary_protocol!(
    ModuloIntegerSecretDividendPublicDivisor,
    "MODM",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::Modulo, 1)]
);
into_mpc_protocol!(ModuloIntegerSecretDividendPublicDivisor);
impl ModuloIntegerSecretDividendPublicDivisor {
    share_binary_protocol!(BytecodeModulo);
}

binary_protocol!(
    ModuloIntegerSecretDivisor,
    "MODM",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::DivisionIntegerSecret, 1)]
);
into_mpc_protocol!(ModuloIntegerSecretDivisor);
impl ModuloIntegerSecretDivisor {
    share_binary_protocol!(BytecodeModulo);
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::{ModuloIntegerPublic, ModuloIntegerSecretDividendPublicDivisor, ModuloIntegerSecretDivisor},
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
        modulo_public_divisor::{ModuloShares, ModuloState, ModuloStateMessage},
        modulo_secret_divisor::{
            ModuloIntegerSecretDivisorShares, ModuloIntegerSecretDivisorState, ModuloIntegerSecretDivisorStateMessage,
        },
    };
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

    impl<T> Instruction<T> for ModuloIntegerPublic
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

            // Modulo in the clear for public variables
            match (dividend_type, divisor_type) {
                (Integer, Integer) => {
                    // Note: fmod implements modulo with floor division. This approach differs from standard rust
                    // mod for negative numbers. Therefore, for signed integers we use fmod instead of the % operator
                    let result = dividend.fmod(&divisor)?;
                    Ok(InstructionResult::Value { value: NadaValue::new_integer(result) })
                }
                (UnsignedInteger, UnsignedInteger) => {
                    // Note: for unsigned integers we use % instead of fmod because fmod assumes signed numbers.
                    let result = (dividend % &divisor)?;
                    Ok(InstructionResult::Value { value: NadaValue::new_unsigned_integer(result) })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for modulo public protocol: {left:?} mod {right:?}"))
                }
            }
        }
    }

    impl<T> Instruction<T> for ModuloIntegerSecretDividendPublicDivisor
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
                        .ok_or_else(|| anyhow!("shares not found for modulo integer with public divisor"))?;
                    let modulo_shares = ModuloShares { dividend, divisor, prep_elements };
                    let (initial_state, messages) = ModuloState::new(
                        vec![modulo_shares],
                        context.secret_sharer(),
                        STATISTIC_KAPPA,
                        get_statistic_k::<T>(),
                    )?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::Modulo(DefaultInstructionStateMachine::new(
                            initial_state,
                            ty,
                        )),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => {
                    Err(anyhow!("unsupported operands for modulo with public divisor protocol: {left:?} mod {right:?}"))
                }
            }
        }
    }

    impl From<ModuloStateMessage> for MPCMessages {
        fn from(message: ModuloStateMessage) -> Self {
            MPCMessages::Modulo(message)
        }
    }

    impl TryFrom<MPCMessages> for ModuloStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::Modulo(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }

    impl<T> Instruction<T> for ModuloIntegerSecretDivisor
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
                    // Use Modulo with Integer Secret divisor protocol for secret dividends.
                    let prep_elements = share_elements.division_integer_secret.pop().ok_or_else(|| {
                        anyhow!("division with secret divisor prep element shares not found for modulo operation")
                    })?;

                    let dividend = dividend.try_into_value()?;
                    let divisor = divisor.try_into_value()?;
                    let modulo_shares = ModuloIntegerSecretDivisorShares { dividend, divisor, prep_elements };

                    let (initial_state, messages) = ModuloIntegerSecretDivisorState::new(
                        vec![modulo_shares],
                        context.secret_sharer(),
                        STATISTIC_KAPPA,
                        get_statistic_k::<T>(),
                    )?;

                    Ok(InstructionResult::StateMachine {
                        state_machine: MPCInstructionRouter::ModuloIntegerSecretDivisor(
                            DefaultInstructionStateMachine::new(initial_state, ty),
                        ),
                        messages: into_instruction_messages(messages),
                    })
                }
                (left, right) => Err(anyhow!(
                    "unsupported operands for secret modulo with secret divisor protocol: {left:?} mod {right:?}"
                )),
            }
        }
    }

    impl From<ModuloIntegerSecretDivisorStateMessage> for MPCMessages {
        fn from(message: ModuloIntegerSecretDivisorStateMessage) -> Self {
            MPCMessages::ModuloIntegerSecretDivisor(message)
        }
    }

    impl TryFrom<MPCMessages> for ModuloIntegerSecretDivisorStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::ModuloIntegerSecretDivisor(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}
