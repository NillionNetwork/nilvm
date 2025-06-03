//! Cyclic pool of clients.

use anyhow::{Context, Error, anyhow};
use log::info;
use nilchain_client::{
    client::NillionChainClient,
    key::{NillionChainAddress, NillionChainPrivateKey},
    transactions::TokenAmount,
};
use nillion_client::{
    SigningKey, async_trait,
    builder::VmClientBuilder,
    payments::{NilChainPayer, TxHash},
    vm::VmClient,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tracing::error;

const MAX_PAYMENT_RETRIES: usize = 5;

/// Cyclic pool of items.
pub struct ItemsPool<T: Clone + 'static> {
    items: Vec<T>,
    next_index: usize,
}

impl<T: Clone + 'static> ItemsPool<T> {
    /// Create a new pool.
    pub fn new(items: Vec<T>) -> Self {
        ItemsPool { items, next_index: 0 }
    }
}

impl ItemsPool<Clients> {
    pub(crate) async fn log_balances(&self) -> anyhow::Result<()> {
        for client in &self.items {
            let nilchain_client = client.payer.0.lock().await;
            let starting_balance = client.starting_balance.to_unil();
            let address = NillionChainAddress(nilchain_client.address.clone());

            let ending_balance = nilchain_client.get_balance(&address).await?.to_unil();
            let total_used = starting_balance.saturating_sub(ending_balance);
            info!(
                "Address {address} started with {starting_balance}unil, ended with {ending_balance}unil, total used {total_used}unil"
            );
        }
        Ok(())
    }
}

impl<T: Clone + 'static> Iterator for ItemsPool<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.items.is_empty() {
            return None;
        }
        let element = self.items[self.next_index % self.items.len()].clone();
        self.next_index += 1;
        Some(element)
    }
}

/// The clients used in a load test.
#[derive(Clone)]
pub struct Clients {
    /// The VM client.
    pub vm: VmClient,

    /// The nilchain payer.
    payer: NillionChainClientPayer,

    /// The starting balance for the nilchain address.
    pub starting_balance: TokenAmount,
}

/// Payment summary.
pub struct PaymentSummary {
    /// The duration of the quote request in nilvm network.
    pub quote_duration: Duration,

    /// The duration of the payment in nilchain blockchain.
    pub payment_duration: Duration,
}

/// Type alias for the NillionClients pool.
pub type ClientsPool = ItemsPool<Clients>;

/// Clients pool builder.
pub struct ClientsPoolBuilder {
    clients_count: u32,
    seeds: Option<Vec<String>>,
    stash_client: NillionChainClient,
    payments_rpc_endpoint: String,
    signing_key: SigningKey,
    required_starting_balance: TokenAmount,
    grpc: Grpc,
}

impl ClientsPoolBuilder {
    /// Create a new builder.
    pub fn new(
        clients_count: u32,
        stash_client: NillionChainClient,
        payments_rpc_endpoint: String,
        required_starting_balance: TokenAmount,
        grpc: Grpc,
        signing_key: SigningKey,
    ) -> Self {
        Self {
            clients_count,
            seeds: None,
            stash_client,
            payments_rpc_endpoint,
            signing_key,
            required_starting_balance,
            grpc,
        }
    }

    /// Set seeds.
    pub fn with_seeds(mut self, seeds: Vec<String>) -> Self {
        self.seeds = Some(seeds);
        self
    }

    fn build_payments_key(&self, client_index: usize) -> Result<NillionChainPrivateKey, Error> {
        let seed = match &self.seeds {
            Some(seeds) => seeds.get(client_index).context("too many clients")?.clone(),
            None => {
                let seed: [char; 32] = rand::random();
                seed.iter().collect()
            }
        };
        NillionChainPrivateKey::from_seed(&seed)
    }

