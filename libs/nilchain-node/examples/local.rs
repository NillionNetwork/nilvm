use clap::Parser;
use nilchain_node::{
    node::{GenesisAccount, NillionChainNodeBuilder},
    transactions::TokenAmount,
};
use tempfile::tempdir;

#[derive(Parser)]
struct Cli {
    #[clap(short, long)]
    bind_address: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let home = tempdir().expect("failed to create temporary directory");
    let stash_key_name = "stash".to_string();
    let mut builder = NillionChainNodeBuilder::new(home.path())
        .genesis_accounts(vec![GenesisAccount {
            name: stash_key_name.clone(),
            amount: TokenAmount::Nil(1_000_000_000),
        }])
        .log(home.path().join("log.txt"));
    if let Some(address) = args.bind_address {
        builder = builder.bind_address(address);
    }
    let node = builder.build().expect("could not create nillion chain node");
    let stash_key = node.get_genesis_account_private_key(&stash_key_name).expect("failed to get stash key");
    println!("Node storing state in {}", node.home().display());
    println!("Stash key is: {}", stash_key.as_hex());
    println!("RPC endpiont: {}", node.rpc_endpoint());
    println!("gRPC endpiont: {}", node.grpc_endpoint());
    println!("REST endpiont: {}", node.rest_api_endpoint());
    println!("chain id: {}", node.chain_id());
    println!("Press <ctrl-c> to stop");

    let _ = tokio::signal::ctrl_c().await;
}
