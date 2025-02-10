//! Program execution planning.

pub mod parallel;
pub mod sequential;

use crate::vm::{
    instructions::Instruction,
    plan::{parallel::parallel_plan, sequential::sequential_plan},
};
use jit_compiler::models::protocols::{memory::ProtocolMemoryError, ExecutionLine, Protocol, ProtocolsModel};
use math_lib::modular::{DecodeError, SafePrime};
use serde::{Deserialize, Serialize};
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use std::marker::PhantomData;

/// Define the strategy that will be used to create the execution plan
#[derive(Default, Clone, Debug, Deserialize, Serialize)]
pub enum PlanStrategy {
    #[default]
    /// The protocols will be executed sequentially, following the order in which they are defined
    /// in the program.
    Sequential = 0,
    /// The protocols will be relocated depending on when their dependencies are resolved.
    /// This strategy tries to minimize the online execution steps sending more than one protocol
    /// message in the same round of communication.
    Parallel = 1,
}

impl PlanStrategy {
    /// Creates the execution steps depending on the strategy
    pub fn build_plan<R, T>(
        &self,
        program: ProtocolsModel<R::Instruction>,
        preprocessing_elements: R,
    ) -> Result<ExecutionPlan<R::Instruction, T>, PlanCreateError>
    where
        R: InstructionRequirementProvider<T>,
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        match self {
            PlanStrategy::Sequential => sequential_plan(program, preprocessing_elements),
            PlanStrategy::Parallel => parallel_plan(program, preprocessing_elements),
        }
    }

    /// Creates the execution steps depending on the strategy without preprocessing elements.
    /// Note: This plan is not executable and it should be use only for debugging reasons
    #[cfg(any(test, feature = "text_repr"))]
    pub fn build_plan_without_preprocessing_elements<I, T>(
        &self,
        program: ProtocolsModel<I>,
    ) -> Result<ExecutionPlan<I, T>, PlanCreateError>
    where
        I: Instruction<T>,
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        let provider = DummyProtocolRequirementProvider::<I, T>::default();
        match self {
            PlanStrategy::Sequential => sequential_plan(program, provider),
            PlanStrategy::Parallel => parallel_plan(program, provider),
        }
    }
}

/// Provides to the instruction the preprocessing elements it requires
pub trait InstructionRequirementProvider<T>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Preprocessing elements that will be provided
    type PreprocessingElement: Default;

    /// Type of the instruction that knows the provider
    type Instruction: Instruction<T, PreprocessingElement = Self::PreprocessingElement>;

    /// Provides the preprocessing elements to the instruction
    fn take(&mut self, instruction: &Self::Instruction) -> Result<Self::PreprocessingElement, PlanCreateError>;
}

/// Protocol requirement provider that simulates returns the requirements
pub struct DummyProtocolRequirementProvider<P, T>
where
    P: Protocol,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    _unused: PhantomData<(P, T)>,
}

impl<P, T> Default for DummyProtocolRequirementProvider<P, T>
where
    P: Protocol,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    fn default() -> Self {
        Self { _unused: PhantomData }
    }
}

impl<I, T> InstructionRequirementProvider<T> for DummyProtocolRequirementProvider<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    type PreprocessingElement = I::PreprocessingElement;
    type Instruction = I;

    fn take(&mut self, _: &Self::Instruction) -> Result<Self::PreprocessingElement, PlanCreateError> {
        Ok(Self::PreprocessingElement::default())
    }
}

/// Represents a group of protocols which inputs have already resolved.
/// An execution step is split into two execution line:
/// - Firstly, the execution engine executes all protocols and don't need communication for it. These
///   are the local protocols.
/// - When the local protocol have been executed, the execution engine executes the online protocols,
///   that are all protocols that can be executed, but they need communication.
#[derive(Debug)]
pub struct ExecutionStep<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Local protocols that will be executed in this execution step
    pub local: Vec<I>,
    /// Online protocols that will be executed in this execution step
    pub online: Vec<(I, I::PreprocessingElement)>,
    _prime: PhantomData<T>,
}

