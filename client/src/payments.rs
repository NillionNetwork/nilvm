//! Payments related types.

use nilchain_client::transactions::TokenAmount;
use std::{fmt, sync::Arc};
use tokio::sync::Mutex;
use tonic::async_trait;

pub use nilchain_client::{client::NillionChainClient, key::NillionChainPrivateKey};

/// A payer that uses the nilchain to submit payments.
#[async_trait]
pub trait NilChainPayer: Send + Sync + 'static {
    /// Submit a payment to the network and get back a transaction hash.
    async fn submit_payment(&self, amount_unil: u64, resource: Vec<u8>) -> Result<TxHash, Box<dyn std::error::Error>>;
}

/// A transaction hash.
#[derive(Clone, Debug, PartialEq)]
pub struct TxHash(pub String);

impl From<TxHash> for String {
    fn from(hash: TxHash) -> Self {
        hash.0
    }
}

impl fmt::Display for TxHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A client built on top of a [NillionChainClient].
#[derive(Clone)]
pub struct NillionChainClientPayer(Arc<Mutex<NillionChainClient>>);

impl NillionChainClientPayer {
    /// Construct a new payer.
    ///
    /// The payer will be internally protected via a mutex so it's safe to use this payer in
    /// multiple clients.
    pub fn new(client: NillionChainClient) -> Self {
        Self(Arc::new(Mutex::new(client)))
    }
}

#[async_trait]
impl NilChainPayer for NillionChainClientPayer {
    async fn submit_payment(&self, amount_unil: u64, resource: Vec<u8>) -> Result<TxHash, Box<dyn std::error::Error>> {
        let mut client = self.0.lock().await;
        let tx_hash = client.pay_for_resource(TokenAmount::Unil(amount_unil), resource).await?;
        Ok(TxHash(tx_hash))
    }
}
