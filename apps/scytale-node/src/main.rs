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
    #[arg(long, visible_alias = "ipc-path", default_value = DEFAULT_SOCKET_PATH)]
    socket: String,

    /// Enable autonomous Proof-of-Work mining
    #[arg(short, long, default_value_t = false)]
    mine: bool,

    /// Outbound Explorer URL for block indexer (e.g. http://127.0.0.1:8080)
    #[arg(long)]
    explorer_url: Option<String>,

    /// Bearer API key for indexer authentication
    #[arg(long)]
    indexer_key: Option<String>,

    /// TCP bind address for Go P2P daemon (e.g. 127.0.0.1:9001)
    #[arg(long)]
    p2p_bind: Option<String>,

    /// Peer address(es) to dial for P2P network sync (can be repeated)
    #[arg(long = "peer", visible_aliases = ["seeds", "seed", "peers", "seed-nodes"], action = clap::ArgAction::Append)]
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
    #[arg(long = "dns-seed", visible_aliases = ["dns-seeds", "dns_seed", "dns_seeds"], action = clap::ArgAction::Append)]
    dns_seeds: Vec<String>,

    /// Disable DNS seed resolution for P2P network discovery
    #[arg(long, default_value_t = false)]
    no_dns_seeds: bool,

    /// Maximum allowed reorganization depth before rejecting a competing branch
    #[arg(long, default_value_t = scytale_consensus::DEFAULT_MAX_REORG_DEPTH)]
    max_reorg_depth: u64,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the Scytale full node daemon
    Start {
        /// Path to data directory
        #[arg(short, long)]
        data_dir: Option<String>,

        /// Path to IPC Unix domain socket
        #[arg(long, visible_alias = "ipc-path")]
        socket: Option<String>,

        /// Enable autonomous Proof-of-Work mining
        #[arg(short, long, default_value_t = false)]
        mine: bool,

        /// Outbound Explorer URL for block indexer (e.g. http://127.0.0.1:8080)
        #[arg(long)]
        explorer_url: Option<String>,

        /// Bearer API key for indexer authentication
        #[arg(long)]
        indexer_key: Option<String>,

        /// TCP bind address for Go P2P daemon (e.g. 127.0.0.1:9001)
        #[arg(long)]
        p2p_bind: Option<String>,

        /// Peer address(es) to dial for P2P network sync (can be repeated)
        #[arg(long = "peer", visible_aliases = ["seeds", "seed", "peers", "seed-nodes"], action = clap::ArgAction::Append)]
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
        #[arg(long = "dns-seed", visible_aliases = ["dns-seeds", "dns_seed", "dns_seeds"], action = clap::ArgAction::Append)]
        dns_seeds: Vec<String>,

        /// Disable DNS seed resolution for P2P network discovery
        #[arg(long, default_value_t = false)]
        no_dns_seeds: bool,

        /// Maximum allowed reorganization depth before rejecting a competing branch
        #[arg(long, default_value_t = scytale_consensus::DEFAULT_MAX_REORG_DEPTH)]
        max_reorg_depth: u64,
    },
    /// Inspect blockchain status
    Status,
}

struct StartOptions {
    mine: bool,
    explorer_url: Option<String>,
    indexer_key: Option<String>,
    p2p_bind: Option<String>,
    peers: Vec<String>,
    p2p_bin: Option<std::path::PathBuf>,
    target: Option<String>,
    miner_payout: Option<String>,
    no_p2p: bool,
    http_bind: String,
    no_http: bool,
    fast_sync: bool,
    dns_seeds: Vec<String>,
    no_dns_seeds: bool,
    max_reorg_depth: u64,
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