#[cfg(feature = "text_repr")]
impl<I, T> ExecutionStep<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Returns the text representation of an ExecutionStep
    pub fn text_repr(&self, program: &ProtocolsModel<I>, step_index: usize) -> String {
        let mut repr = format!("Execution step {step_index} [Local]:\n");
        for protocol in self.local.iter() {
            repr.push_str(&format!("\t{}\n", protocol.text_repr(program)));
        }
        repr.push_str(&format!("Execution step {step_index} [Online]:\n"));
        for (protocol, _) in self.online.iter() {
            repr.push_str(&format!("\t{}\n", protocol.text_repr(program)));
        }
        repr
    }
}

impl<I, T> Default for ExecutionStep<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    fn default() -> Self {
        Self { local: vec![], online: vec![], _prime: PhantomData }
    }
}

/// Represents the execution step when a protocol will be executed.
#[derive(Ord, PartialOrd, Eq, PartialEq, Copy, Clone, Default)]
pub struct ExecutionStepId {
    /// Identifies the position of an execution step in the plan
    index: usize,
    /// Represents the execution line in an execution step
    execution_line: ExecutionLine,
}

/// A plan is the sequence of steps in which the program execution is split. The execution engine
/// will execute them sequentially, and it cannot execute an execution step if the previous one hasn't finished.
#[derive(Debug)]
pub struct ExecutionPlan<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Steps in which a program execution is split.
    pub steps: Vec<ExecutionStep<I, T>>,
}

impl<I, T> Default for ExecutionPlan<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    fn default() -> Self {
        Self { steps: vec![] }
    }
}

impl<I, T> ExecutionPlan<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Inserts a protocol into the plan
    pub(crate) fn insert_protocol<R>(
        &mut self,
        id: ExecutionStepId,
        protocol: I,
        protocol_requirement_provider: &mut R,
    ) -> Result<(), PlanCreateError>
    where
        R: InstructionRequirementProvider<T, Instruction = I, PreprocessingElement = I::PreprocessingElement>,
    {
        // Create the execution step if it is not created.
        if id.index >= self.steps.len() {
            self.steps.push(ExecutionStep::default());
        }
        let step: &mut ExecutionStep<I, T> = self.steps.get_mut(id.index).ok_or(PlanCreateError::StepCreation)?;
        // Insert the protocol in the defined execution step of the plan.
        match id.execution_line {
            ExecutionLine::Local => step.local.push(protocol),
            ExecutionLine::Online => {
                let elements = protocol_requirement_provider.take(&protocol)?;
                step.online.push((protocol, elements))
            }
        };
        Ok(())
    }

    pub(crate) fn next_step(&mut self) -> Option<ExecutionStep<I, T>> {
        self.steps.pop()
    }

    pub(crate) fn reverse(mut self) -> Self {
        self.steps.reverse();
        self
    }

    #[cfg(feature = "text_repr")]
    /// Returns the text representation of an ExecutionPlan
    pub fn text_repr(&self, program: &ProtocolsModel<I>) -> String {
        let mut repr = String::from("Execution plan:\n");
        for (index, execution_step) in self.steps.iter().enumerate() {
            repr.push_str(&execution_step.text_repr(program, index));
        }
        repr
    }
}

/// An error during the execution plan creation.
#[derive(Debug, thiserror::Error)]
pub enum PlanCreateError {
    /// Not enough preprocessing elements of the given type were provided.
    #[error("not enough {0} elements")]
    NotEnoughElements(&'static str),

    /// An error when decoding elements.
    #[error("decoding: {0}")]
    Decode(#[from] DecodeError),

    /// Protocol not found.
    #[error("execution step calculation failed: protocol not found")]
    ProtocolNotFound,

    /// Protocol not found.
    #[error("execution step calculation failed: execution step creation failed")]
    StepCreation,

    /// Protocol memory error.
    #[error("execution step calculation failed: {0}")]
    ProtocolMemory(#[from] ProtocolMemoryError),
}
