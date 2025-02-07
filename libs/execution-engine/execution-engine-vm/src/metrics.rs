//! Protocols metrics

use crate::vm::instructions::InstructionMessage;
use anyhow::{anyhow, Result};
use bincode::Options;
use clap::ValueEnum;
use encoding::codec::MessageCodec;
use humansize::{format_size, DECIMAL};
use humantime::format_duration;
use indexmap::IndexMap;
use instant::{Duration, Instant};
use jit_compiler::models::protocols::{memory::ProtocolAddress, Protocol};
use log::warn;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display, Formatter},
    fs,
};

/// Metrics options.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ExecutionMetricsConfig {
    /// Mark as enable the metrics calculation
    enable: bool,
    /// Mark as enable the message size metrics calculation
    enable_message_size_calculation: bool,
    /// Mark as enable the execution plan metrics report
    enable_execution_plan_metrics: bool,
}

impl ExecutionMetricsConfig {
    /// Disable metrics.
    pub fn disabled() -> Self {
        Self { enable: false, enable_message_size_calculation: false, enable_execution_plan_metrics: false }
    }

    /// Enable metrics, may also enable network message size calculation and detailed execution plan metrics.
    /// Note that message size calculation requires calling bincode's serialized_size function on each round message sent
    /// by the protocols which has an impact on performance, so this should only be enabled when needed.
    /// Preferably in a simulated environment.
    /// The execution plan metrics are calculated always, because the computation is required by the
    /// computation of the summary. The activation affects to the final report only.
    pub fn enabled(enable_message_size_calculation: bool, enable_execution_plan_metrics: bool) -> Self {
        Self { enable: true, enable_message_size_calculation, enable_execution_plan_metrics }
    }
}

/// Total, minimum and maximum durations.
#[derive(Clone, Debug, Serialize)]
pub struct MinMaxDuration {
    /// Total duration.
    pub total: Duration,

    /// Minimum duration.
    pub min: Duration,

    /// Maximum duration;
    pub max: Duration,
}

impl Default for MinMaxDuration {
    fn default() -> Self {
        Self { total: Default::default(), min: Duration::MAX, max: Default::default() }
    }
}

impl MinMaxDuration {
    fn update(&self, duration: Duration) -> Self {
        Self { total: self.total.saturating_add(duration), min: self.min.min(duration), max: self.max.max(duration) }
    }
}

impl Display for MinMaxDuration {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "total duration: {} (min: {}, max: {})",
            format_duration(self.total),
            format_duration(self.min),
            format_duration(self.max)
        )?;
        Ok(())
    }
}

/// Execution VM metrics.
#[derive(Clone, Debug, Serialize)]
pub struct ExecutionMetrics {
    #[serde(skip_serializing)]
    config: ExecutionMetricsConfig,
    /// Summary of the execution plan metrics
    pub summary: ExecutionPlanSummary,
    /// Execution plan metrics in detail
    pub steps: Vec<StepMetrics>,
}

impl ExecutionMetrics {
    pub(crate) fn new(config: ExecutionMetricsConfig) -> Self {
        Self { summary: ExecutionPlanSummary::new(config.enable_message_size_calculation), config, steps: Vec::new() }
    }

    /// The execution of a plan has started
    pub(crate) fn execution_started(&mut self) {
        if !self.config.enable {
            return;
        }
        self.summary.execution_started();
    }

    /// Updates the compute duration.
    pub(crate) fn update_compute_duration(&mut self) {
        if !self.config.enable {
            return;
        }
        self.summary.update_compute_duration();
    }

    /// Add a metrics for a new step in the plan
    pub(crate) fn add_new_step(&mut self) {
        if !self.config.enable {
            return;
        }
        self.steps.push(StepMetrics::new(self.steps.len()));
        self.summary.total_execution_steps = self.summary.total_execution_steps.saturating_add(1);
    }

