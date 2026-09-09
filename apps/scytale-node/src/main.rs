use clap::{Parser, Subcommand};
use scytale_bridge::NetworkEvent;
use scytale_consensus::INITIAL_REWARD;
use scytale_core::QUANTA_PER_SCY;
use scytale_core::{Block, CanonicalDeserialize, CanonicalSerialize, Transaction};
use scytale_node::{IpcServer, Node, NodeConfig, DEFAULT_SOCKET_PATH};
use scytale_node::{
    P2pInbound, P2pOutbound, SyncRequest, SyncResponse, SyncState, MAX_BLOCK_PAYLOAD,
    MAX_TRANSACTION_PAYLOAD,
};
use std::sync::Arc;

const IBD_BATCH_SIZE: u32 = 100;

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
    #[arg(
        short,
        long,
        default_value = "./data",
        help = "Node data directory (redb, identity, and known peers)"
    )]
    data_dir: String,
    #[arg(long, visible_alias = "ipc-path", default_value = DEFAULT_SOCKET_PATH)]
    socket: String,
    #[arg(short, long, default_value_t = false)]
    mine: bool,
    #[arg(long)]
    explorer_url: Option<String>,
    #[arg(long)]
    indexer_key: Option<String>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    miner_payout: Option<String>,
    #[arg(long, default_value_t = false)]
    no_p2p: bool,
    #[arg(
        long,
        default_value_t = 9000,
        help = "TCP port for the libp2p listener"
    )]
    p2p_port: u16,
    #[arg(long, help = "Override the libp2p listen multiaddr")]
    p2p_listen: Option<String>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Initial libp2p bootnode multiaddr (repeat or comma-separate)"
    )]
    bootnodes: Vec<String>,
    #[arg(long, default_value = scytale_node::DEFAULT_HTTP_BIND)]
    http_bind: String,
    #[arg(long, default_value_t = false)]
    no_http: bool,
    #[arg(long, default_value_t = scytale_consensus::DEFAULT_MAX_REORG_DEPTH)]
    max_reorg_depth: u64,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(visible_alias = "run")]
    Start {
        #[arg(
            short,
            long,
            default_value = "./data",
            help = "Node data directory (redb, identity, and known peers)"
        )]
        data_dir: String,
        #[arg(long, visible_alias = "ipc-path")]
        socket: Option<String>,
        #[arg(short, long, default_value_t = false)]
        mine: bool,
        #[arg(long)]
        explorer_url: Option<String>,
        #[arg(long)]
        indexer_key: Option<String>,
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        miner_payout: Option<String>,
        #[arg(long, default_value_t = false)]
        no_p2p: bool,
        #[arg(
            long,
            default_value_t = 9000,
            help = "TCP port for the libp2p listener"
        )]
        p2p_port: u16,
        #[arg(long, help = "Override the libp2p listen multiaddr")]
        p2p_listen: Option<String>,
        #[arg(
            long,
            value_delimiter = ',',
            help = "Initial libp2p bootnode multiaddr (repeat or comma-separate)"
        )]
        bootnodes: Vec<String>,
        #[arg(long, default_value = scytale_node::DEFAULT_HTTP_BIND)]
        http_bind: String,
        #[arg(long, default_value_t = false)]
        no_http: bool,
        #[arg(long, default_value_t = scytale_consensus::DEFAULT_MAX_REORG_DEPTH)]
        max_reorg_depth: u64,
    },
    Status,
}

