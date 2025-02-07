//! The command line argument types.

use crate::nada_project_toml::PrimeSize;
use clap::{Args, Parser, Subcommand};
use clap_utils::shell_completions::ShellCompletionsArgs;
use mpc_vm::vm::simulator::MetricsFormat;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new nada project
    Init(InitArgs),
    /// Build a program
    Build(BuildArgs),
    /// Run a program using the inputs from a test case
    Run(RunArgs),
    /// Run a program, inputs are json from stdin and output is also json
    #[clap(hide(true))]
    RunJson(RunJsonArgs),
    /// Run tests
    Test(TestArgs),
    /// Benchmark one or multiple programs
    Benchmark(BenchmarkArgs),
    /// Generate a test for a program with example values
    GenerateTest(GenerateTestsArgs),
    /// Get requirements for program
    ProgramRequirements(ProgramRequirementsArgs),
    /// Generate shell completions
    ShellCompletions(ShellCompletionsArgs),
    /// List Protocols (instructions) in JSON format
    ListProtocols,
    /// Publish a nada program
    Publish(PublishArgs),
    /// Compute a nada program in a live network
    Compute(ComputeArgs),
    /// Adds a new program to the project.
    NewProgram(NewProgramArgs),
}

#[derive(Args)]
pub struct BuildArgs {
    /// The program to build if not specified will build all programs
    #[clap(index = 1)]
    pub program: Option<String>,
    /// Also output the MIR in JSON format
    #[clap(long, short, action)]
    pub mir_json: bool,
}

#[derive(Args)]
pub struct InitArgs {
    /// Name of the project / project directory
    #[clap(index = 1)]
    pub name: String,
}

#[derive(Args)]
pub struct RunArgs {
    /// Test name to use as inputs for the run (program is specified in the testfile)
    #[clap(index = 1)]
    pub test: String,

    /// Output the Bytecode in JSON format
    #[clap(long, action)]
    pub bytecode_json: bool,

    /// Output the Bytecode in text format
    #[clap(long, short, action)]
    pub bytecode_text: bool,

    /// Output the Protocols model in text format, in `target/<PROGRAM>.protocols.txt`,
    #[clap(long, short, action)]
    pub protocols_text: bool,

    /// Run in debug mode not using the cryptographic protocols to be able to debug the program / see the values
    #[clap(long, short, action)]
    pub debug: bool,

    /// Print protocol runtime information.
    /// Only available in non-debug mode.
    /// Protocols are displayed in execution order.
    /// By default, text metrics are displayed on stdout, JSON metrics in a metrics.json file and YAML metrics in a
    /// `metrics.yaml` file.
    #[clap(long)]
    pub metrics: Option<MetricsFormat>,

    /// If specified, the metrics will be written into a specific file.
    #[clap(long)]
    pub metrics_filepath: Option<String>,

    /// Measure messages size.
    /// Sizes are in bytes.
    #[clap(long, default_value_t = false, hide = true)]
    pub metrics_message_size: bool,

    /// Enable the execution plan metrics.
    /// The execution plan metrics are written always in a file.
    #[clap(long, default_value_t = false, hide = true)]
    pub metrics_execution_plan: bool,
}

#[derive(Args)]
pub struct RunJsonArgs {
    /// Program to run
    #[clap(index = 1)]
    pub program: String,
    /// Run in debug mode not using the cryptographic protocols to be able to debug the program / see the values
    #[clap(long, short, action)]
    pub debug: bool,
}

#[derive(Args)]
pub struct TestArgs {
    /// Test name to run if not specifies run all tests
    #[clap(index = 1)]
    pub test: Option<String>,
    /// Run test in debug mode not using the cryptographic protocols to be able to debug the program / see the values
    #[clap(long, short, action)]
    pub debug: bool,
}

/// The CLI arguments for `nada publish` command
#[derive(Args)]
pub struct PublishArgs {
    /// A valid network name in the configuration
    #[clap(long, short)]
    pub network: String,
    /// The program to be published
    #[clap(index = 1)]
    pub program: String,
}

/// The CLI arguments for `nada compute` command
#[derive(Args)]
pub struct ComputeArgs {
    /// A valid network name in the configuration
    #[clap(long, short)]
    pub network: String,
    /// The test name to be run
    #[clap(index = 1)]
    pub test: String,
}

#[derive(Args)]
pub struct BenchmarkArgs {
    /// Names of the tests to use to benchmark the programs, if not specified benchmark all tests
    #[clap(index = 1)]
    pub tests: Option<Vec<String>>,

    /// How many times each test should be run
    #[clap(long, default_value_t = 1)]
    pub run_count: usize,

    /// Calculate the size of messages exchanged per protocol.
    /// Sizes provided in bytes.
    #[clap(long, default_value_t = true)]
    pub message_size_calculation: bool,
}

#[derive(Args)]
pub struct ProgramRequirementsArgs {
    /// The program to get requirements
    #[clap(index = 1)]
    pub program: String,
}

#[derive(Args)]
pub struct NewProgramArgs {
    /// The name of the program that will be created
    #[clap(index = 1)]
    pub program: String,

    /// The optional prime size
    #[clap(long, short, value_parser=clap::value_parser!(PrimeSize))]
    pub prime_size: Option<PrimeSize>,
}

#[derive(Args)]
pub struct GenerateTestsArgs {
    /// The program to generate the test for
    #[clap(index = 1)]
    pub program: String,
    /// The name of the test to generate / test case name
    #[clap(long, short)]
    pub test_name: String,

    /// Values of the inputs in json format {"my_int": 32, "my_int2": 42}
    #[clap(long, short)]
    pub inputs: Option<String>,

    /// Values of the outputs in json format {"my_out": 32, "my_out2": 42}
    #[clap(long, short)]
    pub outputs: Option<String>,
}

#[cfg(test)]
mod test {
    use super::Cli;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
