use nillion_chain_client::{
    client::NillionChainClient,
    key::NillionChainPrivateKey,
    transactions::TokenAmount,
    tx::{DefaultPaymentTransactionRetriever, PaymentTransactionRetriever},
};
use nodes_fixtures::nodes::{nodes, Nodes};
use rstest::rstest;
use tracing_fixture::{tracing, Tracing};

#[rstest]
#[tokio::test]
async fn test_payment(nodes: &Nodes, _tracing: &Tracing) {
    // Create tx validator
    let tx_retriever = DefaultPaymentTransactionRetriever::new(&nodes.nillion_chain_rpc_endpoint())
        .expect("could not create tx retriever");

    // Create user account
    let new_account_pk = NillionChainPrivateKey::from_seed("payments-key").expect("could not get private key");
    let new_account = new_account_pk.address.clone();

    // Fund user account
    nodes.top_up_balances(vec![new_account.clone()], TokenAmount::Nil(10)).await;

    // Client for user account
    let mut user_client = NillionChainClient::new(nodes.nillion_chain_rpc_endpoint(), new_account_pk)
        .await
        .expect("could not create nillion chain client");

    let user_balance = user_client.get_balance(&new_account).await.expect("could not get balance");
    assert!(user_balance >= TokenAmount::Nil(10));

    let resource = "nonce:test";

    // Let's pay for resource
    let tx_hash =
        user_client.pay_for_resource(TokenAmount::Nil(1), resource.as_bytes().to_vec()).await.expect("could not pay");

    // Retrieve tx
    let tx = tx_retriever.get(tx_hash.as_str()).await.expect("could not retrieve tx");

    assert_eq!(tx.amount, TokenAmount::Nil(1));
    assert_eq!(tx.from_address, new_account.0);
    assert_eq!(tx.resource, resource.as_bytes());
}
