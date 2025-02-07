//! `nilup` is a tool to manage the installation/versions of Nillion SDKs.
#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod args;
mod repository;
#[cfg(target_os = "macos")]
mod run_command;

use crate::{
    args::{Cli, Command, InstallArgs, InstrumentationCommand, ListAvailableArgs, UninstallArgs, UpdateArgs, UseArgs},
    repository::Repository,
};
use clap::CommandFactory;
use clap_utils::{shell_completions::handle_shell_completions, ParserExt};
use client_metrics::{fields, ClientMetrics};
use color_eyre::owo_colors::OwoColorize;
use eyre::{eyre, Result, WrapErr};
use file_find::find_file_with_parents;
use regex::Regex;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::BufRead,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Deserialize, Serialize)]
struct NilSDK {
    version: String,
}

struct Runner {
    repository: Repository,
    nilup_path: PathBuf,
    sdk_path: PathBuf,
    default_sdk_path: PathBuf,
    client_metrics: ClientMetrics,
}

impl Runner {
    fn new(repository: Repository, client_metrics: ClientMetrics) -> Result<Self> {
        let nilup_path = dirs::home_dir().ok_or_else(|| eyre!("Error getting home directory"))?.join(".nilup");
        let sdk_path = nilup_path.join("sdks");
        let default_sdk_path = nilup_path.join("default-nil-sdk.toml");

        Ok(Self { repository, nilup_path, sdk_path, default_sdk_path, client_metrics })
    }

    async fn run(&mut self, cli: Cli) -> Result<()> {
        match cli.command {
            Command::Init => self.init().await?,
            Command::Update(args) => self.update(args).await?,
            Command::Install(args) => self.install(args).await?,
            Command::Uninstall(args) => self.uninstall(args).await?,
            Command::Use(args) => self.use_version(args).await?,
            Command::ListAvailable(args) => self.list_available(args).await?,
            Command::ListInstalled => self.list_installed()?,
            Command::ShellCompletions(args) => handle_shell_completions(args, &mut Cli::command()),
            Command::Instrumentation(args) => match args.command {
                InstrumentationCommand::Enable { wallet } => self.enable_instrumentation(wallet)?,
                InstrumentationCommand::Disable => ClientMetrics::disable().map_err(|e| eyre!(Box::new(e)))?,
            },
        }
        Ok(())
    }

    /// Enable instrumentation asking the user for consent
    fn enable_instrumentation(&self, wallet: Option<String>) -> Result<()> {
        println!(
            "By providing your Ethereum wallet address, you consent to the collection of telemetry data by the Nillion Network."
        );
        println!("That includes but is not limited to");
        println!("- The version of the SDK you are using");
        println!("- The OS you are using");
        println!("- The Processor Architecture you are using");
        println!("- The SDK binary that you are running and the subcommand");
        println!("- The wallet address you provided");
        println!("- The errors produced by the SDK");
        println!(
            "We collect this data to understand how the software is used, and to better assist you in case of issues."
        );
        println!(
            "While we will not collect any personal information, we still recommend using a new wallet address that cannot be linked to your identity by any third party."
        );
        println!("For more information, our privacy policy is available at https://nillion.com/privacy/.");
        println!("Do you consent to the collection of telemetry data? (yes/no)");
        let stdin = std::io::stdin();
        if let Some(Ok(line)) = stdin.lock().lines().next() {
            if line.to_lowercase() == "yes" || line.to_lowercase() == "y" {
                println!("Telemetry data collection enabled");
                ClientMetrics::enable(wallet).map_err(|e| eyre!(Box::new(e)))?;
            } else {
                println!("Telemetry data collection not enabled");
            }
        }
        Ok(())
    }

    /// Init the nilup tool
    /// Creates the .nilup directory in the home directory
    /// Creates the sdks directory in the .nilup directory
    /// Creates links for all SDK binaries to nilup to be used as proxy
    /// Installs the latest version of the SDK
    /// Sets the latest version of the SDK as default
    async fn init(&mut self) -> Result<()> {
        self.client_metrics.send_event("init".to_string(), None).await?;
        create_dir(&self.nilup_path).await?;
        create_dir(&self.sdk_path).await?;

        let nilup_bin_path = self.nilup_path.join("bin");
        let nilup_binary_path = self.nilup_path.join("bin").join("nilup");

        create_symlink(&nilup_binary_path, &nilup_bin_path.join("nillion")).await?;
        create_symlink(&nilup_binary_path, &nilup_bin_path.join("nada-run")).await?;
        create_symlink(&nilup_binary_path, &nilup_bin_path.join("pynadac")).await?;
        create_symlink(&nilup_binary_path, &nilup_bin_path.join("nillion-devnet")).await?;
        create_symlink(&nilup_binary_path, &nilup_bin_path.join("nada")).await?;
        create_symlink(&nilup_binary_path, &nilup_bin_path.join("nilchaind")).await?;
        // TODO create completions proxy for zsh and bash

        self.install(InstallArgs { version: "latest".to_string() }).await?;
        self.use_version(UseArgs { version: "latest".to_string() }).await?;
        Ok(())
    }

