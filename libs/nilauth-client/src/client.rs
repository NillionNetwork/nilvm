use async_trait::async_trait;
use chrono::{DateTime, Utc};
use nillion_chain_client::{client::NillionChainClient, transactions::TokenAmount};
use nillion_nucs::{
    builder::{ExtendTokenError, NucTokenBuildError, NucTokenBuilder},
    envelope::{InvalidSignature, NucEnvelopeParseError, NucTokenEnvelope},
    k256::{
        ecdsa::{signature::Signer, Signature, SigningKey},
        sha2::{Digest, Sha256},
        PublicKey, SecretKey,
    },
    token::{Did, ProofHash, TokenBody},
};
use reqwest::Response;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;
use std::{fmt::Display, iter, time::Duration};
use tokio::time::sleep;
use tracing::warn;

const TOKEN_REQUEST_EXPIRATION: Duration = Duration::from_secs(60);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const TX_RETRY_ERROR_CODE: &str = "TRANSACTION_NOT_COMMITTED";
static PAYMENT_TX_RETRIES: &[Duration] = &[
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(3),
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(10),
    Duration::from_secs(10),
];

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

    /// Get our subscription status.
    async fn subscription_status(&self, key: &SecretKey) -> Result<Subscription, SubscriptionStatusError>;

    /// Get the cost of a subscription.
    async fn subscription_cost(&self) -> Result<TokenAmount, SubscriptionCostError>;

    /// Revoke a token.
    async fn revoke_token(&self, token: &NucTokenEnvelope, key: &SecretKey) -> Result<(), RevokeTokenError>;

    /// Lookup whether a token is revoked.
    async fn lookup_revoked_tokens(
        &self,
        envelope: &NucTokenEnvelope,
    ) -> Result<Vec<RevokedToken>, LookupRevokedTokensError>;
}

/// An error when requesting a token.
#[derive(Debug, thiserror::Error)]
pub enum RequestTokenError {
    #[error("fetching server's about: {0}")]
    About(#[from] AboutError),

    #[error("signing request: {0}")]
    Signing(#[from] SigningError),

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("request: {0:?}")]
    Request(RequestError),
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

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("making payment: {0}")]
    Payment(String),

    #[error("server could not validate payment: {tx_hash}")]
    PaymentValidation { tx_hash: TxHash, payload: String },

    #[error("request: {0:?}")]
    Request(RequestError),
}

/// An error when fetching the subscription status.
#[derive(Debug, thiserror::Error)]
pub enum SubscriptionStatusError {
    #[error("fetching server's about: {0}")]
    About(#[from] AboutError),

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("signing request: {0}")]
    Signing(#[from] SigningError),

    #[error("request: {0:?}")]
    Request(RequestError),
}

/// An error when fetching the subscription cost.
#[derive(Debug, thiserror::Error)]
pub enum SubscriptionCostError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("request: {0:?}")]
    Request(RequestError),
}

/// An error when revoking a token.
#[derive(Debug, thiserror::Error)]
pub enum RevokeTokenError {
    #[error("fetching server's about: {0}")]
    About(#[from] AboutError),

    #[error("requesting token: {0}")]
    RequestToken(#[from] RequestTokenError),

    #[error("malformed token returned from nilauth: {0}")]
    MalformedAuthToken(#[from] NucEnvelopeParseError),

    #[error("invalid signatures in token returned from nilauth: {0}")]
    InvalidAuthTokenSignatures(#[from] InvalidSignature),

    #[error("cannot extend token returned from nilauth: {0}")]
    AuthTokenNotDelegation(#[from] ExtendTokenError),

    #[error("building invocation: {0}")]
    BuildInvocation(#[from] NucTokenBuildError),

    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("request: {0:?}")]
    Request(RequestError),
}

/// An error when requesting the information about a nilauth instance.
#[derive(Debug, thiserror::Error)]
pub enum AboutError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("request: {0:?}")]
    Request(RequestError),
}

/// An error when looking up revoked tokens.
#[derive(Debug, thiserror::Error)]
pub enum LookupRevokedTokensError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("request: {0:?}")]
    Request(RequestError),
}

// implement `From<RequestError>` for a list of types.
macro_rules! impl_from_request_error {
    ($t:ty) => {
        impl From<RequestError> for $t {
            fn from(e: RequestError) -> Self {
                Self::Request(e)
            }
        }
    };
    ($t:ty, $($rest:ty),+) => {
        impl_from_request_error!($t);
        impl_from_request_error!($($rest),+);
    };
}

impl_from_request_error!(
    RequestTokenError,
    PaySubscriptionError,
    SubscriptionStatusError,
    SubscriptionCostError,
    RevokeTokenError,
    AboutError,
    LookupRevokedTokensError
);

/// The default nilauth client that hits the actual service.
pub struct DefaultNilauthClient {
    client: reqwest::Client,
    base_url: String,
}

impl DefaultNilauthClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, reqwest::Error> {
        let client = reqwest::Client::builder().timeout(REQUEST_TIMEOUT).build()?;
        Ok(Self { client, base_url: base_url.into() })
    }

