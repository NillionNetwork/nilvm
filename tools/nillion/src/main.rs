use anyhow::{Context, Result};
use clap::{error::ErrorKind, CommandFactory};
use clap_utils::ParserExt;
use nillion::{
    args::{Cli, Command},
    config::Config,
    context::ContextConfig,
    handlers::{
        context::ContextHandler, identities::IdentitiesHandler, networks::NetworksHandler, nilauth::NilauthHandler,
        nilvm::NilvmHandler, nuc::NucHandler,
    },
    serialize::{serialize_error, serialize_output, NoOutput, SerializeAsAny},
};
use std::{any::TypeId, fs, ops::Deref, path::PathBuf, process::exit};
use tools_config::client::ClientParameters;

fn build_parameters(identity: Option<String>, network: Option<String>) -> ClientParameters {
    match ContextConfig::load() {
        Some(config) => ClientParameters {
            identity: identity.unwrap_or(config.identity),
            network: network.unwrap_or(config.network),
        },
        None => {
            let Some(identity) = identity else {
                Cli::command().error(ErrorKind::MissingRequiredArgument, "identity not provided").exit();
            };
            let Some(network) = network else {
                Cli::command().error(ErrorKind::MissingRequiredArgument, "network not provided").exit();
            };
            ClientParameters { identity, network }
        }
    }
}

async fn run(cli: Cli) -> Result<Box<dyn SerializeAsAny>> {
    let Cli { identity, network, command, .. } = cli;
    match command {
        Command::IdentityGen(args) => IdentitiesHandler::identities_gen(args),
        Command::Identities(command) => IdentitiesHandler::handle(command),
        Command::Networks(command) => NetworksHandler::handle(command),
        Command::Context(command) => ContextHandler::handle(command),
        Command::Nuc(command) => {
            let parameters = build_parameters(identity, network);
            NucHandler::new(parameters).handle(command)
        }
        Command::Nilauth(command) => {
            let parameters = build_parameters(identity, network);
            let handler = NilauthHandler::new(parameters)?;
            handler.handle(command).await
        }
        Command::StoreValues(_)
        | Command::RetrieveValues(_)
        | Command::StoreProgram(_)
        | Command::Compute(_)
        | Command::ClusterInformation
        | Command::DeleteValues(_)
        | Command::PreprocessingPoolStatus(_)
        | Command::InspectIds
        | Command::ShellCompletions(_)
        | Command::RetrievePermissions(_)
        | Command::OverwritePermissions(_)
        | Command::UpdatePermissions(_)
        | Command::Balance(_)
        | Command::Config(_) => {
            let client = build_parameters(identity, network).try_build().await.context("failed to create client")?;
            let handler = NilvmHandler::new(client);
            handler.handle(command).await
        }
    }
}

fn load_config(config_path: PathBuf) -> Result<Config> {
    if fs::exists(&config_path).unwrap_or(true) { Ok(Config::new(config_path)?) } else { Ok(Default::default()) }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let cli = Cli::parse_with_version();
    let config = match load_config(cli.config_path.clone()) {
        Ok(config) => config,
        Err(e) => {
            let output_format = cli.output_format.unwrap_or_default();
            println!("{}", serialize_error(&output_format, &e));
            exit(1);
        }
    };
    let output_format = cli.output_format.clone().or(config.output.format).unwrap_or_default();
    let cmd_result = run(cli).await;

    let serialized_result: Result<Option<String>> = match cmd_result {
        Ok(output) if output.deref().type_id() != TypeId::of::<NoOutput>() => {
            serialize_output(&output_format, output.as_ref()).map(Some)
        }
        Ok(_) => Ok(None),
        Err(e) => Err(e),
    };

    match serialized_result {
        Ok(Some(serialized)) => println!("{}", serialized),
        Ok(None) => {}
        Err(e) => {
            println!("{}", serialize_error(&output_format, &e));
            exit(1);
        }
    }
}
