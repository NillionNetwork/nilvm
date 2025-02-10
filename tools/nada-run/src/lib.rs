use anyhow::{anyhow, bail, Error};
use clap::Parser;
use clap_utils::ParserExt;
use client_metrics::{fields, ClientMetrics};
use log::{debug, error};
use math_lib::modular::{SafePrime, U128SafePrime, U256SafePrime, U64SafePrime};
use metrics::metrics::MetricsRegistry;
use mpc_vm::{
    protocols::MPCProtocol,
    vm::{
        simulator::{
            ExecutionMetrics, InputGenerator, MetricsFormat, ProgramSimulator, SimulationParameters,
            StaticInputGeneratorBuilder,
        },
        ExecutionMetricsConfig, ExecutionVmConfig,
    },
    JitCompiler, MPCCompiler, Program,
};
use nada_compiler_backend::mir::{proto::ConvertProto, ProgramMIR};
use nada_value::{clear::Clear, NadaValue};
use nada_values_args::NadaValueArgs;
use shamir_sharing::secret_sharer::{SafePrimeSecretSharer, ShamirSecretSharer};
use std::{collections::HashMap, fs, fs::File, io::Read};

#[derive(Parser)]
#[clap(author = "Nillion", version, about = "A tool that executes programs under a simulated Nillion network.")]
struct Cli {
    /// Program path.
    program_path: String,

    /// Prime size in bits.
    #[clap(short, long, default_value_t = 256)]
    prime_size: u32,

    /// The size of the simulated network.
    #[clap(short, long, default_value_t = 3)]
    network_size: usize,

    /// The degree of the polynomial used.
    #[clap(short = 'd', long, default_value_t = 1)]
    polynomial_degree: u64,

    /// The input values.
    #[clap(flatten)]
    values: NadaValueArgs,

    /// Print protocol runtime information.
    /// Protocols are displayed in execution order.
    /// By default, text metrics are displayed on stdout, JSON metrics in a metrics.json file and YAML metrics in a
    /// metrics.yaml file.
    #[clap(long, hide = true)]
    metrics: Option<MetricsFormat>,

    /// If specified, the metrics will be written into a specific file.
    #[clap(long, hide = true)]
    metrics_filepath: Option<String>,

    /// Measure protocol message size.
    /// Sizes are in bytes.
    #[clap(long, default_value_t = false, hide = true)]
    metrics_message_size: bool,

    /// Print VM metrics in prometheus format.
    ///
    /// It will print the VM metrics in the file `prometheus.txt`
    #[clap(long, default_value_t = false, hide = true)]
    prometheus_metrics: bool,

    /// Enable the execution plan metrics.
    /// The execution plan metrics are written always in a file.
    #[clap(long, default_value_t = false, hide = true)]
    pub metrics_execution_plan: bool,
}

fn build_inputs(cli: &Cli) -> Result<InputGenerator, Error> {
    let mut builder = StaticInputGeneratorBuilder::default();
    builder.extend(cli.values.parse()?);

    Ok(builder.build())
}

fn simulate<T>(
    program: Program<MPCProtocol>,
    parameters: SimulationParameters,
    secrets: &InputGenerator,
    message_size_calculation: bool,
    execution_plan_metrics: bool,
) -> Result<(HashMap<String, NadaValue<Clear>>, ExecutionMetrics), Error>
where
    T: SafePrime,
    ShamirSecretSharer<T>: SafePrimeSecretSharer<T>,
{
    let simulator = ProgramSimulator::<MPCProtocol, T>::new(
        program,
        parameters,
        secrets,
        ExecutionMetricsConfig::enabled(message_size_calculation, execution_plan_metrics),
    )?;
    simulator.run()
}

fn run(cli: Cli) -> Result<(), Error> {
    debug!("Loading program's MIR from {}", cli.program_path);
    let mut program = vec![];
    File::open(&cli.program_path)
        .map_err(|e| anyhow!("failed to open program's MIR file: {e}"))?
        .read_to_end(&mut program)?;
    let program_mir = ProgramMIR::try_decode(&program).map_err(|e| anyhow!("failed to parse program's MIR: {e}"))?;

    debug!("Parsing program");
    let program = MPCCompiler::compile(program_mir).map_err(|e| anyhow!("failed to compile program's MIR: {e}"))?;

    debug!("Loading secrets");
    let inputs = build_inputs(&cli)?;
    let parameters = SimulationParameters {
        network_size: cli.network_size,
        polynomial_degree: cli.polynomial_degree,
        execution_vm_config: ExecutionVmConfig::default(),
    };

    let client_metrics = ClientMetrics::new_default("nada-run");

    debug!("Running program");
    let (result, metrics) = match cli.prime_size {
        64 => {
            client_metrics.send_event_sync("run", fields! { "prime_size" => "64" });
            simulate::<U64SafePrime>(
                program,
                parameters,
                &inputs,
                cli.metrics_message_size,
                cli.metrics_execution_plan,
            )?
        }
        128 => {
            client_metrics.send_event_sync("run", fields! { "prime_size" => "128" });
            simulate::<U128SafePrime>(
                program,
                parameters,
                &inputs,
                cli.metrics_message_size,
                cli.metrics_execution_plan,
            )?
        }
        256 => {
            client_metrics.send_event_sync("run", fields! { "prime_size" => "256" });
            simulate::<U256SafePrime>(
                program,
                parameters,
                &inputs,
                cli.metrics_message_size,
                cli.metrics_execution_plan,
            )?
        }
        _ => bail!("invalid prime size"),
    };

    metrics.standard_output(cli.metrics, cli.metrics_filepath.as_deref())?;

    print_output(result);

    Ok(())
}

/// Print outputs in human format not modular.
fn print_output(outputs: HashMap<String, NadaValue<Clear>>) {
    for (output_name, value) in outputs {
        println!("Output ({output_name}): {value:?}");
    }
}

/// The driver function that parses the arguments and runs the simulator.
pub fn driver() -> Result<(), Error> {
    let metrics_registry = metrics::initialize(HashMap::new())?;
    let args = Cli::parse_with_version();
    let prometheus_metrics = args.prometheus_metrics;

    if let Err(e) = run(args) {
        error!("Failed to run program: {e}");
    }

    if prometheus_metrics {
        println!("\n Saving metrics in prometheus.txt");
        fs::write("prometheus.txt", metrics_registry.encode_metrics()?)?;
    }

    Ok(())
}
