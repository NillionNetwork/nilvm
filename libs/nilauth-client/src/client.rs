use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nillion_chain_client::{client::NillionChainClient, transactions::TokenAmount};
use nillion_nucs::k256::{
    ecdsa::{signature::Signer, Signature, SigningKey},
    sha2::{Digest, Sha256},
    PublicKey, SecretKey,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const TOKEN_REQUEST_EXPIRATION: Duration = Duration::from_secs(60);

/// An interface to interact with nilauth.
#[async_trait]
pub trait NilauthClient {
    /// Get information about the nilauth instance.
    async fn about(&self) -> Result<About, AboutError>;

    /// Request a token for the given private key.
    async fn request_token(&self, key: &SecretKey) -> Result<String, RequestTokenError>;

    /// Pay for a subscription.
    async fn pay_subscription(
        &self,
        payments_client: &mut NillionChainClient,
        key: &PublicKey,
    ) -> Result<TxHash, PaySubscriptionError>;

    /// Get the cost of a subscription.
    async fn subscription_cost(&self) -> Result<TokenAmount, SubscriptionCostError>;
}

/// An error when requesting a token.
#[derive(Debug, thiserror::Error)]
pub enum RequestTokenError {
    #[error("fetching server's about: {0}")]
    About(#[from] AboutError),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid public key")]
    InvalidPublicKey,

    #[error("request: {0}")]
    Request(#[from] reqwest::Error),
}

/// An error when paying a subscription.
#[derive(Debug, thiserror::Error)]
pub enum PaySubscriptionError {
    #[error("fetching server's about: {0}")]
    About(#[from] AboutError),

    #[error("fetching subscription cost: {0}")]
    Cost(#[from] SubscriptionCostError),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("invalid public key")]
    InvalidPublicKey,

    #[error("request: {0}")]
    Request(#[from] reqwest::Error),

    #[error("making payment: {0}")]
    Payment(String),
}

/// An error when fetching the subscription cost.
#[derive(Debug, thiserror::Error)]
pub enum SubscriptionCostError {
    #[error("request: {0}")]
    Request(#[from] reqwest::Error),
}

/// An error when requesting the information about a nilauth instance.
#[derive(Debug, thiserror::Error)]
pub enum AboutError {
    #[error("request: {0}")]
    Request(#[from] reqwest::Error),
}

/// The default nilauth client that hits the actual service.
pub struct DefaultNilauthClient {
    client: reqwest::Client,
    base_url: String,
}

impl DefaultNilauthClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    fn make_url(&self, path: &str) -> String {
        let base_url = &self.base_url;
        format!("{base_url}{path}")
    }
}

#[async_trait]
impl NilauthClient for DefaultNilauthClient {
    async fn about(&self) -> Result<About, AboutError> {
        let url = self.make_url("/about");
        let about = self.client.get(url).send().await?.json().await?;
        Ok(about)
    }

    async fn request_token(&self, key: &SecretKey) -> Result<String, RequestTokenError> {
        let about = self.about().await?;
        let payload = CreateNucRequestPayload {
            nonce: rand::random(),
            expires_at: Utc::now() + TOKEN_REQUEST_EXPIRATION,
            target_public_key: about.public_key,
        };
        let payload = serde_json::to_string(&payload)?;
        let signature: Signature = SigningKey::from(key).sign(payload.as_bytes());

        let public_key =
            key.public_key().to_sec1_bytes().as_ref().try_into().map_err(|_| RequestTokenError::InvalidPublicKey)?;
        let request =
            CreateNucRequest { public_key, signature: signature.to_bytes().into(), payload: payload.into_bytes() };
        let url = self.make_url("/api/v1/nucs/create");
        let response: CreateNucResponse =
            self.client.post(url).json(&request).send().await?.error_for_status()?.json().await?;
        Ok(response.token)
    }

    async fn pay_subscription(
        &self,
        payments_client: &mut NillionChainClient,
        key: &PublicKey,
    ) -> Result<TxHash, PaySubscriptionError> {
        let about = self.about().await?;
        let cost = self.subscription_cost().await?;
        let payload = ValidatePaymentRequestPayload { nonce: rand::random(), service_public_key: about.public_key };
        let payload = serde_json::to_string(&payload)?;
        let hash = Sha256::digest(&payload);
        let tx_hash = payments_client
            .pay_for_resource(cost, hash.to_vec())
            .await
            .map_err(|e| PaySubscriptionError::Payment(e.to_string()))?;

        let public_key = key.to_sec1_bytes().as_ref().try_into().map_err(|_| PaySubscriptionError::InvalidPublicKey)?;
        let url = self.make_url("/api/v1/payments/validate");
        let request = ValidatePaymentRequest { tx_hash: tx_hash.clone(), payload: payload.into_bytes(), public_key };
        self.client.post(url).json(&request).send().await?.error_for_status()?;
        Ok(TxHash(tx_hash))
    }

    async fn subscription_cost(&self) -> Result<TokenAmount, SubscriptionCostError> {
        let url = self.make_url("/api/v1/payments/cost");
        let response: GetCostResponse = self.client.get(url).send().await?.json().await?;
        Ok(TokenAmount::Unil(response.cost_unils))
    }
}

/// A transaction hash.
#[derive(Clone, Debug, PartialEq)]
pub struct TxHash(pub String);

/// Information about a nilauth server.
#[derive(Clone, Deserialize)]
pub struct About {
    /// The server's public key.
    #[serde(deserialize_with = "hex::serde::deserialize")]
    pub public_key: [u8; 33],
}

#[derive(Serialize)]
struct CreateNucRequest {
    #[serde(serialize_with = "hex::serde::serialize")]
    public_key: [u8; 33],

    #[serde(serialize_with = "hex::serde::serialize")]
    signature: [u8; 64],

    #[serde(serialize_with = "hex::serde::serialize")]
    payload: Vec<u8>,
}

#[derive(Serialize)]
struct CreateNucRequestPayload {
    // A nonce, to add entropy.
    #[serde(serialize_with = "hex::serde::serialize")]
    nonce: [u8; 16],

    // When this payload is no longer considered valid, to prevent reusing this forever if it
    // leaks.
    #[serde(serialize_with = "chrono::serde::ts_seconds::serialize")]
    expires_at: DateTime<Utc>,

    // Our public key, to ensure this request can't be redirected to another authority service.
    #[serde(serialize_with = "hex::serde::serialize")]
    target_public_key: [u8; 33],
}

#[derive(Debug, Deserialize)]
struct CreateNucResponse {
    token: String,
}

#[derive(Serialize)]
struct ValidatePaymentRequest {
    tx_hash: String,

    #[serde(serialize_with = "hex::serde::serialize")]
    payload: Vec<u8>,

    #[serde(serialize_with = "hex::serde::serialize")]
    public_key: [u8; 33],
}

#[derive(Serialize)]
struct ValidatePaymentRequestPayload {
    #[allow(dead_code)]
    #[serde(serialize_with = "hex::serde::serialize")]
    nonce: [u8; 16],

    #[serde(serialize_with = "hex::serde::serialize")]
    service_public_key: [u8; 33],
}

#[derive(Debug, Deserialize)]
struct GetCostResponse {
    // The cost in unils.
    cost_unils: u64,
}
