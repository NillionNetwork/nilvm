use super::HandlerResult;
use crate::args::{CheckRevokedArgs, NilauthCommand, NilauthSubscriptionCommand, RevokeTokenArgs};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use nilauth_client::client::{About, DefaultNilauthClient, NilauthClient};
use nillion_client::payments::{NillionChainClient, NillionChainPrivateKey};
use nillion_nucs::{
    envelope::NucTokenEnvelope,
    k256::SecretKey,
    token::{Did, ProofHash},
};
use serde::Serialize;
use tools_config::{
    client::ClientParameters,
    identities::{Identity, Kind},
    networks::{NetworkConfig, PaymentsConfig},
    ToolConfig,
};

pub struct NilauthHandler {
    key: SecretKey,
    payments_config: Option<PaymentsConfig>,
    client: DefaultNilauthClient,
}

impl NilauthHandler {
    pub fn new(parameters: ClientParameters) -> Result<Self> {
        let identity = Identity::read_from_config(&parameters.identity)?;
        let key = match identity.kind {
            Kind::Secp256k1 => SecretKey::from_slice(&identity.private_key)?,
            Kind::Ed25519 => bail!("ed25519 not supported"),
        };
        let config = NetworkConfig::read_from_config(&parameters.network)?;
        let payments_config = config.payments;
        let nilauth_config = config.nilauth.ok_or_else(|| anyhow!("no nilauth config"))?;
        let client = DefaultNilauthClient::new(nilauth_config.endpoint.clone())?;
        Ok(Self { key, payments_config, client })
    }

    pub async fn handle(self, command: NilauthCommand) -> HandlerResult {
        use NilauthCommand::*;
        match command {
            Subscription(NilauthSubscriptionCommand::Pay) => self.pay_subscription().await,
            Subscription(NilauthSubscriptionCommand::Status) => self.subscription_status().await,
            Token => self.request_token().await,
            Revoke(args) => self.revoke_token(args).await,
            CheckRevoked(args) => self.check_revoked(args).await,
            About => self.about().await,
        }
    }

    async fn pay_subscription(self) -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            tx_hash: String,
        }

        let payments = self.payments_config.ok_or_else(|| anyhow!("no payments config"))?;
        let nilchain_key =
            NillionChainPrivateKey::from_hex(&payments.nilchain_private_key).context("invalid payments private key")?;
        let mut nilchain_client = NillionChainClient::new(payments.nilchain_rpc_endpoint, nilchain_key)
            .await
            .context("creating nilchain client")?;
        if let Some(gas_price) = payments.gas_price {
            nilchain_client.set_gas_price(gas_price);
        }

        let tx_hash = self.client.pay_subscription(&mut nilchain_client, &self.key).await?;
        Ok(Box::new(Output { tx_hash: tx_hash.to_string() }))
    }

    async fn subscription_status(self) -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            subscribed: bool,

            #[serde(skip_serializing_if = "Option::is_none")]
            expires_at: Option<DateTime<Utc>>,
        }

        let subscription = self.client.subscription_status(&self.key).await?;
        let output =
            Output { subscribed: subscription.subscribed, expires_at: subscription.details.map(|s| s.expires_at) };
        Ok(Box::new(output))
    }

    async fn request_token(self) -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            token: String,
        }

        let token = self.client.request_token(&self.key).await?;
        Ok(Box::new(Output { token }))
    }

    async fn revoke_token(self, args: RevokeTokenArgs) -> HandlerResult {
        let token = NucTokenEnvelope::decode(&args.token)?;
        self.client.revoke_token(&token, &self.key).await?;
        Ok(Box::new("Token revoked".to_string()))
    }

    async fn check_revoked(self, args: CheckRevokedArgs) -> HandlerResult {
        #[derive(Serialize)]
        struct Token {
            hash: ProofHash,
            revoked_at: DateTime<Utc>,
        }

        #[derive(Serialize)]
        struct Output {
            tokens: Vec<Token>,
        }

        let token = NucTokenEnvelope::decode(&args.token)?;
        let tokens = self.client.lookup_revoked_tokens(&token).await?;
        let tokens = tokens.into_iter().map(|t| Token { hash: t.token_hash, revoked_at: t.revoked_at }).collect();
        Ok(Box::new(Output { tokens }))
    }

    async fn about(self) -> HandlerResult {
        #[derive(Serialize)]
        struct Output {
            #[serde(with = "hex::serde")]
            public_key: [u8; 33],

            identity: Did,
        }

        let about = self.client.about().await?;
        let About { public_key } = about;
        let identity = Did::new(public_key);
        Ok(Box::new(Output { public_key, identity }))
    }
}
