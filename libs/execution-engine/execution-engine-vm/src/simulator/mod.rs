//! Program simulation utilities.

pub mod inputs;

use crate::{
    metrics::{ExecutionMetrics, ExecutionMetricsConfig},
    simulator::inputs::{InputGenerator, ProgramInputs},
    vm::{
        config::ExecutionVmConfig, instructions::Instruction, plan::InstructionRequirementProvider, sm::VmStateMessage,
        ExecutionVm, VmYield,
    },
};
use anyhow::{anyhow, Error};
use basic_types::{jar::PartyJar, PartyMessage};
use jit_compiler::Program;
use math_lib::modular::SafePrime;
use nada_value::{
    clear::Clear,
    encoders::EncodableWithP,
    encrypted::{nada_values_encrypted_to_nada_values_clear, Encrypted},
    NadaValue,
};
use shamir_sharing::{
    party::PartyId,
    secret_sharer::{PartyShares, SafePrimeSecretSharer, SecretSharerProperties, ShamirSecretSharer},
};
use state_machine::state::{Recipient, RecipientMessage};
use std::{collections::HashMap, fmt::Debug, sync::Arc, time::Instant};
use uuid::Uuid;

/// Implement a program that can be simulated
pub trait SimulatableProgram<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Provider type is built
    type Provider: InstructionRequirementProvider<T, Instruction = I, PreprocessingElement = I::PreprocessingElement>
        + Default;

    /// Build the requirements provider
    fn build_requirements_provider(
        &self,
        sharer: &ShamirSecretSharer<T>,
    ) -> Result<HashMap<PartyId, Self::Provider>, Error>;
}

/// Simulates the execution of a program.
pub struct ProgramSimulator<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    vms: HashMap<PartyId, ExecutionVm<I, T>>,
    sharer: ShamirSecretSharer<T>,
}

impl<I, T> ProgramSimulator<I, T>
where
    I: Instruction<T>,
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    /// Construct a new simulator for the given program.
    pub fn new(
        program: Program<I>,
        parameters: SimulationParameters,
        input_generator: &InputGenerator,
        metrics_config: ExecutionMetricsConfig,
    ) -> Result<Self, Error>
    where
        Program<I>: SimulatableProgram<I, T>,
    {
        let sharers = Self::create_sharers(&parameters)?;
        // We just need _some_ sharer to generate the inputs.
        let some_sharer = sharers.iter().next().ok_or_else(|| anyhow!("no sharers created"))?.1.clone();
        let inputs = ProgramInputs::<T>::from_program(&program, input_generator, &some_sharer)?;
        let vms =
            Self::create_vms(parameters.execution_vm_config, program, sharers, &some_sharer, inputs, metrics_config)?;
        Ok(Self { vms, sharer: some_sharer })
    }

    /// Run the program in all the node vms and returns the final output.
    pub fn run(self) -> Result<(HashMap<String, NadaValue<Clear>>, ExecutionMetrics), Error> {
        let start_time = Instant::now();
        let mut vms = self.vms;
        let mut party_output = Self::run_iteration(&mut vms, |_, vm| vm.initialize())?;
        loop {
            let mut message_jar = MessageJar::default();
            let mut outputs_party_shares: PartyShares<HashMap<String, NadaValue<Encrypted<T>>>> =
                PartyShares::default();
            let mut metrics = Vec::new();
            for (party_id, output) in party_output {
                match output {
                    VmYield::Result(result, metrics_result) => {
                        for (output_name, share) in result {
                            let party_shares = outputs_party_shares.entry(party_id.clone()).or_default();
                            party_shares.insert(output_name, share);
                        }
                        metrics.push(*metrics_result);
                    }
                    VmYield::Messages(messages) => {
                        let messages = messages.into_iter().map(|message| PartyMessage::new(party_id.clone(), message));
                        message_jar.add(messages);
                    }
                    VmYield::Empty => (),
                }
            }
            if !outputs_party_shares.is_empty() {
                let mut party_jar = PartyJar::new(self.sharer.party_count());
                // TODO 'nada_values_encrypted_to_nada_values_clear' doesn't accept NadaValue<Encrypted<T>>.
                //  It should accept this type instead of NadaValue<Encrypted<Encoded>> as parameter.
                //  For now, we need to encode the values.
                for (party, party_shares) in outputs_party_shares {
                    party_jar.add_element(party, party_shares.encode()?)?;
                }
                let mut metrics = ExecutionMetrics::merge(metrics)
                    .ok_or_else(|| anyhow!("expected to have at least one metrics result"))?;
                metrics.summary.execution_duration = start_time.elapsed();
                return Ok((nada_values_encrypted_to_nada_values_clear(party_jar, &self.sharer)?, metrics));
            } else if !message_jar.is_empty() {
                party_output = Self::run_iteration(&mut vms, |party_id, vm| message_jar.forward(party_id, vm))?;
            } else {
                return Err(anyhow!("completed round without any messages"))?;
            }
        }
    }

    fn run_iteration<F>(
        vms: &mut HashMap<PartyId, ExecutionVm<I, T>>,
        mut runner: F,
    ) -> Result<HashMap<PartyId, VmYield<I, T>>, Error>
    where
        F: FnMut(&PartyId, &mut ExecutionVm<I, T>) -> Result<VmYield<I, T>, Error>,
    {
        let mut party_output = HashMap::new();
        for (party_id, vm) in vms {
            let output = runner(party_id, vm)?;
            party_output.insert(party_id.clone(), output);
        }
        Ok(party_output)
    }

    fn create_sharers(parameters: &SimulationParameters) -> Result<HashMap<PartyId, ShamirSecretSharer<T>>, Error> {
        let parties: Vec<_> = (0..parameters.network_size).map(|_| PartyId::from(Uuid::new_v4())).collect();
        let mut sharers = HashMap::new();
        for party_id in &parties {
            let sharer = ShamirSecretSharer::new(party_id.clone(), parameters.polynomial_degree, parties.clone())?;
            sharers.insert(party_id.clone(), sharer);
        }
        Ok(sharers)
    }

    fn create_vms(
        execution_vm_config: ExecutionVmConfig,
        program: Program<I>,
        sharers: HashMap<PartyId, ShamirSecretSharer<T>>,
        sharer: &ShamirSecretSharer<T>,
        mut inputs: ProgramInputs<T>,
        metrics_config: ExecutionMetricsConfig,
    ) -> Result<HashMap<PartyId, ExecutionVm<I, T>>, Error>
    where
        Program<I>: SimulatableProgram<I, T>,
    {
        let mut requirements = program.build_requirements_provider(sharer)?;
        let mut vms = HashMap::new();
        let compute_id = Uuid::new_v4();
        for (party_id, sharer) in sharers {
            let runtime_elements = requirements.remove(&party_id).unwrap_or_default();
            let vm = Self::create_vm(
                compute_id,
                &execution_vm_config,
                program.clone(),
                party_id.clone(),
                sharer,
                runtime_elements,
                &mut inputs,
                metrics_config,
            )?;
            vms.insert(party_id, vm);
        }
        Ok(vms)
    }

    #[allow(clippy::too_many_arguments)]
    fn create_vm<R>(
        compute_id: Uuid,
        config: &ExecutionVmConfig,
        program: Program<I>,
        party_id: PartyId,
        sharer: ShamirSecretSharer<T>,
        runtime_elements: R,
        inputs: &mut ProgramInputs<T>,
        metrics_options: ExecutionMetricsConfig,
    ) -> Result<ExecutionVm<I, T>, Error>
    where
        R: InstructionRequirementProvider<T, Instruction = I, PreprocessingElement = I::PreprocessingElement>,
    {
        let sharer = Arc::new(sharer);
        let party_inputs = inputs.party_inputs.remove(&party_id).unwrap_or_default();
        ExecutionVm::new(compute_id, config, program, party_id, sharer, party_inputs, runtime_elements, metrics_options)
    }
}

