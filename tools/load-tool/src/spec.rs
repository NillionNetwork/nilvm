//! Test specifications.

use crate::{flow::ErrorPolicy, runner::StartPolicy};
use human_size::HumanSize;
use nada_values_args::file::Inputs;
use nillion_chain_client::transactions::TokenAmount;
use serde::Deserialize;
use std::{path::PathBuf, time::Duration};

/// Test specification.
#[derive(Deserialize, Debug)]
pub struct TestSpec {
    /// The operation performed by this test.
    pub operation: Operation,

    /// The maximum amount of time we want to run this for.
    #[serde(with = "humantime_serde", default)]
    pub max_test_duration: Option<Duration>,

    /// The maximum average flow duration we're willing to tolerate.
    #[serde(with = "humantime_serde", default = "default_max_flow_duration")]
    pub max_flow_duration: Duration,

    /// The maximum failure rate we're willing to tolerate before stopping the test.
    ///
    /// Note that this is a rate in the range 0-1.
    #[serde(default = "default_max_error_rate")]
    pub max_error_rate: f64,

    /// The worker increment mode.
    pub mode: WorkerIncrementMode,

    /// The start policy to use.
    #[serde(default = "default_start_policy")]
    pub start_policy: StartPolicy,

    /// The error policy to use.
    #[serde(default = "default_error_policy")]
    pub error_policy: ErrorPolicy,

    /// The seeds used to derive private keys: user and node.
    pub seeds: Option<Seeds>,

    /// The required starting balance in unils.
    #[serde(default = "default_required_starting_balance")]
    pub required_starting_balance: u64,

    /// The signing key mode.
    #[serde(default)]
    pub signing_key: SigningKeyMode,
}

/// Seeds configuration.
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Seeds {
    /// Seed prefix
    Prefix(String),
    /// Seed list
    List(Vec<String>),
}

/// The mode in which the number of workers is incremented.
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum WorkerIncrementMode {
    /// Manual mode where all configurations are up to the user.
    Manual {
        /// Initial number of workers.
        initial_workers: u32,

        /// Number of workers added on every increment.
        worker_increment: u32,

        /// Frequency at which workers are incremented.
        #[serde(with = "humantime_serde")]
        worker_increment_frequency: Duration,
    },

    /// Automatic mode that progressively pushes the number of workers up.
    Automatic,

    /// A mode that keeps the number of workers steady at a certain number.
    Steady {
        /// Number of workers to use.
        workers: u32,

        /// Nillion clients quantity.
        clients: Option<u32>,
    },
}

/// The operation to run.
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum Operation {
    /// A store values operation.
    StoreValues {
        /// The inputs for this operation.
        inputs: ValuesSpec,
    },

    /// A retrieve value operation.
    RetrieveValue {
        /// The inputs for this operation.
        input: ValuesSpec,
    },

    /// A store program operation.
    StoreProgram {
        /// The path to the program to be stored.
        program_path: PathBuf,
    },

    /// A compute operation.
    Compute(ComputeSpec),
}

/// The spec for the secrets being uploaded.
#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum ValuesSpec {
    /// Upload a custom set of values.
    Custom {
        /// The inputs to use.
        #[serde(flatten)]
        inputs: Box<Inputs>,
    },

    /// Upload a blob of a certain size.
    Blob {
        /// The blob size.
        size: HumanSize,
    },
}

/// The spec for the computation being executed.
#[derive(Deserialize, Clone, Debug)]
pub struct ComputeSpec {
    /// The path to the program's in its compiled form.
    pub program_path: PathBuf,

    /// The mode used during the compute operation.
    pub mode: ComputeMode,

    /// The inputs for the computation.
    pub inputs: Option<ValuesSpec>,
}

/// The spec for the computation being executed.
#[derive(Deserialize, Clone, Debug)]
pub enum ComputeMode {
    /// Invoke the computation passing in all secrets along as compute secrets.
    StoreNone,

    /// Upload all inputs ahead of time and simply invoke the program with no compute secrets.
    StoreAll,

    /// Store half of the inputs ahead of time and invoke the program with the rest of them.
    StoreHalf,
}

/// The signing key mode to use.
#[derive(Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum SigningKeyMode {
    /// Use a random signing key.
    #[default]
    Random,

    /// Use a specific private key when signing.
    PrivateKey(#[serde(deserialize_with = "hex::serde::deserialize")] Vec<u8>),
}

fn default_max_flow_duration() -> Duration {
    Duration::from_secs(5)
}

fn default_max_error_rate() -> f64 {
    0.25
}

fn default_error_policy() -> ErrorPolicy {
    ErrorPolicy::StopOnPreprocessingExhausted
}

fn default_start_policy() -> StartPolicy {
    StartPolicy::StartImmediately
}

fn default_required_starting_balance() -> u64 {
    TokenAmount::Nil(100).to_unil()
}
