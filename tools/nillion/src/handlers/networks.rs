use super::{open_in_editor, HandlerResult};
use crate::{
    args::{AddNetworkArgs, EditNetworkArgs, NetworksCommand, RemoveNetworkArgs, ShowNetworkArgs},
    serialize::NoOutput,
};
use anyhow::bail;
use serde::Serialize;
use std::{collections::BTreeMap, fs, iter};
use tools_config::{
    networks::{NetworkConfig, NilauthConfig, PaymentsConfig},
    NamedConfig, ToolConfig,
};

pub struct NetworksHandler;

impl NetworksHandler {
    pub fn handle(command: NetworksCommand) -> HandlerResult {
        match command {
            NetworksCommand::Add(args) => Self::add(args),
            NetworksCommand::List => Self::list(),
            NetworksCommand::Edit(args) => Self::edit(args),
            NetworksCommand::Show(args) => Self::show(args),
            NetworksCommand::Remove(args) => Self::remove(args),
        }
    }

    fn add(args: AddNetworkArgs) -> HandlerResult {
        let AddNetworkArgs {
            name,
            bootnode,
            nilchain_rpc_endpoint,
            nilchain_grpc_endpoint,
            nilchain_private_key,
            nilchain_chain_id,
            nilchain_gas_price,
            nilauth_endpoint,
        } = args;
        let payments = nilchain_rpc_endpoint.map(|nilchain_rpc_endpoint| PaymentsConfig {
            nilchain_chain_id,
            nilchain_rpc_endpoint,
            nilchain_grpc_endpoint,
            // Validation is applied via the clap arguments definition
            nilchain_private_key: nilchain_private_key.expect("private key not set"),
            gas_price: nilchain_gas_price,
        });
        let nilauth = nilauth_endpoint.map(|endpoint| NilauthConfig { endpoint });
        NetworkConfig { bootnode, payments, nilauth }.write_to_file(&name)?;
        Ok(Box::new(format!("Network {name} added")))
    }

    fn edit(args: EditNetworkArgs) -> HandlerResult {
        let EditNetworkArgs { name } = args;
        let path = NetworkConfig::config_path(&name)?;
        if !fs::exists(&path).unwrap_or(false) {
            bail!("network file does not exist");
        }
        open_in_editor(&path)?;
        Ok(Box::new(NoOutput))
    }

    fn list() -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            networks: Vec<String>,
        }

        let configs = NetworkConfig::read_all()?;

        let networks = configs.into_iter().map(|NamedConfig { name, .. }| name).collect::<Vec<_>>();
        Ok(Box::new(Output { networks }))
    }

    fn show(args: ShowNetworkArgs) -> HandlerResult {
        let config = NetworkConfig::read_from_config(&args.name)?;
        let NetworkConfig { bootnode, payments, nilauth } = config;
        let mut output = BTreeMap::from([("bootnode", bootnode)]);

        if let Some(payments) = payments {
            let PaymentsConfig {
                nilchain_rpc_endpoint,
                nilchain_grpc_endpoint,
                gas_price,
                nilchain_private_key,
                nilchain_chain_id,
            } = payments;
            let nilchain_private_key: String = nilchain_private_key
                .chars()
                // take the first 3 and replace the rest with * so we don't expose the private key
                .take(3)
                .chain(iter::repeat('*'))
                .take(nilchain_private_key.len())
                .collect();
            output.insert("nilchain_rpc_endpoint", nilchain_rpc_endpoint);
            if let Some(grpc_endpoint) = nilchain_grpc_endpoint {
                output.insert("nilchain_grpc_endpoint", grpc_endpoint);
            }
            output.insert("nilchain_private_key", nilchain_private_key);
            if let Some(chain_id) = nilchain_chain_id {
                output.insert("nilchain_chain_id", chain_id);
            }
            if let Some(gas_price) = gas_price {
                output.insert("nilchain_gas_price", gas_price.to_string());
            }
        }
        if let Some(nilauth) = nilauth {
            let NilauthConfig { endpoint } = nilauth;
            output.insert("nilauth_endpoint", endpoint);
        }
        Ok(Box::new(output))
    }

    fn remove(args: RemoveNetworkArgs) -> HandlerResult {
        NetworkConfig::remove_config(&args.name)?;
        Ok(Box::new(format!("Network {} removed", args.name)))
    }
}