    /// Add the metrics for a new local protocol in the last step has been added
    pub(crate) fn local_protocol_started(&mut self, protocol: &impl Protocol) {
        if !self.config.enable {
            return;
        }
        let Some(step) = self.steps.last_mut() else {
            warn!("Execution Metrics: execution plan is empty");
            return;
        };
        let mut protocol_metrics = ProtocolMetrics::new(protocol, step.index);
        protocol_metrics.start_duration_calculation();
        self.summary
            .local_protocols
            .entry(protocol.name())
            .or_insert(ProtocolVariantMetrics::new(protocol.name()))
            .increment_call_count();
        step.protocols.insert(protocol.address().to_string(), protocol_metrics);
    }

    /// Add the metrics for a new online protocol in the last step has been added
    pub(crate) fn online_protocol_started(&mut self, protocol: &impl Protocol) {
        if !self.config.enable {
            return;
        }
        let Some(step) = self.steps.last_mut() else {
            warn!("Execution Metrics: execution plan is empty");
            return;
        };
        let mut protocol_metrics = ProtocolMetrics::new(protocol, step.index);
        protocol_metrics.add_preprocessing_elements(protocol);
        protocol_metrics.start_duration_calculation();
        for (requirement, count) in protocol_metrics.preprocessing_requirements.iter() {
            let current_count = self.summary.preprocessing_elements.entry(requirement.clone()).or_default();
            *current_count = current_count.saturating_add(*count);
        }
        self.summary
            .online_protocols
            .entry(protocol.name())
            .or_insert(ProtocolVariantMetrics::new(protocol.name()))
            .increment_call_count();
        step.protocols.insert(protocol.address().to_string(), protocol_metrics);
    }

    /// Pause the execution of a protocol.
    /// This is because the online protocols are paused while they are waiting for responses from
    /// other nodes.
    pub(crate) fn online_protocol_paused(&mut self, address: &ProtocolAddress) {
        self.update_protocol_duration(address);
    }

    /// Resume the execution of a protocol. It should be paused early.
    /// This is because the online protocols have to be resumed when they receive waited messages
    /// from other nodes.
    pub(crate) fn protocol_resumed(&mut self, address: &ProtocolAddress) {
        if !self.config.enable {
            return;
        }
        let Some(step) = self.steps.last_mut() else {
            warn!("Execution Metrics: execution plan is empty");
            return;
        };

        let Some(protocol_metrics) = step.protocols.get_mut(&address.to_string()) else {
            warn!("Execution Metrics: protocol with address {address} has not been found");
            return;
        };
        protocol_metrics.start_duration_calculation();
    }

    /// A local protocol execution has finished
    pub(crate) fn local_protocol_ended(&mut self, address: &ProtocolAddress) {
        if let Some((protocol_variant, duration)) = self.update_protocol_duration(address) {
            // Update the general metrics about the protocol variant
            self.summary
                .local_protocols
                .entry(protocol_variant)
                .or_insert(ProtocolVariantMetrics::new(protocol_variant))
                .update_duration(duration);
        }
    }

    /// An online protocol execution has finished
    pub(crate) fn online_protocol_ended(&mut self, address: &ProtocolAddress) {
        if let Some((protocol_variant, duration)) = self.update_protocol_duration(address) {
            // Update the general metrics about the protocol variant
            self.summary
                .online_protocols
                .entry(protocol_variant)
                .or_insert(ProtocolVariantMetrics::new(protocol_variant))
                .update_duration(duration);
        }
    }

    /// Update the duration of a specific protocol
    fn update_protocol_duration(&mut self, address: &ProtocolAddress) -> Option<(&'static str, Duration)> {
        if !self.config.enable {
            return None;
        }

        let Some(step) = self.steps.last_mut() else {
            warn!("Execution Metrics: execution plan is empty");
            return None;
        };

        let Some(protocol_metrics) = step.protocols.get_mut(&address.to_string()) else {
            warn!("Execution Metrics: protocol with address {address} has not been found");
            return None;
        };

        protocol_metrics.accumulate_partial_duration();
        Some((protocol_metrics.variant, protocol_metrics.duration))
    }

