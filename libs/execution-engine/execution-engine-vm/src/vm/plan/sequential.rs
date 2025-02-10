//! Provide a plan implementation to perform a sequential execution

use crate::vm::plan::{ExecutionPlan, ExecutionStepId, InstructionRequirementProvider, PlanCreateError};
use jit_compiler::models::protocols::{Protocol, ProtocolsModel};
use math_lib::modular::SafePrime;
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};

/// Creates the execution steps following the sequential strategy
pub fn sequential_plan<R, T>(
    program: ProtocolsModel<R::Instruction>,
    mut preprocessing_elements: R,
) -> Result<ExecutionPlan<R::Instruction, T>, PlanCreateError>
where
    R: InstructionRequirementProvider<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let mut execution_plan = ExecutionPlan::default();
    for (index, protocol) in program.protocols.into_values().enumerate() {
        let step_id = ExecutionStepId { index, execution_line: protocol.execution_line() };
        execution_plan.insert_protocol(step_id, protocol, &mut preprocessing_elements)?;
    }
    Ok(execution_plan)
}
