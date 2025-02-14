//! Nil VM builder.

use crate::{
    grpc::MembershipClient,
    payments::{NilChainPayer, TxHash},
    retry::Retrier,
    vm::{PaymentMode, VmClient, VmClientConfig},
};
use grpc_channel::{token::TokenAuthenticator, AuthenticatedGrpcChannel, GrpcChannelConfig, GrpcChannelError};
use nillion_client_core::values::{PartyId, SecretMasker};
use node_api::{
    auth::rust::UserId,
    membership::rust::{Cluster, Prime},
};
use std::{collections::HashMap, future::Future, sync::Arc, time::Duration};
use tonic::async_trait;
use tracing::{info, warn};
use user_keypair::SigningKey;

const DEFAULT_TOKEN_EXPIRATION: Duration = Duration::from_secs(60);

/// The default maximum payload size for gRPC messages - this can differ for different networks, so it is up to the client to override this when needed.
const DEFAULT_MAX_PAYLOAD_SIZE: usize = 6 * 1024 * 1024;

/// A builder for a [VmClient].
#[derive(Default)]
pub struct VmClientBuilder {
    signing_key: Option<SigningKey>,
    bootnode_url: Option<String>,
    ca_cert: Option<Vec<u8>>,
    certificate_domain: Option<String>,
    auth_token_expiration: Option<Duration>,
    nilchain_payer: Option<Arc<dyn NilChainPayer>>,
    max_payload_size: Option<usize>,
    payment_mode: PaymentMode,
}

impl VmClientBuilder {
    /// Set the signing key to be used for authentication.
    pub fn signing_key(mut self, key: SigningKey) -> Self {
        self.signing_key = Some(key);
        self
    }

    /// Set the URL of the bootnode to use an entry point into the network.
    pub fn bootnode_url<S: Into<String>>(mut self, url: S) -> Self {
        self.bootnode_url = Some(url.into());
        self
    }

    /// Set the root CA certificate to use for certificate validation.
    ///
    /// This should only be used when testing against a local TLS-enabled deployment.
    pub fn ca_cert<T: Into<Vec<u8>>>(mut self, cert: T) -> Self {
        self.ca_cert = Some(cert.into());
        self
    }

    /// Set the certificate domain to check against for all node we connect to.
    ///
    /// This should only be used if [VmClientBuilder::ca_cert] is used.
    pub fn certificate_domain<S: Into<String>>(mut self, domain: S) -> Self {
        self.certificate_domain = Some(domain.into());
        self
    }

    /// Set the authentication token expiration.
    ///
    /// The token is renewed periodically transparently to the user so this only controls how long
    /// each token lives for.
    pub fn auth_token_expiration(mut self, expiration: Duration) -> Self {
        self.auth_token_expiration = Some(expiration);
        self
    }

    /// Configure the payer to use for nilchain payments.
    pub fn nilchain_payer<T>(mut self, payer: T) -> Self
    where
        T: NilChainPayer,
    {
        self.nilchain_payer = Some(Arc::new(payer));
        self
    }

    /// Set the maximum payload size for gRPC messages.
    pub fn max_payload_size(mut self, size: usize) -> Self {
        self.max_payload_size = Some(size);
        self
    }

    /// The payment mode to be used.
    pub fn payment_mode(mut self, mode: PaymentMode) -> Self {
        self.payment_mode = mode;
        self
    }

    /// Build a [VmClient] using the provided configuration.
    pub async fn build(mut self) -> Result<VmClient, BuilderError> {
        use BuilderError::MissingProperty;
        let bootnode_url = self.bootnode_url.take().ok_or(MissingProperty("bootnode_url"))?;
        let keypair = self.signing_key.take().ok_or(MissingProperty("keypair"))?;
        let token_expiration = self.auth_token_expiration.take().unwrap_or(DEFAULT_TOKEN_EXPIRATION);
        let nilchain_payer = match self.nilchain_payer.take() {
            Some(payer) => payer,
            None => {
                info!("No payer set, only operations that don't require payments can be invoked");
                Arc::new(DummyPayer)
            }
        };

        let config = self.build_channel_config(bootnode_url);
        let channel = config.build()?;
        let membership_client = MembershipClient::new(channel.clone());
        let cluster = Self::invoke_membership(&membership_client, |c| c.cluster())
            .await
            .map_err(|e| BuilderError::FetchingCluster(e.to_string()))?;

        let mut channels = HashMap::new();
        let mut parties = Vec::new();
        for member in &cluster.members {
            let authenticator = TokenAuthenticator::new(keypair.clone(), member.identity.clone(), token_expiration);
            let channel =
                self.build_channel_config(member.grpc_endpoint.clone()).authentication(authenticator).build()?;
            let identity: Vec<u8> = member.identity.clone().into();
            let party_id = PartyId::from(identity);
            channels.insert(party_id.clone(), channel);
            parties.push(party_id);
        }
        // The leader is currently part of the cluster but that won't necessarily always be the case.
        let leader_channel = self.build_leader_channel(&keypair, &cluster, &channels, token_expiration).await?;
        let degree = cluster.polynomial_degree as u64;
        let masker = match cluster.prime {
            Prime::Safe64Bits => SecretMasker::new_64_bit_safe_prime(degree, parties),
            Prime::Safe128Bits => SecretMasker::new_128_bit_safe_prime(degree, parties),
            Prime::Safe256Bits => SecretMasker::new_256_bit_safe_prime(degree, parties),
        }
        .map_err(|e| BuilderError::SecretSharer(e.to_string()))?;

        let max_payload_size = self.max_payload_size.unwrap_or(DEFAULT_MAX_PAYLOAD_SIZE);
        let config = VmClientConfig {
            channels,
            leader_channel,
            nilchain_payer,
            cluster,
            masker,
            user_id: UserId::from_bytes(keypair.public_key().as_bytes()),
            max_payload_size,
            payment_mode: self.pick_payment_mode(&membership_client).await?,
        };
        let client = VmClient::new(config);
        Ok(client)
    }

