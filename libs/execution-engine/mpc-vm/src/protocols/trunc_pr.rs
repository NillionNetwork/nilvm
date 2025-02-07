//! Implementation of the MPC protocols for the trunc-pr operation

use crate::{protocols::MPCProtocol, requirements::RuntimeRequirementType, utils::into_mpc_protocol};
use jit_compiler::{
    binary_protocol,
    bytecode2protocol::{errors::Bytecode2ProtocolError, Bytecode2ProtocolContext, ProtocolFactory},
    models::{bytecode::TruncPr as BytecodeTruncPr, protocols::ExecutionLine},
    share_shift_protocol,
};

// TruncPr protocol
binary_protocol!(
    TruncPr,
    "TruncPr",
    ExecutionLine::Online,
    RuntimeRequirementType,
    &[(RuntimeRequirementType::TruncPr, 1)]
);
into_mpc_protocol!(TruncPr);
impl TruncPr {
    share_shift_protocol!(BytecodeTruncPr);

    /// Transforms a bytecode truncation into a protocol
    pub(crate) fn transform<F: ProtocolFactory<MPCProtocol>>(
        context: &mut Bytecode2ProtocolContext<MPCProtocol, F>,
        operation: &BytecodeTruncPr,
    ) -> Result<MPCProtocol, Bytecode2ProtocolError> {
        // Check types
        let right_type = context.bytecode.memory_element_type(operation.right)?;
        if !right_type.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "The amount of probabilistic truncation should be public. {} not supported",
                right_type
            )));
        }
        let left_type = context.bytecode.memory_element_type(operation.left)?;
        if left_type.is_public() {
            return Err(Bytecode2ProtocolError::OperationNotSupported(format!(
                "Public left type for probabilistic truncation detected, use '>>' instead. {} not supported",
                right_type
            )));
        }
        Self::share_protocol(context, operation)
    }
}

#[cfg(any(test, feature = "vm"))]
pub(crate) mod vm {
    use crate::{
        protocols::TruncPr,
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
    use math_lib::modular::{ModularNumber, SafePrime};
    use nada_value::NadaValue;
    use protocols::division::truncation_probabilistic::{TruncPrShares, TruncPrState, TruncPrStateMessage};
    use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
    use std::cmp::Ordering;

    impl<T> Instruction<T> for TruncPr
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

            // Parse the type and the primitive value of each of the operands.
            use nada_value::NadaType::*;
            let (left_type, left) = (left.to_type(), left.try_into_value()?);
            let (shift_amount_type, shift_amount) = (right.to_type(), right.try_into_value()?);

            // Check the shift amount.
            match shift_amount.cmp(&ModularNumber::ZERO) {
                Ordering::Less => Err(EvaluationError::NegativeShift)?,
                Ordering::Equal => match (left_type, shift_amount_type) {
                    (ShamirShareInteger, Integer) | (ShamirShareInteger, UnsignedInteger) => {
                        Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_integer(left) })
                    }
                    (ShamirShareUnsignedInteger, Integer) | (ShamirShareUnsignedInteger, UnsignedInteger) => {
                        Ok(InstructionResult::Value { value: NadaValue::new_shamir_share_unsigned_integer(left) })
                    }
                    (left, right) => Err(anyhow!(
                        "unsupported operands for probabilistic truncation protocol: {left:?} >> {right:?}"
                    )),
                },
                Ordering::Greater => match (left_type, shift_amount_type) {
                    (ty @ ShamirShareInteger, Integer)
                    | (ty @ ShamirShareInteger, UnsignedInteger)
                    | (ty @ ShamirShareUnsignedInteger, Integer)
                    | (ty @ ShamirShareUnsignedInteger, UnsignedInteger) => {
                        let prep_elements = share_elements.truncpr.pop().ok_or_else(|| {
                            anyhow!("probabilistic truncation prep element shares not found for trunc_pr operation")
                        })?;
                        let shares = TruncPrShares { dividend: left, divisors_exp_m: shift_amount, prep_elements };

                        let (initial_state, messages) = TruncPrState::new(
                            vec![shares],
                            context.secret_sharer(),
                            STATISTIC_KAPPA,
                            get_statistic_k::<T>(),
                        )?;

                        Ok(InstructionResult::StateMachine {
                            state_machine: MPCInstructionRouter::TruncPr(DefaultInstructionStateMachine::new(
                                initial_state,
                                ty,
                            )),
                            messages: into_instruction_messages(messages),
                        })
                    }
                    (left, right) => Err(anyhow!(
                        "unsupported operands for probabilistic truncation protocol: {left:?} >> {right:?}"
                    )),
                },
            }
        }
    }

    impl From<TruncPrStateMessage> for MPCMessages {
        fn from(message: TruncPrStateMessage) -> Self {
            MPCMessages::TruncPr(message)
        }
    }

    impl TryFrom<MPCMessages> for TruncPrStateMessage {
        type Error = Error;

        fn try_from(msg: MPCMessages) -> Result<Self, Self::Error> {
            let MPCMessages::TruncPr(msg) = msg else {
                return Err(anyhow!("unknown instruction message"));
            };
            Ok(msg)
        }
    }
}
