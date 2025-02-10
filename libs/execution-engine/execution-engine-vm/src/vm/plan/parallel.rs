//! Provide a plan implementation to perform a parallel execution

use crate::vm::plan::{ExecutionPlan, ExecutionStepId, InstructionRequirementProvider, PlanCreateError};
use jit_compiler::models::{
    memory::AddressType,
    protocols::{memory::ProtocolAddress, ExecutionLine, Protocol, ProtocolDependencies, ProtocolsModel},
};
use math_lib::modular::SafePrime;
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use std::collections::HashMap;

/// Creates the execution steps following the parallel strategy
pub fn parallel_plan<R, T>(
    program: ProtocolsModel<R::Instruction>,
    mut preprocessing_elements: R,
) -> Result<ExecutionPlan<R::Instruction, T>, PlanCreateError>
where
    R: InstructionRequirementProvider<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let mut execution_plan = ExecutionPlan::default();
    let mut step_index: HashMap<ProtocolAddress, ExecutionStepId> = HashMap::new();
    // The inputs and literals are also resolved when we start the execution. We don't need to
    // execute any protocol, we only need to index them to mark that the dependencies to them are
    // resolved.
    for (mut address, input) in program.input_memory_scheme {
        for _ in 0..input.sizeof {
            step_index.insert(address, ExecutionStepId::default());
            address = address.next()?;
        }
    }
    for (address, _) in program.literals.iter().enumerate() {
        let address = ProtocolAddress::new(address, AddressType::Literals);
        step_index.insert(address, ExecutionStepId::default());
    }
    for protocol in program.protocols.into_values() {
        // Calculates the execution step when the protocol will be executed by the execution engine.
        let step_id = ExecutionStepId {
            index: protocol_steps(&protocol.dependencies(), &step_index)?,
            execution_line: protocol.execution_line(),
        };
        // Indexes the execution step when the protocols are resolved.
        step_index.insert(protocol.address(), step_id);
        // Insert the protocol in the plan.
        execution_plan.insert_protocol(step_id, protocol, &mut preprocessing_elements)?;
    }
    Ok(execution_plan)
}

/// Calculates the index of an execution step from the dependencies of a protocol.
fn protocol_steps<'a, I>(
    dependencies: I,
    protocol_steps: &HashMap<ProtocolAddress, ExecutionStepId>,
) -> Result<usize, PlanCreateError>
where
    I: IntoIterator<Item = &'a ProtocolAddress>,
{
    let mut later_step = ExecutionStepId::default();
    // We have to traverse all dependencies and get the execution steps when they are resolved. We want to
    // find the later execution step where a dependency is resolved.
    for dependency in dependencies {
        let dependency_step = protocol_steps.get(dependency).ok_or(PlanCreateError::ProtocolNotFound)?;
        match later_step.execution_line {
            // If the execution line of the later execution step is Local we can replace the
            // later execution step if the dependency execution step is greater or equal than
            // the later execution step.
            ExecutionLine::Local if dependency_step.index >= later_step.index => later_step = *dependency_step,
            // If the execution line of the later execution step is Online we replace the later
            // execution step only if the dependency execution step is greater than the later
            // execution step. In other case, we could replace an online execution step with a
            // local and the execution would fail.
            ExecutionLine::Online if dependency_step.index > later_step.index => later_step = *dependency_step,
            _ => {}
        }
    }
    match later_step.execution_line {
        // If the later execution step is local, we can execute the protocol in the same execution
        // step, independent if it is an online or a local protocol
        ExecutionLine::Local => Ok(later_step.index),
        // If the later execution step is online, we must wait the later execution step finished to
        // have the result.
        ExecutionLine::Online => Ok(later_step.index.wrapping_add(1)),
    }
}