struct StartOptions {
    mine: bool,
    explorer_url: Option<String>,
    indexer_key: Option<String>,
    target: Option<String>,
    miner_payout: Option<String>,
    no_p2p: bool,
    p2p_port: u16,
    p2p_listen: Option<String>,
    bootnodes: Vec<String>,
    http_bind: String,
    no_http: bool,
    max_reorg_depth: u64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let (data_dir, socket, opts) = match cli.command {
        Some(Commands::Start {
            data_dir,
            socket,
            mine,
            explorer_url,
            indexer_key,
            target,
            miner_payout,
            no_p2p,
            p2p_port,
            p2p_listen,
            bootnodes,
            http_bind,
            no_http,
            max_reorg_depth,
        }) => (
            data_dir,
            socket.unwrap_or(cli.socket.clone()),
            StartOptions {
                mine: mine || cli.mine,
                explorer_url: explorer_url.or(cli.explorer_url.clone()),
                indexer_key: indexer_key.or(cli.indexer_key.clone()),
                target: target.or(cli.target.clone()),
                miner_payout: miner_payout.or(cli.miner_payout.clone()),
                no_p2p: no_p2p || cli.no_p2p,
                p2p_port,
                p2p_listen: p2p_listen.or(cli.p2p_listen.clone()),
                bootnodes,
                http_bind,
                no_http: no_http || cli.no_http,
                max_reorg_depth,
            },
        ),
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
            StartOptions {
                mine: cli.mine,
                explorer_url: cli.explorer_url.clone(),
                indexer_key: cli.indexer_key.clone(),
                target: cli.target.clone(),
                miner_payout: cli.miner_payout.clone(),
                no_p2p: cli.no_p2p,
                p2p_port: cli.p2p_port,
                p2p_listen: cli.p2p_listen.clone(),
                bootnodes: cli.bootnodes.clone(),
                http_bind: cli.http_bind.clone(),
                no_http: cli.no_http,
                max_reorg_depth: cli.max_reorg_depth,
            },
        ),
    };

    let diff_target = opts.target.as_deref().and_then(|target| {
        target
            .strip_prefix("0x")
            .map(|hex| u32::from_str_radix(hex, 16).ok())
            .unwrap_or_else(|| target.parse::<u32>().ok())
    });
    let miner_payout_script = opts
        .miner_payout
        .as_deref()
        .and_then(|script| scytale_primitives::from_hex(script).ok())
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
        p2p = !opts.no_p2p,
        p2p_port = opts.p2p_port,
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

    let p2p_data_dir = config.data_dir.clone();
    let indexer_handle = opts
        .explorer_url
        .as_ref()
        .map(|url| scytale_node::indexer::start_indexer(url.clone(), opts.indexer_key.clone()));
    let result = tokio::task::spawn_blocking(move || {
        let mut node = Node::open(config)?;
        if let Some(indexer) = indexer_handle {
            node.set_indexer(indexer);
        }
        node.start()?;
        Ok::<Node, scytale_node::NodeError>(node)
    })
    .await;

    match result {
        Ok(Ok(node)) => {
            let node = Arc::new(node);
            tracing::info!(
                height = node.canonical_height(),
                tip = ?node.canonical_tip(),
                state = ?node.state(),
                "node is running; awaiting shutdown signal (Ctrl+C or IPC)"
            );
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
            let ipc_server = IpcServer::new(&socket, Arc::clone(&node), shutdown_tx.clone());
            let ipc_handle = tokio::spawn(async move {
                if let Err(error) = ipc_server.run().await {
                    tracing::error!("IPC server error: {error}");
                }
            });
            let mut p2p_inbound_handle = None;
            let mut p2p_outbound_handle = None;
            let mut p2p_swarm_handle = None;
            let mut p2p_shutdown = None;
            let sync_state = Arc::new(std::sync::Mutex::new(SyncState::Synced));
            if !opts.no_p2p {
                let mut p2p_config = scytale_node::P2pConfig::for_data_dir(&p2p_data_dir);
                p2p_config.listen_addr = opts
                    .p2p_listen
                    .clone()
                    .unwrap_or_else(|| format!("/ip4/0.0.0.0/tcp/{}", opts.p2p_port));
                p2p_config.bootnodes = opts.bootnodes.clone();
                p2p_config.best_height = node.canonical_height();
                match scytale_node::network::p2p::start_with_config(p2p_config).await {
                    Ok(handle) => {
                        tracing::info!(peer_id = %handle.local_peer_id, "libp2p networking started");
                        let swarm_task = handle.task;
                        p2p_shutdown = Some(handle.shutdown);
                        let mut inbound = handle.inbound;
                        let inbound_node = Arc::clone(&node);
                        let inbound_outbound = handle.outbound.clone();
                        let sync_responses = handle.sync_responses.clone();
                        let inbound_sync_state = Arc::clone(&sync_state);
                        let p2p_status = Arc::clone(&handle.status);
                        let mining_enabled = node.config().mining_enabled;
                        p2p_inbound_handle = Some(tokio::spawn(async move {
                            while let Some(message) = inbound.recv().await {
                                match message {
                                    P2pInbound::PeerStatus { peer, best_height } => {
                                        let local_height = inbound_node.canonical_height();
                                        let should_start = {
                                            let mut state = inbound_sync_state.lock().unwrap();
                                            state.observe_peer(local_height, best_height)
                                        };
                                        p2p_status.write().unwrap().sync_state =
                                            inbound_sync_state.lock().unwrap().clone();
                                        if should_start {
                                            tracing::info!(
                                                peer = %peer,
                                                local_height,
                                                target_height = best_height,
                                                "starting initial block download"
                                            );
                                            let node_for_pause = Arc::clone(&inbound_node);
                                            let _ = tokio::task::spawn_blocking(move || {
                                                node_for_pause.stop_mining()
                                            })
                                            .await;
                                            let _ = inbound_outbound
                                                .send(P2pOutbound::SyncRequest {
                                                    peer,
                                                    request: SyncRequest::GetHeaders {
                                                        start_height: local_height
                                                            .saturating_add(1),
                                                        limit: IBD_BATCH_SIZE,
                                                    },
                                                })
                                                .await;
                                        }
                                    }
                                    P2pInbound::Block(bytes) => {
                                        if !inbound_sync_state.lock().unwrap().is_syncing() {
                                            if bytes.len() <= MAX_BLOCK_PAYLOAD {
                                                if let Ok(block) =
                                                    Block::from_canonical_bytes(&bytes)
                                                {
                                                    let _ =
                                                        inbound_node.submit_external_block(block);
                                                }
                                            }
                                        }
                                    }
                                    P2pInbound::Transaction(bytes) => {
                                        if !inbound_sync_state.lock().unwrap().is_syncing() {
                                            if bytes.len() <= MAX_TRANSACTION_PAYLOAD {
                                                if let Ok(transaction) =
                                                    Transaction::from_canonical_bytes(&bytes)
                                                {
                                                    let _ = inbound_node
                                                        .submit_network_transaction(transaction);
                                                }
                                            }
                                        }
                                    }
                                    P2pInbound::SyncRequest {
                                        peer: _,
                                        request,
                                        response,
                                    } => {
                                        let sync_response = match request {
                                            SyncRequest::GetHeaders {
                                                start_height,
                                                limit,
                                            } => {
                                                let hashes = inbound_node
                                                    .get_canonical_hashes()
                                                    .unwrap_or_default()
                                                    .into_iter()
                                                    .skip(start_height as usize)
                                                    .take(limit.min(IBD_BATCH_SIZE) as usize)
                                                    .map(|hash| *hash.as_bytes())
                                                    .collect();
                                                SyncResponse::Headers { hashes }
                                            }
                                            SyncRequest::GetBlocks { hashes } => {
                                                let blocks = hashes
                                                    .into_iter()
                                                    .take(100)
                                                    .filter_map(|bytes| {
                                                        inbound_node
                                                            .get_block(&scytale_core::Hash256::new(
                                                                bytes,
                                                            ))
                                                            .ok()
                                                            .flatten()
                                                    })
                                                    .filter_map(|block| {
                                                        block.to_canonical_bytes().ok()
                                                    })
                                                    .collect();
                                                SyncResponse::Blocks { blocks }
                                            }
                                        };
                                        if let Ok(payload) = bincode::serialize(&sync_response) {
                                            let _ = sync_responses.send((response, payload)).await;
                                        }
                                    }
                                    P2pInbound::SyncResponse { peer, response } => match response {
                                        SyncResponse::Headers { hashes } => {
                                            if !hashes.is_empty()
                                                && inbound_sync_state.lock().unwrap().is_syncing()
                                            {
                                                let _ = inbound_outbound
                                                    .send(P2pOutbound::SyncRequest {
                                                        peer,
                                                        request: SyncRequest::GetBlocks { hashes },
                                                    })
                                                    .await;
                                            }
                                        }
                                        SyncResponse::Blocks { blocks } => {
                                            let mut accepted = 0u64;
                                            for bytes in blocks {
                                                if bytes.len() <= MAX_BLOCK_PAYLOAD {
                                                    if let Ok(block) =
                                                        Block::from_canonical_bytes(&bytes)
                                                    {
                                                        let expected_parent =
                                                            inbound_node.canonical_tip();
                                                        if block.header.previous_block_hash
                                                            != expected_parent
                                                        {
                                                            tracing::warn!(
                                                                expected = %expected_parent,
                                                                actual = %block.header.previous_block_hash,
                                                                "rejecting non-contiguous IBD block batch"
                                                            );
                                                            break;
                                                        }
                                                        match inbound_node
                                                            .submit_external_block(block)
                                                        {
                                                            Ok(_) => accepted += 1,
                                                            Err(error) => {
                                                                tracing::warn!(%error, "IBD block validation failed");
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            let current_height = inbound_node.canonical_height();
                                            let completed = inbound_sync_state
                                                .lock()
                                                .unwrap()
                                                .advance(current_height);
                                            p2p_status.write().unwrap().sync_state =
                                                inbound_sync_state.lock().unwrap().clone();
                                            if completed {
                                                tracing::info!(
                                                    current_height,
                                                    "initial block download complete"
                                                );
                                                if mining_enabled {
                                                    let node_for_resume = Arc::clone(&inbound_node);
                                                    let _ =
                                                        tokio::task::spawn_blocking(move || {
                                                            node_for_resume.start_mining()
                                                        })
                                                        .await;
                                                }
                                            } else if accepted > 0 {
                                                let _ = inbound_outbound
                                                    .send(P2pOutbound::SyncRequest {
                                                        peer,
                                                        request: SyncRequest::GetHeaders {
                                                            start_height: current_height
                                                                .saturating_add(1),
                                                            limit: IBD_BATCH_SIZE,
                                                        },
                                                    })
                                                    .await;
                                            }
                                        }
                                    },
                                }
                            }
                        }));
                        let outbound = handle.outbound;
                        let mut events = node.subscribe_p2p_events();
                        p2p_outbound_handle = Some(tokio::spawn(async move {
                            while let Ok(event) = events.recv().await {
                                let message = match event {
                                    NetworkEvent::BroadcastBlock { block_hex, .. } => {
                                        scytale_primitives::from_hex(&block_hex)
                                            .ok()
                                            .map(P2pOutbound::Block)
                                    }
                                    NetworkEvent::BroadcastTransaction { tx_hex, .. } => {
                                        scytale_primitives::from_hex(&tx_hex)
                                            .ok()
                                            .map(P2pOutbound::Transaction)
                                    }
                                };
                                if let Some(message) = message {
                                    if outbound.send(message).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }));
                        p2p_swarm_handle = Some(swarm_task);
                    }
                    Err(error) => tracing::error!(%error, "failed to start libp2p networking"),
                }
            }
            let http_handle = if !opts.no_http {
                let http_node = Arc::clone(&node);
                let http_bind = opts.http_bind.clone();
                let receiver = shutdown_tx.subscribe();
                Some(tokio::spawn(async move {
                    if let Err(error) =
                        scytale_node::run_http_gateway(&http_bind, http_node, receiver).await
                    {
                        tracing::error!("HTTP gateway error: {error}");
                    }
                }))
            } else {
                None
            };
            tokio::select! {
                _ = shutdown_rx.recv() => tracing::info!("IPC shutdown signal received"),
                result = tokio::signal::ctrl_c() => {
                    if let Err(error) = result { tracing::error!("failed to listen for Ctrl+C: {error}"); }
                    let _ = shutdown_tx.send(());
                }
            }
            tracing::info!("initiating node shutdown sequence");
            let shutdown_node = Arc::clone(&node);
            match tokio::task::spawn_blocking(move || shutdown_node.shutdown()).await {
                Ok(Ok(())) => tracing::info!("node shutdown completed cleanly"),
                Ok(Err(error)) => tracing::error!("error during shutdown: {error}"),
                Err(error) => tracing::error!("shutdown task failed to join: {error}"),
            }
            let _ = ipc_handle.await;
            if let Some(handle) = http_handle {
                let _ = handle.await;
            }
            if let Some(handle) = p2p_inbound_handle {
                handle.abort();
            }
            if let Some(handle) = p2p_outbound_handle {
                handle.abort();
            }
            if let Some(shutdown) = p2p_shutdown {
                let _ = shutdown.send(()).await;
            }
            if let Some(handle) = p2p_swarm_handle {
                let _ = handle.await;
            }
        }
        Ok(Err(error)) => tracing::error!("node failed to start: {error}"),
        Err(error) => tracing::error!("node start task failed to join: {error}"),
    }
}
