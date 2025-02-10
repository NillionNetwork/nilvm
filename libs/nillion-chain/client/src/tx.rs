use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use bytes::Bytes;
use cosmrs::{
    proto::cosmos::tx::v1beta1::{GetTxRequest, GetTxResponse},
    rpc::{Client, HttpClient},
};
use nillion_chain_transactions::{PaymentTransactionMessage, TokenAmount};
use prost::Message;

/// Payment transaction
#[derive(Clone, Debug)]
pub struct PaymentTransaction {
    pub resource: Vec<u8>,
    pub from_address: String,
    pub amount: TokenAmount,
}

impl PaymentTransaction {
    /// Deserializes payment transaction from a Nillion Chain tx payload
    pub fn deserialize(bytes: Vec<u8>) -> Result<Self> {
        let msg = PaymentTransactionMessage::decode(Bytes::from(bytes))?;

        if msg.amounts.len() > 1 {
            return Err(anyhow!("only one payment amount is supported"));
        }

        let amount = if let Some(amount) = msg.amounts.first() {
            let amount_num = amount.amount.parse::<u64>().context("invalid payment amount")?;
            match amount.denom.to_lowercase().as_str() {
                "unil" => TokenAmount::Unil(amount_num),
                "nil" => TokenAmount::Nil(amount_num),
                _ => return Err(anyhow!("unsupported payment denom")),
            }
        } else {
            return Err(anyhow!("no payment amount found"));
        };

        let payment_tx = PaymentTransaction { resource: msg.resource, from_address: msg.from_address, amount };

        Ok(payment_tx)
    }
}

/// A payments transaction retriever.
#[async_trait]
pub trait PaymentTransactionRetriever: Send + Sync + 'static {
    /// Gets a transaction by hash.
    async fn get(&self, tx_hash: &str) -> Result<PaymentTransaction, RetrieveError>;
}

/// Nillion Chain Transaction retriever
pub struct DefaultPaymentTransactionRetriever {
    client: HttpClient,
}

/// Nillion Chain Transaction retriever implementation
impl DefaultPaymentTransactionRetriever {
    /// Creates a new instance of NillionChainTransactionValidator
    pub fn new(rpc_endpoint: &str) -> Result<Self> {
        let client = HttpClient::new(rpc_endpoint)?;
        Ok(Self { client })
    }

    async fn fetch_transaction(&self, tx_hash: &str) -> Result<Vec<u8>, RetrieveError> {
        let request = GetTxRequest { hash: tx_hash.to_string() };

        let query = self
            .client
            .abci_query(Some("/cosmos.tx.v1beta1.Service/GetTx".to_string()), request.encode_to_vec(), None, false)
            .await
            .map_err(|e| RetrieveError::TransactionFetch(e.to_string()))?;
        let response =
            GetTxResponse::decode(query.value.as_slice()).map_err(|e| RetrieveError::Malformed(e.to_string()))?;

        let tx_response = response.tx_response.ok_or(RetrieveError::NotCommitted)?;
        let height = tx_response.height;
        if height == 0 {
            return Err(RetrieveError::NotCommitted);
        }

        let body = response.tx.and_then(|tx| tx.body).ok_or(RetrieveError::NotCommitted)?;
        let msg_payload =
            body.messages.first().ok_or_else(|| RetrieveError::Malformed("no payment message".into()))?.value.clone();
        Ok(msg_payload)
    }
}

#[async_trait]
impl PaymentTransactionRetriever for DefaultPaymentTransactionRetriever {
    /// Retrieves payment transaction data from the Nillion Chain or error if not found or not in the block
    async fn get(&self, tx_hash: &str) -> Result<PaymentTransaction, RetrieveError> {
        let payload = self.fetch_transaction(tx_hash).await?;
        PaymentTransaction::deserialize(payload).map_err(|e| RetrieveError::Malformed(e.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RetrieveError {
    #[error("transaction is not committed yet")]
    NotCommitted,

    #[error("malformed response: {0}")]
    Malformed(String),

    #[error("transaction fetch: {0}")]
    TransactionFetch(String),
}
