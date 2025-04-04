use super::{open_in_editor, HandlerResult};
use crate::{
    args::{
        AddIdentityArgs, EditIdentityArgs, IdentitiesCommand, IdentityGenArgs, RemoveIdentityArgs, ShowIdentityArgs,
    },
    serialize::NoOutput,
};
use anyhow::{bail, Result};
use nillion_client::{Ed25519SigningKey, Secp256k1SigningKey, UserId};
use serde::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use std::fs;
use tools_config::{
    identities::{Identity, Kind},
    NamedConfig, ToolConfig,
};
use tracing::info;
use user_keypair::SigningKey;

pub struct IdentitiesHandler;

impl IdentitiesHandler {
    pub fn handle(command: IdentitiesCommand) -> HandlerResult {
        match command {
            IdentitiesCommand::Add(args) => Self::add(args),
            IdentitiesCommand::Edit(args) => Self::edit(args),
            IdentitiesCommand::List => Self::list(),
            IdentitiesCommand::Show(args) => Self::show(args),
            IdentitiesCommand::Remove(args) => Self::remove(args),
        }
    }

    pub fn identities_gen(args: IdentityGenArgs) -> HandlerResult {
        info!("Generating user identities");
        let user_key = Self::generate_key(args.seed, &args.curve)?.as_bytes();
        let identity = Identity { private_key: user_key, kind: args.curve };
        identity.write_to_file(&args.name)?;
        Ok(Box::new(format!("Identity {} generated", args.name)))
    }

    fn add(args: AddIdentityArgs) -> HandlerResult {
        let kind = Kind::Secp256k1;
        let user_key = Self::generate_key(args.seed, &kind)?.as_bytes();
        let identity = Identity { private_key: user_key, kind };
        identity.write_to_file(&args.name)?;
        Ok(Box::new(format!("Identity {} added", args.name)))
    }

    fn list() -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            identities: Vec<String>,
        }

        let identities = Identity::read_all()?;
        let identities = identities.into_iter().map(|NamedConfig { name, .. }| name).collect::<Vec<_>>();
        Ok(Box::new(Output { identities }))
    }

    fn edit(args: EditIdentityArgs) -> HandlerResult {
        let EditIdentityArgs { name } = args;
        let path = Identity::config_path(&name)?;
        if !fs::exists(&path).unwrap_or(false) {
            bail!("identity file does not exist");
        }
        open_in_editor(&path)?;
        Ok(Box::new(NoOutput))
    }

    fn show(args: ShowIdentityArgs) -> HandlerResult {
        #[serde_as]
        #[derive(Serialize)]
        struct Output {
            #[serde_as(as = "DisplayFromStr")]
            user_id: UserId,

            #[serde(serialize_with = "hex::serde::serialize")]
            public_key: Vec<u8>,

            #[serde_as(as = "DisplayFromStr")]
            kind: Kind,
        }

        let Identity { private_key: user_key, kind } = Identity::read_from_config(&args.name)?;
        let private_key = match kind {
            Kind::Ed25519 => SigningKey::from(Ed25519SigningKey::try_from(user_key.as_ref())?),
            Kind::Secp256k1 => SigningKey::from(Secp256k1SigningKey::try_from(user_key.as_ref())?),
        };
        let public_key = private_key.public_key().as_bytes();
        let user_id = UserId::from_bytes(&public_key);
        Ok(Box::new(Output { public_key, user_id, kind }))
    }

    fn remove(args: RemoveIdentityArgs) -> HandlerResult {
        Identity::remove_config(&args.name)?;
        Ok(Box::new(format!("Identity {} removed", args.name)))
    }

    fn generate_key(seed: Option<String>, curve: &Kind) -> Result<SigningKey> {
        let key = match (seed, curve) {
            (Some(seed), Kind::Ed25519) => {
                info!("Generating ed25519 key using provided seed");
                Ed25519SigningKey::from_seed(&seed).into()
            }
            (None, Kind::Ed25519) => {
                info!("Generating random ed25519 key");
                Ed25519SigningKey::generate().into()
            }
            (Some(seed), Kind::Secp256k1) => {
                info!("Generating secp256k1 key using provided seed");
                Secp256k1SigningKey::try_from_seed(&seed)?.into()
            }
            (None, Kind::Secp256k1) => {
                info!("Generating random secp256k1 key");
                Secp256k1SigningKey::generate().into()
            }
        };
        Ok(key)
    }
}
