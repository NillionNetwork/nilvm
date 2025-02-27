use crate::key::{NillionChainAddress, NillionChainPrivateKey};
use anyhow::{anyhow, bail};
use cosmrs::{
    crypto::secp256k1::SigningKey,
    proto::cosmos::{auth, bank, base, tx},
    rpc::{
        endpoint::{abci_query, broadcast::tx_sync::Response},
        Client, HttpClient,
    },
    tendermint::{abci::Code, chain},
    tx::{Body, BodyBuilder, Fee, SignDoc, SignerInfo},
    Any, Coin, Denom,
};
use futures::future;
use nillion_chain_transactions::{PaymentTransactionMessage, TokenAmount};
use prost::Message;
use rand::Rng;
use std::{future::Future, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::time::sleep;
use tracing::{debug, info, warn};

// The default gas price in unils.
const DEFAULT_GAS_PRICE: f64 = 0.025;
const GAS_ADJUSTMENT_PERCENT: u8 = 3;
const MAX_RETRIES: u32 = 150;
const RETRY_INITIAL_DELAY: Duration = Duration::from_millis(100);
const RETRY_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(5);
const RETRY_MAX_JITTER: u64 = 100;
const RETRY_DELAY_EXPONENTIAL_BASE: f32 = 1.4; // 1.4^0 = 1, 1.4^1 = 1.4, 1.4^2 = 1.96, 1.4^3 = 2.74, 1.4^4 = 3.84

/// A client for the nillion chain.
pub struct NillionChainClient {
    client: HttpClient,
    signing_key: Arc<SigningKey>,
    pub address: String,
    chain_id: chain::Id,
    sequence_id: u64,
    account_number: u64,
    gas_price: f64,
}

#[derive(Error, Debug)]
pub enum NillionChainClientError {
    #[error("account not found: {0}")]
    AccountNotFound(String),
    #[error("failed to get network status: {0}")]
    NetworkStatus(String),
    #[error("http client error: {0}")]
    HttpClient(String),
}

impl NillionChainClient {
    /// Create a new NillionChainClient
    pub async fn new(
        rpc_endpoint: String,
        private_key: NillionChainPrivateKey,
    ) -> anyhow::Result<Self, NillionChainClientError> {
        let address = private_key.address;

        let http_client =
            HttpClient::new(rpc_endpoint.as_str()).map_err(|e| NillionChainClientError::HttpClient(e.to_string()))?;

        debug!("Fetching chain id against RPC endpoint {rpc_endpoint}");
        let client = http_client.clone();
        let chain_id = Self::invoke_with_retries("status", || async move {
            client.status().await.map(|status| status.node_info.network).map_err(Into::into)
        })
        .await
        .map_err(|e| NillionChainClientError::NetworkStatus(e.to_string()))?;

        debug!("Querying for address {address}");
        let account = Self::query_account_with_retry(http_client.clone(), address.0.clone())
            .await
            .map_err(|e| NillionChainClientError::AccountNotFound(e.to_string()))?;
        let sequence_id = account.sequence;
        let account_number = account.account_number;

        info!("Client created, sequence {sequence_id}, account {account_number}, address {address}");

        Ok(Self {
            client: http_client,
            signing_key: private_key.key,
            address: address.0,
            chain_id,
            sequence_id,
            account_number,
            gas_price: DEFAULT_GAS_PRICE,
        })
    }

    /// Set the gas price in unils.
    pub fn set_gas_price(&mut self, price: f64) {
        self.gas_price = price;
    }

    /// Make a payment to a given address and return transaction hash
    pub async fn pay(&mut self, to: &NillionChainAddress, amount: TokenAmount) -> anyhow::Result<String> {
        debug!("Paying {amount} to {to}");

        let msg = Any::from_msg(&bank::v1beta1::MsgSend {
            from_address: self.address.clone(),
            to_address: to.0.clone(),
            amount: vec![base::v1beta1::Coin {
                denom: TokenAmount::lowest_denomination().to_string(),
                amount: amount.to_unil().to_string(),
            }],
        })
        .map_err(|e| anyhow!("creating any: {e}"))?;
        let payload = BodyBuilder::new().msg(msg).finish();

        let tx = self.sign_and_broadcast(payload).await?;
        let tx_hash = tx.hash.to_string();
        self.poll_for_transaction(tx_hash.clone()).await?;
        debug!("Paid {amount} to {to} in tx {tx_hash}");

        Ok(tx_hash)
    }

    /// Make a payment to multiple addresses at once.
    pub async fn pay_many(&mut self, payments: Vec<(NillionChainAddress, TokenAmount)>) -> anyhow::Result<String> {
        let denom = TokenAmount::lowest_denomination().to_string();
        let total_payments = payments.len();
        let total_out = payments.iter().map(|(_, amount)| amount.to_unil()).sum::<u64>();
        let outputs = payments
            .into_iter()
            .map(|(address, amount)| bank::v1beta1::Output {
                address: address.0,
                coins: vec![base::v1beta1::Coin { denom: denom.clone(), amount: amount.to_unil().to_string() }],
            })
            .collect();
        let msg = Any::from_msg(&bank::v1beta1::MsgMultiSend {
            inputs: vec![bank::v1beta1::Input {
                address: self.address.clone(),
                coins: vec![base::v1beta1::Coin { denom: denom.clone(), amount: total_out.to_string() }],
            }],
            outputs,
        })
        .map_err(|e| anyhow!("creating any: {e}"))?;
        let payload = BodyBuilder::new().msg(msg).finish();

        let tx = self.sign_and_broadcast(payload).await?;
        let tx_hash = tx.hash.to_string();
        self.poll_for_transaction(tx_hash.clone()).await?;
        debug!("Paid {total_out}unil to {total_payments} addresses tx {tx_hash}");

        Ok(tx_hash)
    }

    /// Top up the balance of the accounts to a desired amount if falls below the threshold percent
    pub async fn top_up_balances(
        &mut self,
        addresses: Vec<NillionChainAddress>,
        minimum_balance: TokenAmount,
        balance_target: TokenAmount,
    ) -> anyhow::Result<Option<String>> {
        if balance_target < minimum_balance {
            bail!("Minimum must be lower than target balance");
        }
        // Get all balances at once.
        let mut futs = Vec::new();
        for address in &addresses {
            futs.push(self.get_balance(address));
        }
        let results = future::join_all(futs).await;

        // Now check which addresses need topping up.
        let mut payments = Vec::new();
        for (address, result) in addresses.into_iter().zip(results) {
            match result {
                Ok(current_balance) => {
                    if current_balance < minimum_balance {
                        let amount_to_pay =
                            TokenAmount::Unil(balance_target.clone().to_unil() - current_balance.clone().to_unil());
                        debug!(
                            "Balance {current_balance} is below threshold {minimum_balance} for {address}, topping up {amount_to_pay}"
                        );
                        payments.push((address, amount_to_pay));
                    } else {
                        debug!("Balance {current_balance} is above threshold {minimum_balance}, not topping up");
                    }
                }
                Err(e) => {
                    debug!(
                        "Unable to get balance ({:?}) for {address}, assuming new account, topping full amount {minimum_balance}",
                        e
                    );
                    payments.push((address, minimum_balance));
                }
            }
        }
        if payments.is_empty() {
            info!("No address needs funding");
            Ok(None)
        } else {
            let tx_hash = self.pay_many(payments).await?;
            Ok(Some(tx_hash))
        }
    }

    /// Get the balance of an address
    pub async fn get_balance(&self, address: &NillionChainAddress) -> anyhow::Result<TokenAmount> {
        let response = Self::abci_query_with_retry(
            self.client.clone(),
            Some("/cosmos.bank.v1beta1.Query/Balance".to_string()),
            bank::v1beta1::QueryBalanceRequest {
                address: address.to_string(),
                denom: TokenAmount::lowest_denomination().to_string(),
            }
            .encode_to_vec(),
        )
        .await?;
        let response_value = response.value;

        let balance_response = bank::v1beta1::QueryBalanceResponse::decode(&*response_value)
            .map_err(|e| anyhow!("decoding balance: {}", e))?;
        let balance = balance_response.balance.ok_or_else(|| anyhow!("no balance found"))?;
        if balance.denom != TokenAmount::lowest_denomination() {
            bail!("denomination mismatch");
        }
        let balance_amount: u64 = balance.amount.parse().map_err(|e| anyhow!("parsing balance: {}", e))?;

        Ok(TokenAmount::Unil(balance_amount))
    }

    /// Make a payment for a resource and return transaction hash
    pub async fn pay_for_resource(&mut self, amount: TokenAmount, resource: Vec<u8>) -> anyhow::Result<String> {
        let tx_msg = PaymentTransactionMessage::new(self.address.clone(), amount, resource).build()?;
        let body = BodyBuilder::new().msg(Any { type_url: tx_msg.type_url, value: tx_msg.value }).finish();

        debug!("Paying {amount} for resource");
        let tx_response = self.sign_and_broadcast(body).await?;
        let tx_hash = tx_response.hash.to_string().to_lowercase();
        self.poll_for_transaction(tx_hash.clone()).await?;
        debug!("Paid {amount} for resource in tx {tx_hash}");

        Ok(tx_hash.to_string())
    }

    async fn query_account_with_retry(
        client: HttpClient,
        address: String,
    ) -> anyhow::Result<auth::v1beta1::BaseAccount> {
        Self::invoke_with_retries("query_account", || async move {
            let response = Self::abci_query(
                client,
                Some("/cosmos.auth.v1beta1.Query/Account".to_string()),
                auth::v1beta1::QueryAccountRequest { address }.encode_to_vec(),
            )
            .await
            .map_err(Into::<anyhow::Error>::into)?;

            debug!("Queried for account");
            let account_response = auth::v1beta1::QueryAccountResponse::decode(response.value.as_slice())
                .map_err(|e| anyhow!("decoding query account response: {e}"))?;
            match account_response.account {
                Some(account) => auth::v1beta1::BaseAccount::decode(account.value.as_slice())
                    .map_err(|e| anyhow!("decoding base account: {e}")),
                None => Err(anyhow!("account does not exist on chain yet: {}", response.log)),
            }
        })
        .await
    }

    // Poll for transaction status
    async fn poll_for_transaction(&self, tx_hash: String) -> anyhow::Result<()> {
        debug!("Polling for transaction {tx_hash}");
        let client = self.client.clone();
        let hash = tx_hash.clone();
        Self::invoke_with_retries("get_tx", || async move {
            let response = client
                .abci_query(
                    Some("/cosmos.tx.v1beta1.Service/GetTx".to_string()),
                    tx::v1beta1::GetTxRequest { hash: hash.clone() }.encode_to_vec(),
                    None,
                    false,
                )
                .await;
            if let Ok(response) = response {
                let response = tx::v1beta1::GetTxResponse::decode(response.value.as_slice())?;
                if response.tx_response.is_some() {
                    debug!("Transaction {hash} has been committed");
                    Ok(())
                } else {
                    bail!("Transaction {hash} found but not yet committed");
                }
            } else {
                bail!("Transaction {hash} not found");
            }
        })
        .await
        .map_err(|e| anyhow!("timed out while waiting for {tx_hash}: {e}"))
    }

    async fn sign_and_broadcast(&mut self, tx: Body) -> anyhow::Result<Response> {
        for tried_resyncing in [false, true] {
            match self.sign(&tx).await {
                Ok(payload) => {
                    let client = self.client.clone();
                    let response = Self::invoke_with_retries("broadcast_tx_sync", || async move {
                        client.broadcast_tx_sync(payload).await.map_err(Into::into)
                    })
                    .await
                    .map_err(|e| anyhow!("broadcast tx sync: {e}"))?;

                    if matches!(response.code, Code::Err(_)) {
                        warn!("Failed to submit transaction: {}", response.log);
                        bail!("submitting transaction failed: {}", response.log);
                    }
                    debug!("Submitted tx successfully using sequence: {}", self.sequence_id);
                    self.sequence_id = self.sequence_id.wrapping_add(1);
                    return Ok(response);
                }
                Err(e) if e.to_string().contains("account sequence mismatch") && !tried_resyncing => {
                    warn!("Syncing sequence number: {e}");
                    let response = Self::query_account_with_retry(self.client.clone(), self.address.clone()).await?;
                    if response.sequence != self.sequence_id {
                        info!(
                            "Updating sequence number, ours was {}, network claims {}",
                            self.sequence_id, response.sequence
                        );
                        self.sequence_id = response.sequence;
                    } else {
                        warn!("Network claims our sequence number is correct, retrying");
                    }
                }
                Err(e) => return Err(e),
            };
        }
        // We can't get here given we only sync the sequence number on the first loop.
        bail!("unreachable")
    }

    async fn sign(&mut self, tx: &Body) -> anyhow::Result<Vec<u8>> {
        let sequence_id = self.sequence_id;
        debug!("Signing transaction using sequence {sequence_id}");
        let lowest_denom: Denom =
            TokenAmount::lowest_denomination().parse().map_err(|e| anyhow!("parsing denomination: {e}"))?;
        let gas_price = Coin { amount: 1, denom: lowest_denom.clone() };
        let auth_info = SignerInfo::single_direct(Some(self.signing_key.public_key()), sequence_id)
            .auth_info(Fee::from_amount_and_gas(gas_price.clone(), 100u64));

        let sign_doc = SignDoc::new(tx, &auth_info, &self.chain_id, self.account_number)
            .map_err(|e| anyhow!("signing doc: {e}"))?;
        let tx_raw = sign_doc.sign(&self.signing_key).map_err(|e| anyhow!("signing: {e}"))?;
        let simulated_tx = self.simulate(tx_raw.to_bytes().map_err(|e| anyhow!("tx raw bytes: {e}"))?).await?;

        let Some(gas_info) = simulated_tx.gas_info else {
            bail!("no gas info");
        };

        let tx_fee = Self::calculate_tx_fee(gas_info.gas_used, self.gas_price, GAS_ADJUSTMENT_PERCENT, lowest_denom);

        let auth_info = SignerInfo::single_direct(Some(self.signing_key.public_key()), sequence_id).auth_info(tx_fee);

        let sign_doc = SignDoc::new(tx, &auth_info, &self.chain_id, self.account_number)
            .map_err(|e| anyhow!("signing doc: {e}"))?;
        sign_doc
            .sign(&self.signing_key)
            .map_err(|e| anyhow!("signing: {e}"))?
            .to_bytes()
            .map_err(|e| anyhow!("signed tx to bytes: {e}"))
    }

    fn calculate_tx_fee(gas_needed: u64, gas_price_in_unil: f64, adjustment_percent: u8, lowest_denom: Denom) -> Fee {
        let gas_limit = gas_needed.saturating_mul((100 + adjustment_percent) as u64) / 100;
        let gas_price = (gas_limit as f64 * gas_price_in_unil).ceil() as u128;
        let gas_price = Coin { amount: gas_price, denom: lowest_denom.clone() };
        debug!(
            "Gas needed: {gas_needed}; Gas limit: {gas_limit} unil; Gas price (tx fee): {} {lowest_denom}",
            gas_price.amount
        );
        Fee::from_amount_and_gas(gas_price, gas_limit)
    }

    async fn simulate(&mut self, tx: Vec<u8>) -> anyhow::Result<tx::v1beta1::SimulateResponse> {
        let response = Self::abci_query_with_retry(
            self.client.clone(),
            Some("/cosmos.tx.v1beta1.Service/Simulate".to_string()),
            tx::v1beta1::SimulateRequest { tx_bytes: tx, ..Default::default() }.encode_to_vec(),
        )
        .await?;

        if response.code != Code::Ok {
            bail!("simulate failed: {}", response.log);
        }
        tx::v1beta1::SimulateResponse::decode(response.value.as_slice())
            .map_err(|e| anyhow!("decoding simulate response: {e}"))
    }

    pub async fn invoke_with_retries<F, Fut, T>(operation_name: &str, operation: F) -> Result<T, anyhow::Error>
    where
        F: FnOnce() -> Fut + Clone + Send + 'static,
        Fut: Future<Output = Result<T, anyhow::Error>> + Send + 'static,
    {
        let mut total_delay_ms = 0.0;

        for retry in 1..=MAX_RETRIES {
            let f = operation.clone();
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => warn!("Operation {operation_name} failed on attempt {retry}/{MAX_RETRIES}: {e}"),
            }

            let exponent = retry - 1;
            let delay =
                (RETRY_INITIAL_DELAY * RETRY_DELAY_EXPONENTIAL_BASE.powf(exponent as f32) as u32).min(MAX_RETRY_DELAY);
            let jitter_millis = rand::thread_rng().gen_range(0..RETRY_MAX_JITTER);
            let delay_with_jitter = delay + Duration::from_millis(jitter_millis);

            total_delay_ms += delay_with_jitter.as_millis() as f32;
            if total_delay_ms > RETRY_TIMEOUT.as_millis() as f32 {
                bail!("Operation {operation_name} failed after exceeding max retry timeout {RETRY_TIMEOUT:?}");
            }

            info!("Retrying {operation_name} in {delay_with_jitter:?}");
            sleep(delay_with_jitter).await;
        }
        bail!("Operation {operation_name} failed after {MAX_RETRIES} retries")
    }

    async fn abci_query_with_retry<V>(
        client: HttpClient,
        path: Option<String>,
        data: V,
    ) -> anyhow::Result<abci_query::AbciQuery>
    where
        V: Into<Vec<u8>> + Send + Clone + 'static,
    {
        Self::invoke_with_retries("abci_query", || async move {
            Self::abci_query(client, path, data).await.map_err(Into::into)
        })
        .await
        .map_err(|e| anyhow!("abci_query failed: {e}"))
    }

    async fn abci_query<V>(client: HttpClient, path: Option<String>, data: V) -> anyhow::Result<abci_query::AbciQuery>
    where
        V: Into<Vec<u8>> + Send + Clone + 'static,
    {
        client.abci_query(path, data, None, false).await.map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use crate::client::NillionChainClient;
    use nillion_chain_transactions::TokenAmount;
    use rstest::rstest;

    #[rstest]
    #[case::exact(4000, 0.025, 0, 100, 4000)]
    #[case::small_percentage_increase_rounding_up(4000, 0.025, 1, 101, 4040)]
    #[case::remainder_rounding_up(4001, 0.025, 0, 101, 4001)]
    #[case::small_percentage_increase_and_remainder_rounding_up(4001, 0.025, 1, 102, 4041)]
    #[case::large_exact(1_000_000, 0.025, 0, 25_000, 1_000_000)]
    #[case::large_with_small_percentage_increase_exact(1_000_000, 0.025, 1, 25250, 1_010_000)]
    #[case::tiny_fraction_rounding(4000, 0.000_000_1, 0, 1, 4000)]
    #[case::small_percentage_increase_and_tiny_fraction_remainder_rounding_up(4000, 0.000_000_1, 1, 1, 4040)]
    fn test_calculate_tx_fee(
        #[case] gas_needed: u64,
        #[case] gas_price_unil: f64,
        #[case] gas_percentage_adjustment: u8,
        #[case] expected_fee: u128,
        #[case] expected_gas_limit: u128,
    ) {
        let lowest_denom = TokenAmount::lowest_denomination().parse().expect("parsing denom");
        let tx_fee =
            NillionChainClient::calculate_tx_fee(gas_needed, gas_price_unil, gas_percentage_adjustment, lowest_denom);
        assert_eq!(tx_fee.amount.get(0).expect("amount").amount, expected_fee);
        assert_eq!(tx_fee.gas_limit as u128, expected_gas_limit);
    }
}