    async fn update(&mut self, args: UpdateArgs) -> Result<()> {
        let version = args.version.unwrap_or("latest".to_string());
        self.client_metrics.send_event("update", fields! {"version" => version}).await?;

        self.install(InstallArgs { version: version.clone() }).await?;

        // we execvp the cp command so that our current process is replaced by the cp command
        // so the nilup binary that we are going to update is not used to then be able to overwrite
        // the nilup binary that was previously on use
        let err = std::process::Command::new("cp")
            .args(&[self.sdk_path.join(version).join("nilup"), self.nilup_path.join("bin").join("nilup")])
            .exec();

        Err(err.into())
    }

    /// Install a version of the Nillion SDK
    /// If nada_dsl is true it will install the nada_dsl package
    /// If python_client is true it will install the python client package
    /// If browser_client is true it will install the browser client package
    async fn install(&mut self, InstallArgs { version }: InstallArgs) -> Result<()> {
        self.client_metrics
            .send_event(
                "install",
                fields! {
                    "version" => version,
                },
            )
            .await?;

        if !self.repository.version_exist(&version).await? {
            println!("Version {version} not found");
            return Ok(());
        }

        let sdk_path = self.sdk_path.join(&version);

        let is_reinstalling = dir_exists(&sdk_path);

        if is_reinstalling {
            println!("SDK version {version} already installed");
            remove_dir(&sdk_path).await?;
            println!("Reinstalling SDK version {version}");
            // TODO install completions
        } else {
            println!("Installing SDK bins version {version}");
        }

        create_dir(&sdk_path).await?;

        if let Err(e) = self.repository.download_sdk_bins(&version, &sdk_path).await {
            remove_dir(&sdk_path).await?;
            return Err(e);
        };

        println!("SDK version {version} installed");

        // Print release notes URL if a release candidate is installed.
        let rc_pattern = Regex::new(r"-rc\.\d+$")?;

        if rc_pattern.is_match(&version) {
            // Check presence of RELEASE_NOTES.html via HEAD request before printing.
            let client = Client::new();
            let release_notes_url = format!("https://releases.nilogy.xyz/sdk-rc/{version}/RELEASE_NOTES.html");

            // Send request.
            let response = client.head(&release_notes_url).timeout(Duration::from_secs(1)).send().await;

            if let Ok(response) = response {
                if response.status() == StatusCode::OK {
                    println!("\nFind release notes at {release_notes_url}");
                }
            }
        }

        Ok(())
    }

    /// Uninstall a version of the Nillion SDK
    async fn uninstall(&self, UninstallArgs { version }: UninstallArgs) -> Result<()> {
        self.client_metrics.send_event("uninstall".to_string(), fields! {"version" => version}).await?;

        remove_dir(&self.sdk_path.join(&version)).await?;
        println!("SDK version {version} uninstalled");
        Ok(())
    }
    /// Generate shell completions
    async fn use_version(&self, UseArgs { version }: UseArgs) -> Result<()> {
        self.client_metrics
            .send_event(
                "use-version".to_string(),
                fields! {
                    "version" => version
                },
            )
            .await?;

        self.create_default_sdk_file(version.clone())?;
        println!("SDK version {version} set as default");
        Ok(())
    }

    /// List available versions of the Nillion SDK
    async fn list_available(&self, args: ListAvailableArgs) -> Result<()> {
        let versions = self.repository.list_versions(args.rc).await?;
        for version in versions {
            println!("{}", version);
        }
        Ok(())
    }

    /// List installed versions of the Nillion SDK
    fn list_installed(&self) -> Result<()> {
        let current_version = self.read_default_sdk_file().unwrap_or_default();
        for entry in fs::read_dir(&self.sdk_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let version = path.file_name().unwrap().to_string_lossy();
                if current_version == version {
                    println!("{}", format!("* {}", version).bold().green());
                } else {
                    println!("  {}", version)
                }
            }
        }
        Ok(())
    }

    /// Create default-nil-sdk.toml file with the default version of the SDK
    fn create_default_sdk_file(&self, version: String) -> Result<()> {
        let nil_sdk = NilSDK { version };
        let nil_sdk_toml = toml::to_string(&nil_sdk).wrap_err("Error serializing nil-sdk.toml")?;

        fs::write(&self.default_sdk_path, nil_sdk_toml)
            .wrap_err(format!("Error writing file {}", self.default_sdk_path.to_string_lossy()))?;

        Ok(())
    }

    /// Reads the current version used from default-nil-sdk.toml
    fn read_default_sdk_file(&self) -> Result<String> {
        let default_sdk = fs::read_to_string(self.default_sdk_path.clone())
            .wrap_err(format!("Error reading current version file: {}", self.default_sdk_path.to_string_lossy()))?;
        let nil_sdk: NilSDK = toml::from_str(&default_sdk)
            .wrap_err(format!("Error de-serialising version from {}", self.default_sdk_path.to_string_lossy()))?;
        Ok(nil_sdk.version)
    }
}