    fn make_url(&self, path: &str) -> String {
        let base_url = &self.base_url;
        format!("{base_url}{path}")
    }

    async fn parse_reponse<T, E>(response: Response) -> Result<T, E>
    where
        T: DeserializeOwned,
        E: From<reqwest::Error> + From<RequestError>,
    {
        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let error: RequestError = response.json().await?;
            Err(error.into())
        }
    }

    async fn post<R, O, E>(&self, url: &str, request: &R) -> Result<O, E>
    where
        R: Serialize,
        O: DeserializeOwned,
        E: From<reqwest::Error> + From<RequestError>,
    {
        let response = self.client.post(url).json(&request).send().await?;
        Self::parse_reponse(response).await
    }

    async fn get<O, E>(&self, url: &str) -> Result<O, E>
    where
        O: DeserializeOwned,
        E: From<reqwest::Error> + From<RequestError>,
    {
        let response = self.client.get(url).send().await?;
        Self::parse_reponse(response).await
    }
}

#[async_trait]
impl NilauthClient for DefaultNilauthClient {
    async fn about(&self) -> Result<About, AboutError> {
        let url = self.make_url("/about");
        self.get(&url).await
    }

    async fn request_token(&self, key: &SecretKey) -> Result<String, RequestTokenError> {
        let about = self.about().await?;
        let payload = CreateNucRequestPayload {
            nonce: rand::random(),
            expires_at: Utc::now() + TOKEN_REQUEST_EXPIRATION,
            target_public_key: about.public_key,
        };
        let request = SignedRequest::new(key, &payload)?;
        let url = self.make_url("/api/v1/nucs/create");
        let response: Result<CreateNucResponse, RequestTokenError> = self.post(&url, &request).await;
        Ok(response?.token)
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
        let request =
            ValidatePaymentRequest { tx_hash: tx_hash.clone(), payload: payload.as_bytes().to_vec(), public_key };
        let tx_hash = TxHash(tx_hash);
        for delay in PAYMENT_TX_RETRIES {
            let response: Result<(), PaySubscriptionError> = self.post(&url, &request).await;
            match response {
                Ok(_) => return Ok(tx_hash),
                Err(PaySubscriptionError::Request(e)) if e.error_code == TX_RETRY_ERROR_CODE => {
                    warn!("Server could not validate payment, retyring in {delay:?}");
                    sleep(*delay).await;
                }
                Err(e) => return Err(e),
            };
        }
        Err(PaySubscriptionError::PaymentValidation { tx_hash, payload })
    }

    async fn subscription_status(&self, key: &SecretKey) -> Result<Subscription, SubscriptionStatusError> {
        let payload =
            SubscriptionStatusRequest { nonce: rand::random(), expires_at: Utc::now() + TOKEN_REQUEST_EXPIRATION };
        let request = SignedRequest::new(key, &payload)?;
        let url = self.make_url("/api/v1/subscriptions/status");
        self.post(&url, &request).await
    }

    async fn subscription_cost(&self) -> Result<TokenAmount, SubscriptionCostError> {
        let url = self.make_url("/api/v1/payments/cost");
        let response: Result<GetCostResponse, SubscriptionCostError> = self.get(&url).await;
        Ok(TokenAmount::Unil(response?.cost_unils))
    }

    async fn revoke_token(&self, token: &NucTokenEnvelope, key: &SecretKey) -> Result<(), RevokeTokenError> {
        let about = self.about().await?;
        let token = token.encode();
        let auth_token = self.request_token(key).await?;
        let auth_token = NucTokenEnvelope::decode(&auth_token)?.validate_signatures()?;
        // SAFETY: this can't not be an object
        let args = json!({"token": token}).as_object().cloned().expect("not an object");
        let invocation = NucTokenBuilder::extending(auth_token)?
            .audience(Did::new(about.public_key))
            .body(TokenBody::Invocation(args))
            .command(["nuc", "revoke"])
            .build(&key.into())?;
        let header_value = format!("Bearer {invocation}");
        let url = self.make_url("/api/v1/revocations/revoke");
        let response = self.client.post(url).header("Authorization", header_value).send().await?;
        Self::parse_reponse(response).await
    }