    /// Add a new communication round.
    pub(crate) fn add_step_round(&mut self) {
        if !self.config.enable {
            return;
        }

        let Some(step) = self.steps.last_mut() else {
            warn!("Execution Metrics: execution plan is empty");
            return;
        };

        step.add_round();
        self.summary.total_rounds = self.summary.total_rounds.saturating_add(1);
    }

    /// Add one communication round for a protocol in the last execution step has been added
    pub(crate) fn add_protocol_round<M>(&mut self, address: &ProtocolAddress, messages: &[InstructionMessage<M>])
    where
        M: Serialize + Clone + Debug,
    {
        if !self.config.enable {
            return;
        }

        let Some(step) = self.steps.last_mut() else {
            warn!("Execution Metrics: execution plan is empty");
            return;
        };

        let Some(protocol_metrics) = step.protocols.get_mut(&address.to_string()) else {
            warn!("Execution Metrics: protocol with address {address} has not been found");
            return;
        };

        protocol_metrics.add_communication_round();

        // serialized_size could be potentially costly, so we offer an option to disable it.
        if self.config.enable_message_size_calculation {
            let mut total_message_size = 0u64;
            for message in messages {
                let Ok(message_size) = MessageCodec::bincode_options().serialized_size(message) else {
                    warn!("Metrics: failed getting serialized message size for online protocol with address {address}");
                    continue;
                };
                total_message_size = total_message_size.saturating_add(message_size);
            }
            protocol_metrics.inc_message_size(total_message_size);
            self.summary.update_total_message_size(protocol_metrics.variant, total_message_size);

            let Some(round_size) = step.rounds_message_size.last_mut() else {
                warn!("Execution metrics: step does not contain any round");
                return;
            };
            *round_size = round_size.saturating_add(total_message_size);
        }
    }

    /// Merges multiple execution plan metrics results into one, calculating average values.
    /// Returns None if an empty Vec was provided.
    pub fn merge(metrics: Vec<Self>) -> Option<Self> {
        let mut config = None;
        let mut all_summaries = vec![];
        let mut all_steps = vec![];
        for metric in metrics {
            let (other_config, summary, steps) = metric.into_parts();
            config = Some(other_config); // The config should be the same always.
            all_summaries.push(summary);
            all_steps.push(steps);
        }

        Some(Self {
            config: config?,
            summary: ExecutionPlanSummary::merge(all_summaries)?,
            steps: StepMetrics::merge_executions_steps(all_steps)?,
        })
    }

    /// Return the metrics of the execution in parts.
    fn into_parts(self) -> (ExecutionMetricsConfig, ExecutionPlanSummary, Vec<StepMetrics>) {
        (self.config, self.summary, self.steps)
    }

    /// Displays or writes to a file the metrics, depending on chosen options.
    pub fn standard_output(self, format: Option<MetricsFormat>, filepath: Option<&str>) -> Result<()> {
        if let Some(format) = format {
            let metrics_output = if self.config.enable_execution_plan_metrics {
                match format {
                    MetricsFormat::Text => self.to_string(),
                    MetricsFormat::Json => serde_json::to_string(&self)
                        .map_err(|e| anyhow!("failed to serialize metrics into JSON: {e}"))?,
                    MetricsFormat::Yaml => serde_yaml::to_string(&self)
                        .map_err(|e| anyhow!("failed to serialize metrics into YAML: {e}"))?,
                }
            } else {
                match format {
                    MetricsFormat::Text => self.summary.to_string(),
                    MetricsFormat::Json => serde_json::to_string(&self.summary)
                        .map_err(|e| anyhow!("failed to serialize metrics into JSON: {e}"))?,
                    MetricsFormat::Yaml => serde_yaml::to_string(&self.summary)
                        .map_err(|e| anyhow!("failed to serialize metrics into YAML: {e}"))?,
                }
            };

            let output = {
                if let Some(metrics_filepath) = filepath {
                    Some((metrics_filepath.to_string(), metrics_output))
                } else {
                    match format {
                        MetricsFormat::Text if self.config.enable_execution_plan_metrics => {
                            Some(("metrics.txt".to_owned(), metrics_output))
                        }
                        MetricsFormat::Text => {
                            println!("{metrics_output}");
                            None
                        }
                        MetricsFormat::Json => Some(("metrics.json".to_owned(), metrics_output)),
                        MetricsFormat::Yaml => Some(("metrics.yaml".to_owned(), metrics_output)),
                    }
                }
            };

            if let Some((metrics_filepath, metrics_output)) = output {
                fs::write(&metrics_filepath, metrics_output)
                    .map_err(|e| anyhow!("failed writing metrics into {metrics_filepath}: {e}"))?;
            }
        }

        Ok(())
    }
}

