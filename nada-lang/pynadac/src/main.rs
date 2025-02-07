use anyhow::{anyhow, Context, Error, Result};
use clap::Parser;
use clap_utils::ParserExt;
use client_metrics::ClientMetrics;
use colored::Colorize;
use pynadac::{check_version_matches, CheckVersionError, Compiler, CompilerOptions, PersistOptions};
use std::{fs::create_dir_all, process::exit};

#[derive(Parser, Debug)]
#[clap(author, version, about = "nada compiler for python programs")]
struct Args {
    /// The target directory where output files will be written to.
    #[clap(short, long, default_value = ".")]
    target_dir: String,

    /// Generate a MIR JSON file as an extra output.
    #[clap(short = 'm', long)]
    generate_mir_json: bool,

    /// The path to the program to be compiled.
    program_path: String,
}

fn run(args: Args) -> Result<(), Error> {
    let options = CompilerOptions {
        persist: PersistOptions {
            // We always want the bin mir as that's our true output.
            mir_bin: true,
            mir_json: args.generate_mir_json,
        },
    };
    create_dir_all(&args.target_dir)
        .with_context(|| format!("failed to create target directory: {}", args.target_dir))?;
    let compiler = Compiler::with_options(args.target_dir, options);
    let compiler_output = compiler.compile(&args.program_path)?;
    let validation_result = compiler_output.validation_result;
    if !validation_result.is_successful() {
        validation_result.print(&compiler_output.mir)?;
        return Err(anyhow!("MIR validation failed"));
    }
    Ok(())
}

fn main() -> Result<()> {
    let client_metrics = ClientMetrics::new_default("pynadac");
    client_metrics.send_event_sync("compile", None);
    let args = Args::parse_with_version();
    let version_check_result = check_version_matches();
    if let Err(e) = version_check_result {
        match e {
            CheckVersionError::MissingVersion => println!("{}", format!("WARNING: {e}").yellow().bold()),
            CheckVersionError::InvalidPipShowOutput | CheckVersionError::IncompatibleVersion(_, _) => {
                println!("{}", format!("ERROR: {e}").red().bold());
                exit(1);
            }
        }
    }

    let result = run(args).context("compilation failed");

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            client_metrics.send_error_sync("error", &e, None);
            Err(e)
        }
    }
}
