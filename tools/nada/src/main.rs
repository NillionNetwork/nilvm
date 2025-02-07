//! Nada CLI tool to manage Nada projects
//! You can use this tool to build, run, test, and publish Nada programs
//! You can also generate tests for your programs, get the requirements for a program and benchmark programs
//!
#![feature(once_cell_get_mut)]
#![deny(missing_docs)]

use crate::{
    args::{
        BuildArgs, Cli, Command, GenerateTestsArgs, InitArgs, ProgramRequirementsArgs, RunArgs, RunJsonArgs, TestArgs,
    },
    json::json_to_values,
    nada_project_toml::{NadaProjectToml, ProgramConf},
    program::Program,
    run::RunOptions,
    test::{staticfile, TestCase},
};
use args::{ComputeArgs, NewProgramArgs, PublishArgs};
use clap::CommandFactory;
use clap_utils::{shell_completions::handle_shell_completions, ParserExt};
use client_metrics::{fields, ClientMetrics};
use color_eyre::owo_colors::OwoColorize;
use colored::Colorize;
use compute::compute_test;
use error::IntoEyre;
use eyre::{eyre, Context, Result};
use log::warn;
use mpc_vm::protocols::MPCProtocol;
use nada_value::json::nada_values_to_json;
use publish::publish_program;
use serde_files_utils::yaml::write_yaml;
use std::{collections::HashMap, io, io::Read, process::exit};

mod args;
mod benchmark;
mod build;
mod compute;
pub(crate) mod error;
mod init;
mod nada_project_toml;
mod new_program;
mod paths;
mod publish;
mod requirements;
mod run;

mod json;
mod program;
mod test;

struct Runner {}

async fn send_client_metrics_event(
    client_metrics: &ClientMetrics,
    command: &str,
    fields: Option<HashMap<String, String>>,
) {
    match client_metrics.send_event(command, fields).await {
        Ok(_) => (),
        Err(e) => warn!("Error sending client metric: {}", e),
    }
}

