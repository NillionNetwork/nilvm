use super::HandlerResult;
use crate::{
    args::{
        ContextCommand, IdentitiesCommand, NetworksCommand, ShowContextArgs, ShowIdentityArgs, ShowNetworkArgs,
        UseContextArgs,
    },
    context::ContextConfig,
    handlers::{identities::IdentitiesHandler, networks::NetworksHandler},
    serialize::SerializeAsAny,
};
use anyhow::bail;
use serde::Serialize;
use std::collections::HashMap;
use tools_config::{identities::Identity, networks::NetworkConfig, ToolConfig};

pub struct ContextHandler;

impl ContextHandler {
    pub fn handle(command: ContextCommand) -> HandlerResult {
        match command {
            ContextCommand::Use(args) => Self::use_context(args),
            ContextCommand::Show(ShowContextArgs { verbose: true }) => Self::show_detailed(),
            ContextCommand::Show(ShowContextArgs { verbose: false }) => Self::show(),
        }
    }

    pub fn use_context(args: UseContextArgs) -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            identity: String,
            network: String,
        }

        let UseContextArgs { identity, network }: UseContextArgs = args;
        if let Err(e) = Identity::read_from_config(&identity) {
            bail!("Invalid identity: {e}");
        }
        if let Err(e) = NetworkConfig::read_from_config(&network) {
            bail!("Invalid network: {e}");
        }
        let config = ContextConfig { identity: identity.clone(), network: network.clone() };
        config.store()?;
        Ok(Box::new(Output { identity, network }))
    }

    pub fn show() -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            identity: String,
            network: String,
        }

        let Some(context) = ContextConfig::load() else {
            return Ok(Box::new(HashMap::<(), ()>::new()));
        };
        let ContextConfig { identity, network } = context;
        Ok(Box::new(Output { identity, network }))
    }

    pub fn show_detailed() -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            identity: Box<dyn SerializeAsAny>,
            network: Box<dyn SerializeAsAny>,
        }

        let Some(context) = ContextConfig::load() else {
            return Ok(Box::new(HashMap::<(), ()>::new()));
        };
        let ContextConfig { identity, network } = context;
        let identity = IdentitiesHandler::handle(IdentitiesCommand::Show(ShowIdentityArgs { name: identity }))?;
        let network = NetworksHandler::handle(NetworksCommand::Show(ShowNetworkArgs { name: network }))?;
        Ok(Box::new(Output { identity, network }))
    }
}