    fn build_channel_config(&self, endpoint: String) -> GrpcChannelConfig {
        let mut config = GrpcChannelConfig::new(endpoint);
        if let Some(cert) = self.ca_cert.clone() {
            config = config.ca_certificate(&cert);
            if let Some(domain) = self.certificate_domain.clone() {
                config = config.domain(domain);
            }
        }
        config
    }

    async fn build_leader_channel(
        &self,
        keypair: &SigningKey,
        cluster: &Cluster,
        channels: &HashMap<PartyId, AuthenticatedGrpcChannel>,
        token_expiration: Duration,
    ) -> Result<AuthenticatedGrpcChannel, BuilderError> {
        let leader_party_id = PartyId::from(Vec::from(cluster.leader.identity.clone()));
        // Check if the leader has the same endpoint in `leader` and in `members`
        let same_leader_endpoint = cluster
            .members
            .iter()
            .any(|m| m.identity == cluster.leader.identity && m.grpc_endpoint == cluster.leader.grpc_endpoint);
        match channels.get(&leader_party_id) {
            // Don't reuse the channel if the leader has a different endpoint in the `leader` field
            Some(channel) if same_leader_endpoint => Ok(channel.clone()),
            _ => {
                let authenticator =
                    TokenAuthenticator::new(keypair.clone(), cluster.leader.identity.clone(), token_expiration);
                Ok(self
                    .build_channel_config(cluster.leader.grpc_endpoint.clone())
                    .authentication(authenticator)
                    .build()?)
            }
        }
    }

    async fn invoke_membership<'a, C, F, O>(client: &'a MembershipClient, callback: C) -> tonic::Result<O>
    where
        C: Fn(&'a MembershipClient) -> F,
        F: Future<Output = tonic::Result<O>>,
    {
        let mut retrier = Retrier::default();
        retrier.add_request("<leader>", client, ());
        retrier.invoke_single(|c, _| callback(c)).await
    }

    async fn pick_payment_mode(&self, client: &MembershipClient) -> Result<PaymentMode, BuilderError> {
        match &self.payment_mode {
            PaymentMode::PayPerOperation => Ok(PaymentMode::PayPerOperation),
            PaymentMode::FromBalance => {
                let version = Self::invoke_membership(client, |c| c.node_version())
                    .await
                    .map_err(|e| BuilderError::FetchingNodeVersion(e.to_string()))?;
                // 0.8.0+ supports this and consider no version == local test run
                let supports_balances = version.version.map(|v| v.major > 0 || v.minor >= 8).unwrap_or(true);
                if supports_balances {
                    Ok(PaymentMode::FromBalance)
                } else {
                    warn!("Node doesn't support balance payments, falling back to paying per operation");
                    Ok(PaymentMode::PayPerOperation)
                }
            }
        }
    }
}

/// An error during the construction of the client.
#[derive(Debug, thiserror::Error)]
pub enum BuilderError {
    /// A required property is missing.
    #[error("required property '{0}' is missing")]
    MissingProperty(&'static str),

    /// An error during the construction of the grpc channel.
    #[error("building grpc channel failed: {0}")]
    GrpcChannel(String),

    /// Failed to get cluster definition.
    #[error("fetching cluster definition failed: {0}")]
    FetchingCluster(String),

    /// Failed to get node version.
    #[error("fetching node version failed: {0}")]
    FetchingNodeVersion(String),

    /// Failed to create secret sharer.
    #[error("creating secret sharer failed: {0}")]
    SecretSharer(String),
}

impl From<GrpcChannelError> for BuilderError {
    fn from(error: GrpcChannelError) -> Self {
        Self::GrpcChannel(error.to_string())
    }
}

struct DummyPayer;

#[async_trait]
impl NilChainPayer for DummyPayer {
    async fn submit_payment(
        &self,
        _amount_unil: u64,
        _resource: Vec<u8>,
    ) -> Result<TxHash, Box<dyn std::error::Error>> {
        Err(NoPayerError.into())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("no payer configured in client")]
struct NoPayerError;