impl Display for ExecutionMetrics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Summary:")?;
        writeln!(f, "{}", self.summary)?;
        writeln!(f)?;
        writeln!(f, "Execution Plan:")?;
        for step in &self.steps {
            writeln!(f, "{}", step)?;
        }
        Ok(())
    }
}

/// Represents the result of a VM execution's metrics calculation.
#[derive(Clone, Debug, Serialize)]
pub struct ExecutionPlanSummary {
    #[serde(skip_serializing)]
    execution_start: Option<Instant>,

    /// Total execution duration.
    pub execution_duration: Duration,

    /// Computational time that the node spends in the execution
    pub compute_duration: MinMaxDuration,

    /// Preprocessing elements.
    pub preprocessing_elements: BTreeMap<String, usize>,

    /// Local protocols.
    pub local_protocols: IndexMap<&'static str, ProtocolVariantMetrics>,

    /// Online protocols.
    pub online_protocols: IndexMap<&'static str, ProtocolVariantMetrics>,

    /// Total message size that a node has sent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_message_size: Option<u64>,

    /// Total number of execution steps according to the [`ExecutionPlan`]
    pub total_execution_steps: u64,

    /// Total number of communication rounds
    pub total_rounds: u64,
}

impl ExecutionPlanSummary {
    /// Creates a new ExecutionPlanSummary
    fn new(enable_message_size_calculation: bool) -> Self {
        Self {
            execution_start: None,
            execution_duration: Duration::default(),
            compute_duration: MinMaxDuration::default(),
            preprocessing_elements: BTreeMap::default(),
            local_protocols: IndexMap::default(),
            online_protocols: IndexMap::default(),
            total_message_size: if enable_message_size_calculation { Some(0) } else { None },
            total_execution_steps: 0,
            total_rounds: 0,
        }
    }

    /// The execution has started
    pub(crate) fn execution_started(&mut self) {
        self.execution_start = Some(Instant::now())
    }

    /// Update the compute duration.
    fn update_compute_duration(&mut self) {
        let Some(execution_start) = self.execution_start.take() else {
            warn!("Metrics: execution is not running");
            return;
        };
        self.compute_duration = self.compute_duration.update(execution_start.elapsed());
    }

    /// Update the total protocol message size that the node has sent to other nodes
    fn update_total_message_size(&mut self, protocol_variant: &'static str, message_size: u64) {
        if let Some(total_message_size) = &mut self.total_message_size {
            self.online_protocols
                .entry(protocol_variant)
                .or_insert(ProtocolVariantMetrics::new(protocol_variant))
                .update_total_message_size(message_size);
            *total_message_size = total_message_size.saturating_add(message_size);
        }
    }

