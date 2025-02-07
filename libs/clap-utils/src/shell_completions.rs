//! This module contains the `shell-completions` subcommand args and handler.
/// needs to be enabled with the `shell-completions` feature flag.
/// Usage:
/// ```no_run
/// use clap_utils::shell_completions::{handle_shell_completions, ShellCompletionsArgs};
/// use clap::{Args, Parser, Subcommand, Command, CommandFactory};
///
/// #[derive(Parser)]
/// pub struct Cli {
///     #[command(subcommand)]
///     pub command: MySubcommand,
/// }
///
/// #[derive(Subcommand)]
/// pub enum MySubcommand {
///     MySubcommand,
///     /// Generate shell completions
///     ShellCompletions(ShellCompletionsArgs),
/// }
///
/// let cli = Cli::parse();
///
/// match cli.command {
///     MySubcommand::MySubcommand => println!("MySubcommand"),
///     MySubcommand::ShellCompletions(args) => {
///         let mut cmd = Cli::command();
///         handle_shell_completions(args, &mut cmd)
///     }
/// }
///
///```
use clap::{Args, Command, ValueEnum};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};
use std::io;

fn serialize_shell<S>(shell: &Shell, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&shell.to_string())
}

fn deserialize_shell<'de, D>(deserializer: D) -> Result<Shell, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Shell::from_str(&s, false).map_err(serde::de::Error::custom)
}

/// The arguments for the `shell-completions` subcommand.
#[derive(Args, Serialize, Deserialize, Debug, PartialEq)]
pub struct ShellCompletionsArgs {
    /// The Shell to generate the completions for
    #[arg(long, short, value_enum)]
    #[serde(serialize_with = "serialize_shell")]
    #[serde(deserialize_with = "deserialize_shell")]
    pub shell: Shell,
}

/// Handle the `shell-completions` subcommand.
/// Will generate the completions for the given shell and write them to the appropriate location.
/// if `print` is true, the completions will be printed to stdout.
pub fn handle_shell_completions(args: ShellCompletionsArgs, cmd: &mut Command) {
    let name = cmd.get_name().to_string();
    clap_complete::generate(args.shell, cmd, name, &mut io::stdout());
}