impl Runner {
    async fn run_command(cli: Cli) -> Result<()> {
        let client_metrics = ClientMetrics::new_default("nada");
        let result = async {
            match cli.command {
                Command::Init(args) => {
                    send_client_metrics_event(&client_metrics, "init", None).await;
                    Self::init(args)?
                }
                Command::Build(args) => {
                    send_client_metrics_event(&client_metrics, "build", None).await;
                    Self::build(&args)?
                }
                Command::Run(args) => {
                    send_client_metrics_event(&client_metrics, "run", fields! {"debug" => args.debug}).await;
                    Self::run(&args)?
                }
                Command::RunJson(args) => {
                    send_client_metrics_event(&client_metrics, "run-json", fields! {"debug" => args.debug}).await;
                    Self::run_json(&args)?
                }
                Command::Test(args) => {
                    send_client_metrics_event(&client_metrics, "test", fields! {"debug" => args.debug}).await;
                    Self::test(args)?
                }
                Command::Benchmark(args) => {
                    send_client_metrics_event(&client_metrics, "benchmark", None).await;
                    Self::benchmark(&args)?
                }
                Command::ProgramRequirements(args) => {
                    send_client_metrics_event(&client_metrics, "get-requirements", None).await;
                    Self::program_requirements(args)?
                }
                Command::GenerateTest(args) => {
                    send_client_metrics_event(&client_metrics, "generate-test", None).await;
                    Self::generate_test(args)?
                }
                Command::ShellCompletions(args) => {
                    send_client_metrics_event(&client_metrics, "shell-completions", fields! {"shell" => args.shell})
                        .await;
                    handle_shell_completions(args, &mut Cli::command())
                }
                Command::ListProtocols => {
                    println!("{:?}", MPCProtocol::list());
                }
                Command::Publish(args) => {
                    send_client_metrics_event(&client_metrics, "publish", None).await;
                    Self::publish(&args).await?
                }
                Command::Compute(args) => {
                    send_client_metrics_event(&client_metrics, "compute", None).await;
                    Self::compute(&args).await?
                }
                Command::NewProgram(args) => {
                    send_client_metrics_event(&client_metrics, "new-program", None).await;
                    Self::new_program(args)?
                }
            }
            Ok(())
        }
        .await;
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                match client_metrics.send_error("error", &e, None).await {
                    Ok(_) => (),
                    Err(e) => warn!("Error sending client metric error: {}", e),
                }
                Err(e)
            }
        }
    }

    fn init(args: InitArgs) -> Result<()> {
        println!("Creating new nada project: {}", args.name.green().bold());
        init::init(args.name)?;
        println!("{}", "Project created!".green().bold());
        Ok(())
    }

    fn test(args: TestArgs) -> Result<()> {
        let conf = NadaProjectToml::find_self()?;
        let mut test_passed = true;

        // run single test
        if let Some(test_name) = args.test {
            let test_case = test::find_test_case_definition(&test_name)
                .with_context(|| format!("failed finding test case {test_name}"))?;
            println!("Building ...");
            let test_case = test_case.build(&conf)?;
            println!("Running ...");
            let test_result = test_case.test(args.debug)?;
            test_passed &= test_result.passed();
            println!("{}", test_result);
        } else {
            // run all tests
            println!("Building ...");
            let test_cases = test::get_all_test_case_definition(&conf)?
                .into_values()
                .map(|test_case| test_case.build(&conf))
                .collect::<Result<Vec<Box<dyn TestCase>>>>()?;
            println!("Running ...");
            for test_case in test_cases {
                let test_result = test_case.test(args.debug)?;
                test_passed &= test_result.passed();
                print!("{}", test_result);
            }
        }
        if test_passed {
            Ok(())
        } else {
            println!("{}", "One or more tests failed".red());
            exit(1)
        }
    }

    fn run(args: &RunArgs) -> Result<()> {
        let conf = NadaProjectToml::find_self()?;
        let test_case = test::find_test_case_definition(&args.test)
            .with_context(|| format!("failed finding test case {}", args.test))?;
        println!(
            "Running program '{}' with inputs from test case {}",
            test_case.program_name().green().bold(),
            args.test.green().bold()
        );
        println!("Building ...");
        let test_case = test_case.build(&conf)?;
        println!("Running ...");

        let (outputs, metrics) = test_case.run(RunOptions {
            debug: args.debug,
            bytecode_json: args.bytecode_json,
            bytecode_text: args.bytecode_text,
            protocols_text: args.protocols_text,
            message_size_compute: args.metrics_message_size,
            execution_plan_metrics: args.metrics_execution_plan,
        })?;
        println!("{}", "Program ran!".green().bold());

        if !args.debug {
            if let Some(metrics) = metrics {
                metrics.standard_output(args.metrics, args.metrics_filepath.as_deref()).into_eyre()?;
            }
        }

        println!("Outputs: {:#?}", outputs);
        Ok(())
    }

    fn run_json(args: &RunJsonArgs) -> Result<()> {
        let conf = NadaProjectToml::find_self()?;
        let program = Program::build(&conf, &args.program)?;
        let mut json_inputs = String::new();
        io::stdin().read_to_string(&mut json_inputs).context("Error reading inputs from stdin")?;
        let inputs = json_to_values(Some(json_inputs), program.program.contract.input_types())
            .context("Parsing program inputs")?;
        let (outputs, _) = run::run_program(
            &program,
            inputs,
            RunOptions {
                debug: args.debug,
                bytecode_json: false,
                bytecode_text: false,
                protocols_text: false,
                message_size_compute: false,
                execution_plan_metrics: false,
            },
        )
        .context("Running program")?;
        let json_outputs = nada_values_to_json(outputs).into_eyre().context("Transforming program outputs to JSON")?;
        let json_outputs = serde_json::to_string(&json_outputs).context("Error serializing outputs to JSON")?;
        println!("{}", json_outputs);
        Ok(())
    }

    fn build(args: &BuildArgs) -> Result<()> {
        if let Some(program) = &args.program {
            println!("Building program: {}", program.green().bold());
            let program_conf = Self::get_program_conf(program)?;
            build::build_program(&program_conf, args.mir_json)?;
            println!("{}", "Build complete!".green().bold());
        } else {
            let conf = NadaProjectToml::find_self()?;
            let programs = conf.get_programs()?;
            for program in programs.values() {
                println!("Building program: {}", program.name.green().bold());
                build::build_program(program, args.mir_json)?;
                println!("{}", "Build complete!".green().bold());
            }
        }
        Ok(())
    }

    async fn publish(args: &PublishArgs) -> Result<()> {
        let program = &args.program;
        println!("Publishing program: {}", program.green().bold());
        let program_conf = Self::get_program_conf(program)?;
        build::build_program(&program_conf, false)?;
        println!("{}", "Build complete!".green().bold());
        let network = &args.network;
        let (program_id, _) = publish_program(network, &program_conf).await?;
        println!("Program ID: {program_id}");
        Ok(())
    }

    async fn compute(args: &ComputeArgs) -> Result<()> {
        let conf = NadaProjectToml::find_self()?;
        let test_definition = test::find_test_case_definition(&args.test)
            .with_context(|| format!("failed finding test case {}", args.test))?;
        println!("Building ...");
        let program_name = test_definition.name();
        let test_case = test_definition.build(&conf)?;
        println!("{}", "Build complete!".green().bold());
        println!(
            "Running program '{}' on network: {}, with inputs from test file {}",
            program_name.green().bold(),
            args.network.green().bold(),
            args.test.green().bold()
        );
        let network = &args.network;
        compute_test(network, test_case).await
    }

    fn program_requirements(args: ProgramRequirementsArgs) -> Result<()> {
        let program_conf = Self::get_program_conf(&args.program)?;
        build::build_program(&program_conf, false)?;
        let requirements = requirements::program_requirements(program_conf)?;
        println!("{}", toml::to_string(&requirements)?);
        Ok(())
    }

    fn generate_test(args: GenerateTestsArgs) -> Result<()> {
        println!("Generating test '{}' for ", args.test_name.green().bold());
        let program_conf = Self::get_program_conf(&args.program)?;
        println!("Building ...");
        build::build_program(&program_conf, false)?;
        println!("Generating test case ...");
        let test_file = staticfile::generate_test_file(&program_conf, args.inputs, args.outputs)?;
        let test_file_path = paths::get_tests_path()?.join(format!("{}.yaml", args.test_name));
        write_yaml(test_file_path, &test_file).into_eyre()?;
        println!("{}", "Test generated!".green().bold());
        Ok(())
    }

    fn get_program_conf(program_name: &String) -> Result<ProgramConf> {
        let conf = NadaProjectToml::find_self()?;
        let programs = conf.get_programs()?;
        let program = programs.get(program_name).ok_or_else(|| eyre!("Program not found: {program_name}"))?;
        Ok(program.clone())
    }

    fn new_program(args: NewProgramArgs) -> Result<()> {
        println!("Creating new program: {}", args.program.green().bold());
        new_program::new_program(args.program, args.prime_size)?;
        println!("{}", "Program created!".green().bold());
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    if let Ok(value) = std::env::var("RUST_LOG") {
        std::env::set_var("RUST_LOG", value + ",bytecode_evaluator=info");
        env_logger::init();
    } else {
        std::env::set_var("RUST_LOG", "bytecode_evaluator=info");
        env_logger::builder()
            .format_target(false)
            .format_level(false)
            .format_timestamp(None)
            .format_module_path(false)
            .init();
    }

    let cli = Cli::parse_with_version();
    Runner::run_command(cli).await?;

    Ok(())
}