    /// Merges multiple execution plan summary into one, calculating average values.
    /// Returns None if an empty Vec was provided.
    pub fn merge(metrics: Vec<Self>) -> Option<Self> {
        let metrics_count = metrics.len();
        let mut metrics_iterator = metrics.into_iter();
        let mut result = metrics_iterator.next()?;
        let mut preprocessing_elements = result.preprocessing_elements.clone();

        // We keep the first one and use it to calculate the total sum for calls and duration.
        for other_metrics in metrics_iterator {
            result.compute_duration.total =
                result.compute_duration.total.saturating_add(other_metrics.compute_duration.total);
            result.compute_duration.min = result.compute_duration.min.min(other_metrics.compute_duration.min);
            result.compute_duration.max = result.compute_duration.max.max(other_metrics.compute_duration.max);

            for (preprocessing_elements_name, elements_count) in other_metrics.preprocessing_elements {
                let elements = preprocessing_elements.entry(preprocessing_elements_name).or_default();
                *elements = elements.saturating_add(elements_count);
            }

            for (protocol_variant, other_protocol) in other_metrics.local_protocols {
                let protocol = result
                    .local_protocols
                    .entry(protocol_variant)
                    .or_insert(ProtocolVariantMetrics::new(protocol_variant));
                protocol.calls = protocol.calls.saturating_add(other_protocol.calls);
                protocol.duration.total = protocol.duration.total.saturating_add(other_protocol.duration.total);
                protocol.duration.min = protocol.duration.min.min(other_protocol.duration.min);
                protocol.duration.max = protocol.duration.max.max(other_protocol.duration.max);
            }

            for (protocol_variant, other_protocol) in other_metrics.online_protocols {
                let protocol = result
                    .online_protocols
                    .entry(protocol_variant)
                    .or_insert(ProtocolVariantMetrics::new(protocol_variant));
                protocol.calls = protocol.calls.saturating_add(other_protocol.calls);
                protocol.total_message_size =
                    protocol.total_message_size.saturating_add(other_protocol.total_message_size);
                protocol.duration.total = protocol.duration.total.saturating_add(other_protocol.duration.total);
                protocol.duration.min = protocol.duration.min.min(other_protocol.duration.min);
                protocol.duration.max = protocol.duration.max.max(other_protocol.duration.max);
            }
        }

        // Average over input metrics.
        result.compute_duration.total =
            result.compute_duration.total.checked_div(metrics_count as u32).unwrap_or_default();

        for elements_count in preprocessing_elements.values_mut() {
            *elements_count = elements_count.checked_div(metrics_count).unwrap_or_default();
        }

        for protocol in result.local_protocols.values_mut() {
            protocol.calls = protocol.calls.checked_div(metrics_count as u64).unwrap_or_default();
            protocol.duration.total = protocol.duration.total.checked_div(metrics_count as u32).unwrap_or_default();
        }

        for protocol in result.online_protocols.values_mut() {
            protocol.calls = protocol.calls.checked_div(metrics_count as u64).unwrap_or_default();
            protocol.total_message_size =
                protocol.total_message_size.checked_div(metrics_count as u64).unwrap_or_default();
            protocol.duration.total = protocol.duration.total.checked_div(metrics_count as u32).unwrap_or_default();
        }

        Some(result)
    }
}

impl Display for ExecutionPlanSummary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Preprocessing elements:")?;
        for (element, count) in &self.preprocessing_elements {
            writeln!(f, "\t\t{element:?}: {count}")?;
        }
        writeln!(f)?;
        writeln!(f, "Execution metrics:")?;
        writeln!(f, "\tExecution duration: \n\t\t{}\n", format_duration(self.execution_duration))?;
        writeln!(f, "\tCompute duration: \n\t\t{}\n", self.compute_duration)?;
        writeln!(f, "\tTotal rounds: {}", self.total_rounds)?;
        if let Some(message_size) = self.total_message_size {
            writeln!(f, "\tTotal network messages size: {}", format_size(message_size, DECIMAL))?;
        }
        writeln!(f)?;
        writeln!(f, "\tLocal protocols: (execution order)")?;
        for protocol in self.local_protocols.values() {
            writeln!(f, "{protocol}")?;
        }
        writeln!(f, "\tOnline protocols: (execution order)")?;
        for protocol in self.online_protocols.values() {
            writeln!(f, "{protocol}")?;
        }

        Ok(())
    }
}

/// Represents an executed step. It contains the information about the execution of all protocols
/// that were executed during the step.
#[derive(Default, Clone, Debug, Serialize)]
pub struct StepMetrics {
    /// Step index in the plan
    pub index: usize,
    /// Executed protocols during this step.
    pub protocols: BTreeMap<String, ProtocolMetrics>,
    /// Total message size that is sent in each round
    pub rounds_message_size: Vec<u64>,
}

