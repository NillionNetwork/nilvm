use clap::Parser;
use std::{net::IpAddr, path::PathBuf};
use uuid::Uuid;

/// Run a test Nillion cluster locally.
#[derive(Parser)]
pub struct Cli {
    /// The number of nodes in the cluster.
    #[arg(short, long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(3..100))]
    pub node_count: u32,

    /// The uuid of the cluster.
    ///
    /// A random uuid will be used if none is provided, honoring the --seed parameter.
    #[arg(short, long)]
    pub cluster_id: Option<Uuid>,

    /// The directory where the node's states is stored.
    ///
    /// A temporary directory will be used if none is provided.
    #[arg(short = 'd', long)]
    pub state_directory: Option<PathBuf>,

    /// The seed to use for keys and cluster ids.
    #[arg(short = 's', long, default_value = "nillion-devnet")]
    pub seed: String,

    /// The number of bits in the prime number to be used.
    #[arg(short, long, default_value_t = 256)]
    pub prime_bits: usize,

    /// The address to bind to.
    #[arg(short, long, default_value = "127.0.0.1")]
    pub bind_address: IpAddr,

    /// Whether to export prometheus metrics.
    #[arg(long, hide = true)]
    pub enable_metrics: bool,

    /// The IP address to bind to when exporting metrics.
    #[arg(long, hide = true, default_value = "127.0.0.1")]
    pub metrics_bind_address: IpAddr,

    /// A path to a PEM encoded TLS certificate to be used for the gRPC server.
    #[arg(long, hide = true)]
    pub tls_certificate: Option<PathBuf>,

    /// A path to a PEM encoded certificate key to be used for the gRPC server.
    #[arg(long, hide = true)]
    pub tls_key: Option<PathBuf>,

    /// A path to a PEM encoded TLS certificate for the CA that signed the certificate.
    #[arg(long, hide = true)]
    pub tls_ca_certificate: Option<PathBuf>,

    /// Whether to enable tracing (disables env logger).
    #[arg(long, hide = true)]
    pub enable_tracing: bool,

    /// Disable program auditor
    #[arg(long, hide = true)]
    pub disable_program_auditor: bool,
}

#[cfg(test)]
mod test {
    use super::Cli;

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