async fn read_nil_sdk(path: &PathBuf) -> Result<NilSDK> {
    let nil_sdk_toml =
        tokio::fs::read_to_string(path).await.wrap_err(format!("Error reading file {}", path.to_string_lossy()))?;
    let nil_sdk = toml::from_str(&nil_sdk_toml).wrap_err("Error deserializing nil-sdk.toml")?;
    Ok(nil_sdk)
}

fn dir_exists(path: &Path) -> bool {
    path.exists() && path.is_dir()
}

async fn create_symlink(source: &PathBuf, destination: &PathBuf) -> Result<()> {
    // Check if the link already exists. If it does, check whether it already points to source. If
    // it already points to source, then return Ok.
    if tokio::fs::metadata(destination.as_path()).await.is_ok() {
        let dest_link = tokio::fs::read_link(destination)
            .await
            .wrap_err(format!("Error reading link {}", destination.to_string_lossy()))?;

        if dest_link == *source {
            return Ok(());
        }
    }

    tokio::fs::symlink(source, destination).await.wrap_err(format!(
        "Error creating symlink from {} to {}",
        source.to_string_lossy(),
        destination.to_string_lossy()
    ))?;
    Ok(())
}

async fn create_dir(path: &PathBuf) -> Result<()> {
    tokio::fs::create_dir_all(path).await.wrap_err(format!("Error creating directory {}", path.to_string_lossy()))?;
    Ok(())
}

async fn remove_dir(path: &PathBuf) -> Result<()> {
    tokio::fs::remove_dir_all(path).await.wrap_err(format!("Error removing directory {}", path.to_string_lossy()))?;
    Ok(())
}

/// Main function of `nilup` it will check if the program name is `nilup` or `<path>/nilup` and call nilup main
/// If not then we are being called as a proxy for another binary of Nillion SDK and we will call proxy main
#[tokio::main]
async fn main() -> Result<()> {
    if let Some(program_name) = std::env::args().next() {
        if program_name == "nilup" || program_name.ends_with("/nilup") {
            nilup_main().await
        } else {
            proxy_main(program_name).await
        }
    } else {
        Err(eyre!("program name not found"))
    }
}

/// We see if the version needed is installed and if not we download it.
/// Then we call the right version of the binary.
async fn proxy_call(program_name: String, version: String, args: Vec<String>) -> Result<()> {
    // TODO if program is pynadac check if nada_dsl is installed and has the right version, if not install it

    let version_path = dirs::home_dir()
        .ok_or_else(|| eyre!("Error getting home directory"))?
        .join(".nilup")
        .join("sdks")
        .join(&version);

    if !dir_exists(&version_path) {
        create_dir(&version_path).await?;
        let mut repository = Repository::new().await;
        if let Err(e) = repository.download_sdk_bins(&version, &version_path).await {
            remove_dir(&version_path).await?;
            return Err(e);
        };
    }

    let program_path = version_path.join(&program_name);

    let mut command = std::process::Command::new(program_path);
    command.args(args);
    let status = command.status()?;
    std::process::exit(status.code().unwrap_or(1));
}

/// We are being called as a proxy for another binary of Nillion SDK
/// We will check if the first argument is a version flag and call the proxy main with the version
/// If not we will read the nil-sdk.toml or .nil-sdk.toml file in current path or parent paths and call the proxy main with the version of this file
/// If the nil-sdk.toml file is not found we will call the proxy main with the default-nil-sdk.toml version
async fn proxy_main(program_name: String) -> Result<()> {
    let nilup_path = dirs::home_dir().ok_or_else(|| eyre!("Error getting home directory"))?.join(".nilup");

    let flag_version = env::args().nth(1).and_then(|mut first_arg| {
        if first_arg.starts_with('+') {
            first_arg.remove(0);
            Some(first_arg)
        } else {
            None
        }
    });

    if let Some(flag_version) = flag_version {
        let args = std::env::args().skip(2).collect::<Vec<String>>();
        proxy_call(program_name, flag_version, args).await?;
        return Ok(());
    }

    let nil_sdk_path = find_file_with_parents("nil-sdk.toml")
        .or_else(|_| find_file_with_parents(".nil-sdk.toml"))
        .unwrap_or_else(|_| nilup_path.join("default-nil-sdk.toml"));

    let version = read_nil_sdk(&nil_sdk_path).await?.version;
    let args = env::args().skip(1).collect::<Vec<String>>();
    proxy_call(program_name, version, args).await?;
    Ok(())
}

/// Main of `nilup` tool that will parse the command line arguments and run the subcommand
async fn nilup_main() -> Result<()> {
    color_eyre::install()?;

    let cli = Cli::parse_with_version();
    let client_metrics = ClientMetrics::new_default("nilup");
    let repository = Repository::new().await;
    let mut runner = Runner::new(repository, client_metrics.clone())?;
    let result = runner.run(cli).await;

    match result {
        Ok(_) => Ok(()),
        Err(e) => {
            let _ = client_metrics.send_error("error", &e, None).await;
            Err(e)
        }
    }
}