impl StepMetrics {
    /// Creates a new instance
    fn new(index: usize) -> Self {
        Self { index, protocols: BTreeMap::new(), rounds_message_size: vec![] }
    }

    /// Add a new communication round. An execution step can have multiple rounds, depending on:
    /// - The communication rounds a protocol needs
    /// - The limit of protocol messages that the nodes have defined. This is for preventing too
    ///   large messages and in this case, the nodes split the original round into multiple rounds.
    fn add_round(&mut self) {
        self.rounds_message_size.push(0);
    }

    /// Merge the steps of several execution of the same plan.
    fn merge_executions_steps(executions_steps: Vec<Vec<Self>>) -> Option<Vec<Self>> {
        let executions_count = executions_steps.len();
        let mut executions_steps_iterator = executions_steps.into_iter();
        let mut merged_steps = executions_steps_iterator.next()?;
        // Accumulation of values that can change during the execution
        for execution_steps in executions_steps_iterator {
            for (merged_steps, steps) in merged_steps.iter_mut().zip(execution_steps) {
                for (address, metrics) in merged_steps.protocols.iter_mut() {
                    let Some(other_metrics) = steps.protocols.get(address) else {
                        warn!("Execution Metrics: protocol {address} not found in an execution");
                        continue;
                    };
                    // Only duration should be different.
                    metrics.duration = metrics.duration.saturating_add(other_metrics.duration);
                }
                for (round, other_round) in merged_steps.rounds_message_size.iter_mut().zip(steps.rounds_message_size) {
                    *round = round.saturating_add(other_round);
                }
            }
        }
        // Calculate the average of the values have been accumulated in the previous stage.
        for step in merged_steps.iter_mut() {
            for metrics in step.protocols.values_mut() {
                metrics.duration = metrics.duration.checked_div(executions_count as u32).unwrap_or_default();
            }
            for round_size in step.rounds_message_size.iter_mut() {
                *round_size = round_size.checked_div(executions_count as u64).unwrap_or_default();
            }
        }
        Some(merged_steps)
    }
}

impl Display for StepMetrics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Step #{}:", self.index)?;
        writeln!(f)?;
        writeln!(f, "\tCommunication rounds: {}", self.rounds_message_size.len())?;
        for (index, round_message_size) in self.rounds_message_size.iter().enumerate() {
            writeln!(f, "\t\tRound #{} message size: {}", index, round_message_size)?;
        }
        writeln!(f)?;
        writeln!(f, "\tProtocols:")?;
        for metrics in self.protocols.values() {
            writeln!(f, "{metrics}")?;
        }
        Ok(())
    }
}

/// Represents an executed protocol. It contains all information that is related with the execution
/// of a protocol.
#[derive(Clone, Debug, Serialize)]
pub struct ProtocolMetrics {
    /// Protocol variant
    pub variant: &'static str,
    /// Executed protocol
    pub address: String,
    /// Step which it was executed into the plan
    pub step: usize,
    /// The number of preprocessing elements that the protocols required
    pub preprocessing_requirements: BTreeMap<String, usize>,
    /// Number of communication rounds that the protocol requires for its execution
    pub rounds: usize,
    /// Total message size that the protocol sent
    pub total_message_size: u64,
    /// Total execution time for this protocol.
    pub duration: Duration,
    #[serde(skip_serializing)]
    start_time: Option<Instant>,
}

impl ProtocolMetrics {
    /// Creates the metrics for a local protocol.
    fn new(protocol: &impl Protocol, step: usize) -> Self {
        Self {
            variant: protocol.name(),
            address: protocol.address().to_string(),
            step,
            preprocessing_requirements: BTreeMap::default(),
            rounds: 0,
            total_message_size: 0,
            duration: Duration::default(),
            start_time: None,
        }
    }

    /// Creates the metrics for an online protocol. The number of communication rounds and message size
    /// will be updated during the execution.
    fn add_preprocessing_elements(&mut self, protocol: &impl Protocol) {
        for (requirement_type, count) in protocol.runtime_requirements() {
            self.insert_requirements(format!("{requirement_type:?}"), *count)
        }
    }