    async fn lookup_revoked_tokens(
        &self,
        envelope: &NucTokenEnvelope,
    ) -> Result<Vec<RevokedToken>, LookupRevokedTokensError> {
        let hashes = iter::once(envelope.token()).chain(envelope.proofs()).map(|t| t.compute_hash()).collect();
        let request = LookupRevokedTokensRequest { hashes };
        let url = self.make_url("/api/v1/revocations/lookup");
        let response: Result<LookupRevokedTokensResponse, LookupRevokedTokensError> = self.post(&url, &request).await;
        Ok(response?.revoked)
    }
}

/// A transaction hash.
#[derive(Clone, Debug, PartialEq)]
pub struct TxHash(pub String);

impl Display for TxHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Information about a nilauth server.
#[derive(Clone, Deserialize)]
pub struct About {
    /// The server's public key.
    #[serde(with = "hex::serde")]
    pub public_key: [u8; 33],
}

#[derive(Serialize)]
struct SignedRequest {
    #[serde(with = "hex::serde")]
    public_key: [u8; 33],

    #[serde(with = "hex::serde")]
    signature: [u8; 64],

    #[serde(with = "hex::serde")]
    payload: Vec<u8>,
}

impl SignedRequest {
    fn new<T>(key: &SecretKey, payload: &T) -> Result<Self, SigningError>
    where
        T: Serialize,
    {
        let payload = serde_json::to_string(&payload)?;
        let signature: Signature = SigningKey::from(key).sign(payload.as_bytes());

        let public_key =
            key.public_key().to_sec1_bytes().as_ref().try_into().map_err(|_| SigningError::InvalidPublicKey)?;
        let request = Self { public_key, signature: signature.to_bytes().into(), payload: payload.into_bytes() };
        Ok(request)
    }
}

/// An error when signing a request.
#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    #[error("payload serialization: {0}")]
    PayloadSerde(#[from] serde_json::Error),

    #[error("invalid public key")]
    InvalidPublicKey,
}

#[derive(Serialize)]
struct CreateNucRequestPayload {
    // A nonce, to add entropy.
    #[serde(with = "hex::serde")]
    nonce: [u8; 16],

    // When this payload is no longer considered valid, to prevent reusing this forever if it
    // leaks.
    #[serde(with = "chrono::serde::ts_seconds")]
    expires_at: DateTime<Utc>,

    // Our public key, to ensure this request can't be redirected to another authority service.
    #[serde(with = "hex::serde")]
    target_public_key: [u8; 33],
}

#[derive(Debug, Deserialize)]
struct CreateNucResponse {
    token: String,
}

#[derive(Serialize)]
struct ValidatePaymentRequest {
    tx_hash: String,

    #[serde(with = "hex::serde")]
    payload: Vec<u8>,

    #[serde(with = "hex::serde")]
    public_key: [u8; 33],
}

#[derive(Serialize)]
struct ValidatePaymentRequestPayload {
    #[allow(dead_code)]
    #[serde(with = "hex::serde")]
    nonce: [u8; 16],

    #[serde(with = "hex::serde")]
    service_public_key: [u8; 33],
}

#[derive(Debug, Deserialize)]
struct GetCostResponse {
    // The cost in unils.
    cost_unils: u64,
}

#[derive(Serialize)]
struct LookupRevokedTokensRequest {
    hashes: Vec<ProofHash>,
}

#[derive(Deserialize)]
struct LookupRevokedTokensResponse {
    revoked: Vec<RevokedToken>,
}

/// A revoked token.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct RevokedToken {
    /// The token hash.
    pub token_hash: ProofHash,

    /// The timestamp at which the token was revoked.
    pub revoked_at: DateTime<Utc>,
}

/// An error when performing a request.
#[derive(Clone, Debug, Deserialize)]
pub struct RequestError {
    /// The error message.
    pub message: String,

    /// The error code.
    pub error_code: String,
}

#[derive(Serialize)]
struct SubscriptionStatusRequest {
    #[serde(with = "hex::serde")]
    nonce: [u8; 16],

    #[serde(with = "chrono::serde::ts_seconds")]
    expires_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct Subscription {
    /// Whether the user is actively subscribed.
    pub subscribed: bool,

    /// The details about the subscription.
    pub details: Option<SubscriptionDetails>,
}

/// The subscription information.
#[derive(Deserialize)]
pub struct SubscriptionDetails {
    /// The timestamp at which the subscription expires.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub expires_at: DateTime<Utc>,
}