    let (data_dir, socket, start_opts) = match &cli.command {
        Some(Commands::Start {
            data_dir,
            socket,
            mine,
            explorer_url,
            indexer_key,
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
            max_reorg_depth,
        }) => {
            let final_data_dir = data_dir.clone().unwrap_or_else(|| cli.data_dir.clone());
            let final_socket = socket.clone().unwrap_or_else(|| cli.socket.clone());
            let mut all_peers = cli.peers.clone();
            all_peers.extend(peers.clone());
            let mut all_dns = cli.dns_seeds.clone();
            all_dns.extend(dns_seeds.clone());
            (
                final_data_dir.clone(),
                final_socket.clone(),
                Some(StartOptions {
                    mine: *mine || cli.mine,
                    explorer_url: explorer_url.clone().or_else(|| cli.explorer_url.clone()),
                    indexer_key: indexer_key.clone().or_else(|| cli.indexer_key.clone()),
                    p2p_bind: p2p_bind.clone().or_else(|| cli.p2p_bind.clone()),
                    peers: all_peers,
                    p2p_bin: p2p_bin.clone().or_else(|| cli.p2p_bin.clone()),
                    target: target.clone().or_else(|| cli.target.clone()),
                    miner_payout: miner_payout.clone().or_else(|| cli.miner_payout.clone()),
                    no_p2p: *no_p2p || cli.no_p2p,
                    http_bind: http_bind.clone(),
                    no_http: *no_http || cli.no_http,
                    fast_sync: *fast_sync || cli.fast_sync,
                    dns_seeds: all_dns,
                    no_dns_seeds: *no_dns_seeds || cli.no_dns_seeds,
                    max_reorg_depth: *max_reorg_depth,
                }),
            )
        }
        Some(Commands::Status) => {
            println!(
                "Scytale Node Status: Operational (data dir: {})",
                cli.data_dir
            );
            return;
        }
        None => (
            cli.data_dir.clone(),
            cli.socket.clone(),
            Some(StartOptions {
                mine: cli.mine,
                explorer_url: cli.explorer_url.clone(),
                indexer_key: cli.indexer_key.clone(),
                p2p_bind: cli.p2p_bind.clone(),
                peers: cli.peers.clone(),
                p2p_bin: cli.p2p_bin.clone(),
                target: cli.target.clone(),
                miner_payout: cli.miner_payout.clone(),
                no_p2p: cli.no_p2p,
                http_bind: cli.http_bind.clone(),
                no_http: cli.no_http,
                fast_sync: cli.fast_sync,
                dns_seeds: cli.dns_seeds.clone(),
                no_dns_seeds: cli.no_dns_seeds,
                max_reorg_depth: cli.max_reorg_depth,
            }),
        ),
    };

    if let Some(opts) = start_opts {
        let diff_target = opts.target.as_deref().and_then(|t| {
            if let Some(hex) = t.strip_prefix("0x") {
                u32::from_str_radix(hex, 16).ok()
            } else {
                t.parse::<u32>().ok()
            }
        });

        let miner_payout_script = opts
            .miner_payout
            .as_deref()
            .and_then(|s| scytale_primitives::from_hex(s).ok())
            .unwrap_or_else(|| vec![0x01, 0x02, 0x03]);

        let config = NodeConfig {
            data_dir: data_dir.into(),
            mining_enabled: opts.mine,
            miner_payout_script,
            genesis_difficulty_target: diff_target.unwrap_or(0x1d00_ffff),
            explorer_url: opts.explorer_url.clone(),
            indexer_key: opts.indexer_key.clone(),
            max_reorg_depth: opts.max_reorg_depth,
            ..NodeConfig::default()
        };
        tracing::info!(
            data_dir = %config.data_dir.display(),
            mining = config.mining_enabled,
            socket = %socket,
            p2p_bind = ?opts.p2p_bind,
            peers = ?opts.peers,
            http_bind = %opts.http_bind,
            http_enabled = !opts.no_http,
            explorer_url = ?opts.explorer_url,
            max_reorg_depth = config.max_reorg_depth,
            "starting scytale node daemon"
        );
        tracing::info!(
            "protocol baseline: initial subsidy = {} quanta ({} SCY)",
            INITIAL_REWARD,
            INITIAL_REWARD / QUANTA_PER_SCY
        );

        // If explorer-url is present, initialize indexer and pass handle down into node state
        let indexer_handle = opts.explorer_url.as_ref().map(|url| {
            scytale_node::indexer::start_indexer(url.clone(), opts.indexer_key.clone())
        });

        let res = tokio::task::spawn_blocking(move || {
            let mut node = Node::open(config)?;
            if let Some(indexer) = indexer_handle {
                node.set_indexer(indexer);
            }
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
                    IpcServer::new(&socket, Arc::clone(&node), shutdown_tx.clone());

                let ipc_handle = tokio::spawn(async move {
                    if let Err(e) = ipc_server.run().await {
                        tracing::error!("IPC server error: {e}");
                    }
                });

                // Launch P2P Supervisor if not disabled
                let p2p_handle = if !opts.no_p2p && (opts.p2p_bind.is_some() || !opts.peers.is_empty() || !opts.dns_seeds.is_empty()) {
                    let bridge_sock = node.config().data_dir.join("p2p_bridge.sock");
                    let mut p2p_supervisor = P2pSupervisor::new(
                        bridge_sock,
                        opts.p2p_bind.clone(),
                        opts.peers.clone(),
                        opts.p2p_bin.clone(),
                        Arc::clone(&node),
                        shutdown_tx.clone(),
                    );
                    p2p_supervisor.set_fast_sync(opts.fast_sync);
                    p2p_supervisor.set_dns_seeds(opts.dns_seeds.clone(), opts.no_dns_seeds);
                    Some(tokio::spawn(async move {
                        if let Err(e) = p2p_supervisor.run().await {
                            tracing::error!("P2P supervisor error: {e}");
                        }
                    }))
                } else {
                    None
                };

                // Launch HTTP Gateway if not disabled
                let http_handle = if !opts.no_http {
                    let node_http = Arc::clone(&node);
                    let http_addr = opts.http_bind.clone();
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
}
