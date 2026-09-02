use clap::{Parser, Subcommand};
use scytale_consensus::INITIAL_REWARD;
use scytale_core::QUANTA_PER_SCY;
use scytale_node::{Node, NodeConfig};

#[derive(Parser, Debug)]
#[command(
    name = "scytale-node",
    author,
    version,
    about = "Scytale Blockchain Engine CLI & Node Daemon"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to data directory
    #[arg(short, long, default_value = ".scytale")]
    data_dir: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Scytale full node daemon
    Start {
        /// Enable autonomous Proof-of-Work mining
        #[arg(short, long, default_value_t = false)]
        mine: bool,
    },
    /// Inspect blockchain status
    Status,
}

#[allow(clippy::result_large_err)]
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Start { mine }) => {
            let config = NodeConfig {
                data_dir: cli.data_dir.clone().into(),
                mining_enabled: *mine,
                ..NodeConfig::default()
            };
            tracing::info!(
                data_dir = %config.data_dir.display(),
                mining = config.mining_enabled,
                "starting scytale node daemon"
            );
            tracing::info!(
                "protocol baseline: initial subsidy = {} quanta ({} SCY)",
                INITIAL_REWARD,
                INITIAL_REWARD / QUANTA_PER_SCY
            );

            let res = tokio::task::spawn_blocking(move || {
                let mut node = Node::open(config)?;
                node.start()?;
                Ok::<Node, scytale_node::NodeError>(node)
            })
            .await;

            match res {
                Ok(Ok(mut node)) => {
                    tracing::info!(
                        height = node.canonical_height(),
                        tip = ?node.canonical_tip(),
                        state = ?node.state(),
                        "node is running; awaiting shutdown signal (Ctrl+C)"
                    );
                    if let Err(e) = tokio::signal::ctrl_c().await {
                        tracing::error!("failed to listen for Ctrl+C: {e}");
                        return;
                    }
                    tracing::info!("shutdown signal received");
                    match tokio::task::spawn_blocking(move || node.shutdown()).await {
                        Ok(Ok(())) => tracing::info!("node shutdown completed cleanly"),
                        Ok(Err(e)) => tracing::error!("error during shutdown: {e}"),
                        Err(e) => tracing::error!("shutdown task failed to join: {e}"),
                    }
                }
                Ok(Err(e)) => tracing::error!("node failed to start: {e}"),
                Err(e) => tracing::error!("node start task failed to join: {e}"),
            }
        }
        Some(Commands::Status) => {
            println!(
                "Scytale Node Status: Operational (data dir: {})",
                cli.data_dir
            );
        }
        None => {
            println!("No subcommand specified. Run with `--help` for available commands.");
        }
    }
}
