//! Client creation utilities.

use crate::{
    identities::{Identity, Kind},
    networks::NetworkConfig,
    ToolConfig,
};
use anyhow::{anyhow, Context};
use nillion_client::{
    builder::VmClientBuilder,
    payments::{NillionChainClient, NillionChainClientPayer, NillionChainPrivateKey},
    vm::VmClient,
    Ed25519SigningKey, Secp256k1SigningKey,
};

/// The parameters to create a client.
#[derive(Clone)]
pub struct ClientParameters {
    /// The client's identity.
    pub identity: String,

    /// The network configuration.
    pub network: String,
}

impl ClientParameters {
    pub async fn try_build(self) -> anyhow::Result<VmClient> {
        let ClientParameters { identity, network } = self;
        let NetworkConfig { bootnode, payments } = NetworkConfig::read_from_config(&network)?;
        let Identity { private_key: user_key, kind: curve } = Identity::read_from_config(&identity)?;
        let user_key = user_key.try_into().map_err(|_| anyhow!("invalid user key length"))?;
        let user_key = match curve {
            Kind::Ed25519 => Ed25519SigningKey::from_bytes(&user_key).into(),
            Kind::Secp256k1 => Secp256k1SigningKey::try_from_bytes(&user_key)?.into(),
        };

        let mut builder = VmClientBuilder::default().bootnode_url(bootnode).signing_key(user_key);
        if let Some(payments) = payments {
            let nillion_chain_key = NillionChainPrivateKey::from_hex(&payments.nilchain_private_key)
                .context("invalid payments private key")?;
            let mut nillion_chain_client = NillionChainClient::new(payments.nilchain_rpc_endpoint, nillion_chain_key)
                .await
                .context("creating nilchain client")?;
            if let Some(gas_price) = payments.gas_price {
                nillion_chain_client.set_gas_price(gas_price);
            }
            let payer = NillionChainClientPayer::new(nillion_chain_client);
            builder = builder.nilchain_payer(payer);
        }

        let client = builder.build().await.context("failed to create client")?;
        Ok(client)
    }
}