    /// Build the clients pool.
    pub async fn build(mut self) -> Result<ClientsPool, Error> {
        if let Some(seeds) = &self.seeds {
            if seeds.len() != self.clients_count as usize {
                return Err(anyhow!("The number of seeds must be equal to the number of clients"));
            }
        }

        let mut addresses = Vec::new();
        let mut clients = Vec::with_capacity(self.clients_count as usize);
        let mut keys = Vec::new();
        for client_index in 0..self.clients_count as usize {
            let key = self.build_payments_key(client_index)?;
            addresses.push(key.address.clone());
            keys.push(key);
        }
        let balance_target = TokenAmount::Unil((self.required_starting_balance.to_unil() as f64 * 1.1) as u64);
        self.stash_client
            .top_up_balances(addresses, self.required_starting_balance, balance_target)
            .await
            .context("funding payments key")?;

        for payments_key in keys {
            let starting_balance = self.stash_client.get_balance(&payments_key.address).await?;
            let payments_client = NillionChainClient::new(self.payments_rpc_endpoint.clone(), payments_key)
                .await
                .context("creating payments client")?;
            let payments_client = Arc::new(Mutex::new(payments_client));
            let payer = NillionChainClientPayer(payments_client);
            let client = self.build_vm_client(payer.clone()).await;

            clients.push(Clients { vm: client, payer, starting_balance });
        }

        Ok(ClientsPool::new(clients))
    }

    async fn build_vm_client(&self, payer: NillionChainClientPayer) -> VmClient {
        let builder = match &self.grpc {
            Grpc::EnabledInsecure(url) => VmClientBuilder::default().bootnode_url(url),
            Grpc::EnabledSecure(url, root_cert) => VmClientBuilder::default()
                .bootnode_url(url)
                .ca_cert(root_cert.clone())
                .certificate_domain("nillion.local"),
        };
        let builder = builder.signing_key(self.signing_key.clone()).nilchain_payer(payer);
        builder.build().await.expect("failed to build nilvm client")
    }
}

/// The gRPC config
#[derive(Debug)]
pub enum Grpc {
    /// The gRPC client is enabled via an insecure channel.
    EnabledInsecure(String),

    /// The gRPC client is enabled via a secure channel using the given root CA.
    EnabledSecure(String, Vec<u8>),
}

#[derive(Clone)]
struct NillionChainClientPayer(Arc<tokio::sync::Mutex<NillionChainClient>>);

#[async_trait]
impl NilChainPayer for NillionChainClientPayer {
    async fn submit_payment(&self, amount_unil: u64, resource: Vec<u8>) -> Result<TxHash, Box<dyn std::error::Error>> {
        let mut client = self.0.lock().await;
        for _ in 0..MAX_PAYMENT_RETRIES {
            match client.pay_for_resource(TokenAmount::Unil(amount_unil), resource.clone()).await {
                Ok(tx_hash) => return Ok(TxHash(tx_hash)),
                Err(e) => {
                    error!("Failed to make payment: {e}");
                }
            };
        }
        Err(anyhow!("maximum retries reached").into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cyclic_pool() {
        let mut pool = ItemsPool::new(vec![1, 2, 3]);
        assert_eq!(pool.next(), Some(1));
        assert_eq!(pool.next(), Some(2));
        assert_eq!(pool.next(), Some(3));
        assert_eq!(pool.next(), Some(1));
        assert_eq!(pool.next(), Some(2));
        assert_eq!(pool.next(), Some(3));
    }

    #[test]
    fn test_cyclic_pool_2() {
        #[derive(Clone, Debug, PartialEq)]
        struct TestStruct {
            i: i32,
        }

        let mut pool = ItemsPool::new(vec![TestStruct { i: 1 }, TestStruct { i: 2 }, TestStruct { i: 3 }]);
        let item_1 = pool.next().unwrap();
        let item_2 = pool.next().unwrap();
        let item_3 = pool.next().unwrap();
        let item_4 = pool.next().unwrap();
        let item_5 = pool.next().unwrap();
        let item_6 = pool.next().unwrap();

        assert_eq!(item_1, item_4);
        assert_eq!(item_2, item_5);
        assert_eq!(item_3, item_6);
    }
}
