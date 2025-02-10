use nodes_fixtures::nodes::nodes;
use std::thread::sleep;
use tracing_fixture::tracing;

#[tokio::main]
async fn main() {
    let tracing_handle = tracing();
    let _nodes = nodes(&tracing_handle);
    loop {
        sleep(std::time::Duration::from_secs(1000));
    }
}
