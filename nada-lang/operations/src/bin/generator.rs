//! Operations-related files generator.

use std::path::Path;

use anyhow::Error;
use clap::{Parser, ValueEnum};
use operations::output::{
    markdown_table::generate_markdown_tables, nada_tests::generate_tests, nada_types::generate_types,
};

/// Output mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Mode {
    /// Generate Nada types with all corresponding operations.
    NadaTypes,

    /// Generate tests for Nada types.
    NadaTests,

    /// Generate a summary Markdown table.
    MarkdownTable,
}

/// Program arguments.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Output mode.
    #[clap(short, long)]
    mode: Mode,

    /// Target path.
    /// When generating Nada types this is the directory where to generate files.
    /// When generating a Markdown table this is the filepath where to write the table in.
    #[clap(short, long)]
    target: String,

    /// Base path.
    /// When generating Nada types this is the directory where to find the base test file.
    #[clap(short, long)]
    base: String,
}

fn main() -> Result<(), Error> {
    let args: Args = Args::parse();

    let operations = operations::build();
    let target_path: &Path = Path::new(&args.target);
    let base_path: &Path = Path::new(&args.base);

    match args.mode {
        Mode::NadaTypes => generate_types(&operations, target_path)?,
        Mode::NadaTests => generate_tests(&operations, base_path, target_path)?,
        Mode::MarkdownTable => generate_markdown_tables(&operations, target_path)?,
    }

    Ok(())
}