/// The parameters for a simulation.
#[derive(Clone, Debug)]
pub struct SimulationParameters {
    /// The size of the network.
    pub network_size: usize,

    /// The degree of the polynomial to be used.
    pub polynomial_degree: u64,

    /// Execution engine configuration properties
    pub execution_vm_config: ExecutionVmConfig,
}

struct MessageJar<M: Clone + Debug> {
    messages: HashMap<PartyId, Vec<PartyMessage<VmStateMessage<M>>>>,
}

impl<M: Clone + Debug> Default for MessageJar<M> {
    fn default() -> Self {
        Self { messages: HashMap::new() }
    }
}

impl<M: Clone + Debug> MessageJar<M> {
    fn add<I>(&mut self, messages: I)
    where
        I: IntoIterator<Item = PartyMessage<RecipientMessage<PartyId, VmStateMessage<M>>>>,
    {
        for message in messages {
            let (sender_party_id, message) = message.into_parts();
            let (recipient, message) = message.into_parts();
            let parties = match recipient {
                Recipient::Single(party_id) => vec![party_id],
                Recipient::Multiple(parties) => parties,
            };
            for party_id in parties {
                self.messages
                    .entry(party_id)
                    .or_default()
                    .push(PartyMessage::new(sender_party_id.clone(), message.clone()));
            }
        }
    }

    fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn forward<I, T>(&mut self, party_id: &PartyId, vm: &mut ExecutionVm<I, T>) -> Result<VmYield<I, T>, Error>
    where
        I: Instruction<T, Message = M>,
        T: SafePrime,
        ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
    {
        let messages = match self.messages.remove(party_id) {
            Some(messages) => messages,
            None => return Ok(VmYield::Empty),
        };
        let mut vm_yield = VmYield::Empty;
        for message in messages {
            vm_yield = match (vm_yield, vm.proceed(message)?) {
                (VmYield::Messages(existing), VmYield::Messages(new)) => {
                    VmYield::Messages(existing.into_iter().chain(new.into_iter()).collect())
                }
                (messages @ VmYield::Messages(_), VmYield::Empty) => messages,
                (_, new) => new,
            };
        }
        Ok(vm_yield)
    }
}
