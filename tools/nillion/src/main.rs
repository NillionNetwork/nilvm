use anyhow::{Context, Result};
use clap::{error::ErrorKind, CommandFactory};
use clap_utils::ParserExt;
use nillion::{
    args::{Cli, Command, ContextCommand, IdentitiesCommand, NetworksCommand},
    config::Config,
    context::ContextConfig,
    runner::Runner,
    serialize::{serialize_error, serialize_output, NoOutput, SerializeAsAny},
};
use std::{any::TypeId, fs, ops::Deref, path::PathBuf, process::exit};
use tools_config::client::ClientParameters;

async fn run(cli: Cli) -> Result<Box<dyn SerializeAsAny>> {
    match cli.command {
        Command::IdentityGen(args) => return Runner::identities_gen(args),
        Command::Identities(command) => {
            return match command {
                IdentitiesCommand::Add(args) => Runner::add_identity(args),
                IdentitiesCommand::Edit(args) => Runner::edit_identity(args),
                IdentitiesCommand::List => Runner::list_identities(),
                IdentitiesCommand::Show(args) => Runner::show_identity(args),
                IdentitiesCommand::Remove(args) => Runner::remove_identity(args),
            };
        }
        Command::Networks(command) => {
            return match command {
                NetworksCommand::Add(args) => Runner::add_network(args),
                NetworksCommand::Edit(args) => Runner::edit_network(args),
                NetworksCommand::List => Runner::list_networks(),
                NetworksCommand::Show(args) => Runner::show_network(args),
                NetworksCommand::Remove(args) => Runner::remove_network(args),
            };
        }
        Command::Context(command) => match command {
            ContextCommand::Use(args) => return Runner::use_context(args),
            ContextCommand::Show => return Runner::show_context(),
        },
        _ => (),
    }
    let Cli { identity, network, command, .. } = cli;
    let parameters = match ContextConfig::load() {
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
    };
    let client = parameters.try_build().await.context("failed to create client")?;
    let cli_runner = Runner::new(client);
    cli_runner.run(command).await
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
