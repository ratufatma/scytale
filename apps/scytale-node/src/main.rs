use clap::{Parser, Subcommand};
use scytale_consensus::INITIAL_REWARD;
use scytale_core::QUANTA_PER_SCY;
use scytale_node::{IpcServer, Node, NodeConfig, P2pSupervisor, DEFAULT_SOCKET_PATH};
use std::sync::Arc;

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

    /// Path to IPC Unix domain socket
    #[arg(long, default_value = DEFAULT_SOCKET_PATH)]
    socket: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Scytale full node daemon
    Start {
        /// Enable autonomous Proof-of-Work mining
        #[arg(short, long, default_value_t = false)]
        mine: bool,

        /// TCP bind address for Go P2P daemon (e.g. 127.0.0.1:9001)
        #[arg(long)]
        p2p_bind: Option<String>,

        /// Peer address(es) to dial for P2P network sync (can be repeated)
        #[arg(long = "peer", visible_alias = "seed-nodes", action = clap::ArgAction::Append)]
        peers: Vec<String>,

        /// Custom path to the scytale-p2p binary
        #[arg(long)]
        p2p_bin: Option<std::path::PathBuf>,

        /// Target difficulty for testnet/local testing (compact format, e.g. 0x207fffff)
        #[arg(long)]
        target: Option<String>,

        /// Custom miner payout locking script in hex (e.g. 010203 or 040506)
        #[arg(long)]
        miner_payout: Option<String>,

        /// Disable P2P subsystem completely (standalone mode)
        #[arg(long, default_value_t = false)]
        no_p2p: bool,

        /// HTTP REST API bind address (e.g. 127.0.0.1:8332 or 0.0.0.0:8332)
        #[arg(long, default_value = scytale_node::DEFAULT_HTTP_BIND)]
        http_bind: String,

        /// Disable HTTP gateway completely
        #[arg(long, default_value_t = false)]
        no_http: bool,

        /// Enable UTXO snapshot fast sync mode
        #[arg(long, default_value_t = false)]
        fast_sync: bool,

        /// DNS seed domain(s) to query for peer discovery (can be repeated)
        #[arg(long = "dns-seed", visible_alias = "seed", action = clap::ArgAction::Append)]
        dns_seeds: Vec<String>,

        /// Disable DNS seed resolution for P2P network discovery
        #[arg(long, default_value_t = false)]
        no_dns_seeds: bool,
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
        Some(Commands::Start {
            mine,
            p2p_bind,
            peers,
            p2p_bin,
            target,
            miner_payout,
            no_p2p,
            http_bind,
            no_http,
            fast_sync,
            dns_seeds,
            no_dns_seeds,
        }) => {
            let diff_target = target.as_deref().and_then(|t| {
                if let Some(hex) = t.strip_prefix("0x") {
                    u32::from_str_radix(hex, 16).ok()
                } else {
                    t.parse::<u32>().ok()
                }
            });

            let miner_payout_script = miner_payout
                .as_deref()
                .and_then(|s| scytale_primitives::from_hex(s).ok())
                .unwrap_or_else(|| vec![0x01, 0x02, 0x03]);

            let config = NodeConfig {
                data_dir: cli.data_dir.clone().into(),
                mining_enabled: *mine,
                miner_payout_script,
                genesis_difficulty_target: diff_target.unwrap_or(0x1d00_ffff),
                ..NodeConfig::default()
            };
            tracing::info!(
                data_dir = %config.data_dir.display(),
                mining = config.mining_enabled,
                socket = %cli.socket,
                p2p_bind = ?p2p_bind,
                peers = ?peers,
                http_bind = ?http_bind,
                http_enabled = !no_http,
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
                Ok(Ok(node)) => {
                    let node = Arc::new(node);
                    tracing::info!(
                        height = node.canonical_height(),
                        tip = ?node.canonical_tip(),
                        state = ?node.state(),
                        "node is running; awaiting shutdown signal (Ctrl+C or IPC)"
                    );

                    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
                    let ipc_server =
                        IpcServer::new(&cli.socket, Arc::clone(&node), shutdown_tx.clone());

                    let ipc_handle = tokio::spawn(async move {
                        if let Err(e) = ipc_server.run().await {
                            tracing::error!("IPC server error: {e}");
                        }
                    });

                    // Launch P2P Supervisor if not disabled
                    let p2p_handle = if !no_p2p && (p2p_bind.is_some() || !peers.is_empty() || !dns_seeds.is_empty()) {
                        let bridge_sock = node.config().data_dir.join("p2p_bridge.sock");
                        let mut p2p_supervisor = P2pSupervisor::new(
                            bridge_sock,
                            p2p_bind.clone(),
                            peers.clone(),
                            p2p_bin.clone(),
                            Arc::clone(&node),
                            shutdown_tx.clone(),
                        );
                        p2p_supervisor.set_fast_sync(*fast_sync);
                        p2p_supervisor.set_dns_seeds(dns_seeds.clone(), *no_dns_seeds);
                        Some(tokio::spawn(async move {
                            if let Err(e) = p2p_supervisor.run().await {
                                tracing::error!("P2P supervisor error: {e}");
                            }
                        }))
                    } else {
                        None
                    };

                    // Launch HTTP Gateway if not disabled
                    let http_handle = if !no_http {
                        let node_http = Arc::clone(&node);
                        let http_addr = http_bind.clone();
                        let rx = shutdown_tx.subscribe();
                        Some(tokio::spawn(async move {
                            if let Err(e) =
                                scytale_node::run_http_gateway(&http_addr, node_http, rx).await
                            {
                                tracing::error!("HTTP gateway error: {e}");
                            }
                        }))
                    } else {
                        None
                    };

                    tokio::select! {
                        _ = shutdown_rx.recv() => {
                            tracing::info!("IPC shutdown signal received");
                        }
                        ctrl_c_res = tokio::signal::ctrl_c() => {
                            if let Err(e) = ctrl_c_res {
                                tracing::error!("failed to listen for Ctrl+C: {e}");
                            } else {
                                tracing::info!("Ctrl+C signal received");
                            }
                            let _ = shutdown_tx.send(());
                        }
                    }

                    tracing::info!("initiating node shutdown sequence");
                    let node_clone = Arc::clone(&node);
                    match tokio::task::spawn_blocking(move || node_clone.shutdown()).await {
                        Ok(Ok(())) => tracing::info!("node shutdown completed cleanly"),
                        Ok(Err(e)) => tracing::error!("error during shutdown: {e}"),
                        Err(e) => tracing::error!("shutdown task failed to join: {e}"),
                    }
                    let _ = ipc_handle.await;
                    if let Some(h) = p2p_handle {
                        let _ = h.await;
                    }
                    if let Some(h) = http_handle {
                        let _ = h.await;
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
