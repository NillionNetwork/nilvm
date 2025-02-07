use rstest::fixture;
use std::io;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub struct Tracing;

#[fixture]
#[once]
pub fn tracing() -> Tracing {
    let _ = std::env::var("RUST_LOG").map_err(|_| {
        std::env::set_var("RUST_LOG", "client_fixture=debug,node=debug,nillion_client=debug,state_machine=debug,functional=debug,nillion_chain_client=debug,nillion_chain_node=debug")
    });

    let fmt_layer = tracing_subscriber::fmt::layer().with_writer(io::stderr);
    let filter_layer = EnvFilter::from_default_env();

    tracing_subscriber::registry().with(filter_layer).with(fmt_layer).init();

    Tracing
}