    /// Catch the instant when a protocol execution is started/resumed. This instant will be used
    /// to calculate the computational time that the protocol has consumed.
    fn start_duration_calculation(&mut self) {
        if self.start_time.is_some() {
            warn!(
                "Execution Metrics: protocol with address {} started or resumed without having been paused",
                self.address
            );
            return;
        }
        self.start_time = Some(Instant::now());
    }

    /// Update the duration of a protocol when it is paused/stopped.
    fn accumulate_partial_duration(&mut self) {
        let Some(start_time) = self.start_time.take() else {
            warn!(
                "Execution Metrics: protocol with address {} paused or finished without to be executing",
                self.address
            );
            return;
        };
        self.duration = self.duration.saturating_add(start_time.elapsed());
    }

    /// Insert the count of a specific preprocessing element that a protocol requires
    fn insert_requirements<E: Debug>(&mut self, requirement_type: E, count: usize) {
        if count > 0 {
            self.preprocessing_requirements.insert(format!("{requirement_type:?}"), count);
        }
    }

    /// Add a new communication round and the message size for it.
    fn add_communication_round(&mut self) {
        self.rounds = self.rounds.wrapping_add(1);
    }

    /// Increment the total message size that the protocol has sent
    fn inc_message_size(&mut self, message_size: u64) {
        self.total_message_size = self.total_message_size.wrapping_add(message_size);
    }
}

impl Display for ProtocolMetrics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\t\t{} - {}:", self.address, self.variant)?;
        // writeln!(f)?;
        writeln!(f, "\t\t\tExecution step: {}", self.step)?;
        writeln!(f, "\t\t\tTotal duration: {}", format_duration(self.duration))?;
        writeln!(f, "\t\t\tCommunication rounds: {}", self.rounds)?;
        writeln!(f, "\t\t\tTotal message size: {}", self.total_message_size)?;
        if !self.preprocessing_requirements.is_empty() {
            writeln!(f, "\t\t\tUsed preprocessing elements:")?;
            for (element_type, count) in &self.preprocessing_requirements {
                writeln!(f, "\t\t\t\t- {element_type}: {count}")?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
/// Contains the metrics about a protocol variant
pub struct ProtocolVariantMetrics {
    /// Protocol variant
    pub variant: &'static str,
    /// Metrics about the duration of all protocols of a type
    pub duration: MinMaxDuration,
    /// Count of all calls to the protocol variant
    pub calls: u64,
    /// Sum of the size of all message that all protocol of this variant have sent
    pub total_message_size: u64,
}

impl Display for ProtocolVariantMetrics {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "\t\t{}:", self.variant)?;
        writeln!(f, "\t\t\tcalls: {}", self.calls)?;
        writeln!(f, "\t\t\t{}", self.duration)?;
        if self.total_message_size > 0 {
            writeln!(f, "\t\t\ttotal network messages size: {}", format_size(self.total_message_size, DECIMAL))?;
        }
        Ok(())
    }
}

impl ProtocolVariantMetrics {
    /// Create a new instance
    pub(crate) fn new(variant: &'static str) -> Self {
        Self { variant, duration: Default::default(), calls: 0, total_message_size: 0 }
    }

    /// Update the message size that the protocol variant has sent
    fn update_total_message_size(&mut self, message_size: u64) {
        self.total_message_size = self.total_message_size.saturating_add(message_size);
    }

    /// Update the computational time that the protocol variant has consumed.
    fn update_duration(&mut self, duration: Duration) {
        self.duration = self.duration.update(duration);
    }

    /// Count the number of calls to this variant in the execution plan
    fn increment_call_count(&mut self) {
        self.calls = self.calls.saturating_add(1);
    }
}

/// Metrics format to use when writing.
#[derive(Clone, Copy, Serialize, ValueEnum)]
pub enum MetricsFormat {
    /// Metrics in text format.
    Text,

    /// Metrics in JSON format.
    Json,

    /// Metrics in YAML format.
    Yaml,
}
