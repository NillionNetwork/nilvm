//! Execution engine virtual machine.

pub mod errors;
pub mod instructions;
pub mod memory;
pub mod plan;
pub mod sm;

use std::{collections::HashMap, sync::Arc};

use crate::{
    metrics::{ExecutionMetrics, ExecutionMetricsConfig},
    vm::{
        config::ExecutionVmConfig,
        instructions::Instruction,
        memory::RuntimeMemory,
        plan::InstructionRequirementProvider,
        sm::{ExecutionContext, VmState, VmStateMachine, VmStateMessage},
    },
};
use basic_types::{PartyId, PartyMessage};
use instant::{Duration, Instant};
use jit_compiler::Program;
use math_lib::modular::SafePrime;
use metrics::{maybe::MaybeMetric, prelude::*, Counter, Histogram};
use nada_value::{encrypted::Encrypted, NadaValue};
use once_cell::sync::Lazy;
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use state_machine::{sm::HandleOutput, state::StateMachineMessage, StateMachine, StateMachineOutput};
use uuid::Uuid;

pub mod config;

static METRICS: Lazy<VmMetrics> = Lazy::new(VmMetrics::default);

/// The program execution VM.
///
/// The state of this VM is represented by the `VmStateMachine`. Because some programs require
/// nodes to talk to each other during the execution, this VM uses the notion of "yield points":
///
/// * An instruction in the program that requires nodes to synchronize is a yield point. This
///   yields the messages that need to be delivered to other nodes to continue the execution.
/// * The end of the program is also a yield point. This yields the program's final output.
///
/// In the context of executing a program:
///
/// * Execution will always go as far as possible until hitting the next yield point.
/// * When hitting a yield point, program execution will halt until all provided information,
///   usually in the form of state machine messages, is provided. At that point, execution will again
///   continue until hitting a yield point.
pub struct ExecutionVm<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    state: VmStateMachine<I, T>,
    our_party_id: PartyId,
    started_at: Option<Instant>,
}

impl<I, T> ExecutionVm<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new executor for the given program.
    #[allow(clippy::too_many_arguments)]
    pub fn new<R>(
        compute_id: Uuid,
        config: &ExecutionVmConfig,
        program: Program<I>,
        our_party_id: PartyId,
        secret_sharer: Arc<ShamirSecretSharer<T>>,
        values: HashMap<String, NadaValue<Encrypted<T>>>,
        preprocessing_elements_provider: R,
        metrics_options: ExecutionMetricsConfig,
    ) -> Result<Self, anyhow::Error>
    where
        R: InstructionRequirementProvider<T, Instruction = I, PreprocessingElement = I::PreprocessingElement>,
    {
        let memory = RuntimeMemory::new(&program, values)?;
        let plan = config.plan_strategy.build_plan(program.body, preprocessing_elements_provider)?;
        let context = ExecutionContext::new(
            plan,
            secret_sharer,
            memory,
            config.max_protocol_messages_count,
            metrics_options,
            compute_id,
        );
        let state = StateMachine::new(VmState::new(context));
        Ok(Self { state, our_party_id, started_at: None })
    }

    /// Initializes the execution VM.
    ///
    /// This method must be called a single time before any calls to `ExecutionVm::proceed`.
    pub fn initialize(&mut self) -> Result<VmYield<I, T>, anyhow::Error> {
        self.started_at = Some(Instant::now());
        let message = PartyMessage::new(self.our_party_id.clone(), VmStateMessage::Bootstrap);
        let output = self.state.handle_message(message)?;
        Ok(self.transform_output(output))
    }

    /// Continues the execution of the VM.
    ///
    /// This picks up the execution from the last yield point, feeds the given message in, and
    /// advances as far as possible.
    pub fn proceed(
        &mut self,
        message: PartyMessage<VmStateMessage<I::Message>>,
    ) -> Result<VmYield<I, T>, anyhow::Error> {
        let output = self.state.handle_message(message)?;
        Ok(self.transform_output(output))
    }

    fn transform_output(&mut self, output: HandleOutput<VmState<I, T>>) -> VmYield<I, T> {
        use StateMachineOutput::*;
        match output {
            Messages(messages) => {
                METRICS.round_completed();
                VmYield::Messages(messages)
            }
            Final((output, metrics)) => {
                if let Some(start_time) = self.started_at.take() {
                    METRICS.observe_execution(start_time.elapsed());
                }

                VmYield::Result(output, Box::new(metrics))
            }
            Empty => VmYield::Empty,
        }
    }
}

/// The output of a VM execution until it hits a yield point.
pub enum VmYield<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// The final computation outputs.
    /// Note: MetricsResult are boxed to reduce the enum's total size (https://rust-lang.github.io/rust-clippy/master/index.html#large_enum_variant).
    Result(HashMap<String, NadaValue<Encrypted<T>>>, Box<ExecutionMetrics>),

    /// Messages that need to be delivered to other compute nodes.
    Messages(Vec<StateMachineMessage<VmState<I, T>>>),

    /// The VM didn't yield anything on this execution.
    Empty,
}

struct VmMetrics {
    execution_duration: MaybeMetric<Histogram<Duration>>,
    execution_steps: MaybeMetric<Counter>,
}

impl Default for VmMetrics {
    fn default() -> Self {
        let execution_duration = Histogram::new(
            "program_execution_duration_seconds",
            "Total time taken to execute a program",
            &[],
            TimingBuckets::sub_minute(),
        )
        .into();
        let execution_rounds =
            Counter::new("program_execution_steps_total", "Number of steps taken to execute a program", &[]).into();
        Self { execution_duration, execution_steps: execution_rounds }
    }
}

impl VmMetrics {
    fn round_completed(&self) {
        self.execution_steps.with_labels([]).inc();
    }

    fn observe_execution(&self, execution_duration: Duration) {
        self.execution_duration.with_labels([]).observe(&execution_duration);
    }
}
