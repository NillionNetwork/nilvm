//! The command line argument types.

use clap::{Args, Parser, Subcommand};
use clap_utils::shell_completions::ShellCompletionsArgs;

#[derive(Parser)]
pub struct Cli {
    /// The command to be run.
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Install a version of the Nillion SDK
    Install(InstallArgs),
    /// Uninstall a version of the Nillion SDK
    Uninstall(UninstallArgs),
    /// Set a global version of the Nillion SDK to be used
    Use(UseArgs),
    /// List available versions of the Nillion SDK
    ListAvailable(ListAvailableArgs),
    /// List installed versions of the Nillion SDK
    ListInstalled,
    #[clap(hide = true)]
    Init,
    /// Update nilup
    Update(UpdateArgs),
    /// Enable/Disable instrumentation
    Instrumentation(InstrumentationArgs),
    /// Generate shell completions
    ShellCompletions(ShellCompletionsArgs),
}
#[derive(Args)]
pub struct ListAvailableArgs {
    /// List the release candidates
    #[clap(long, default_value = "false")]
    pub(crate) rc: bool,
}

#[derive(Args)]
pub struct UpdateArgs {
    /// The version of nilup to update to, if not provided the latest version will be installed.
    pub version: Option<String>,
}

#[derive(Args)]
pub struct InstallArgs {
    /// The version of the Nillion SDK to install.
    pub version: String,
}

#[derive(Args)]
pub struct UninstallArgs {
    /// The version of the Nillion SDK to uninstall.
    pub version: String,
}

#[derive(Args)]
pub struct UseArgs {
    /// The version of the Nillion SDK to use as default.
    pub version: String,
}

#[derive(Args)]
pub struct InstrumentationArgs {
    /// The subcommand to be run.
    #[command(subcommand)]
    pub command: InstrumentationCommand,
}

#[derive(Subcommand)]
pub enum InstrumentationCommand {
    /// Enable instrumentation
    Enable {
        #[clap(long, short)]
        wallet: Option<String>,
    },
    /// Disable instrumentation
    Disable,
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
