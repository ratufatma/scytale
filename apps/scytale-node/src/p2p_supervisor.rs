//! P2P Supervisor: Manages child Go P2P daemon and Unix domain socket bridge server.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::node::Node;
use scytale_bridge::{
    read_ipc_message, write_ipc_message, P2pBridgeMessage, P2pBridgeRequest, P2pBridgeResponse,
};
use scytale_core::codec::{CanonicalDeserialize, CanonicalSerialize};
use scytale_core::{Block, Hash256, Transaction};
use scytale_primitives::{from_hex, to_hex};

/// Manages the Go `scytale-p2p` child process and consensus bridge server.
pub struct P2pSupervisor {
    bridge_socket_path: PathBuf,
    p2p_bind: Option<String>,
    peers: Vec<String>,
    p2p_bin: Option<PathBuf>,
    node: Arc<Node>,
    shutdown_sender: broadcast::Sender<()>,
    child_process: Option<Child>,
    fast_sync: bool,
}

impl P2pSupervisor {
    pub fn new(
        bridge_socket_path: impl Into<PathBuf>,
        p2p_bind: Option<String>,
        peers: Vec<String>,
        p2p_bin: Option<PathBuf>,
        node: Arc<Node>,
        shutdown_sender: broadcast::Sender<()>,
    ) -> Self {
        Self {
            bridge_socket_path: bridge_socket_path.into(),
            p2p_bind,
            peers,
            p2p_bin,
            node,
            shutdown_sender,
            child_process: None,
            fast_sync: false,
        }
    }

    /// Configures fast sync state download mode for the supervised daemon.
    pub fn set_fast_sync(&mut self, fast_sync: bool) {
        self.fast_sync = fast_sync;
    }

    /// Resolves or compiles the `scytale-p2p` Go executable.
    fn resolve_p2p_binary(&self) -> Result<PathBuf, std::io::Error> {
        if let Some(ref path) = self.p2p_bin {
            if path.exists() {
                return Ok(path.clone());
            }
        }

        // Check system PATH locations, target/release, target/debug, or network/bin
        let candidates = [
            PathBuf::from("/usr/local/bin/scytale-p2p"),
            PathBuf::from("target/release/scytale-p2p"),
            PathBuf::from("target/debug/scytale-p2p"),
            PathBuf::from("../target/release/scytale-p2p"),
            PathBuf::from("../target/debug/scytale-p2p"),
            PathBuf::from("../../target/release/scytale-p2p"),
            PathBuf::from("../../target/debug/scytale-p2p"),
            PathBuf::from("network/bin/scytale-p2p"),
            PathBuf::from("/tmp/scytale-p2p"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(candidate.clone());
            }
        }

        // Try building from network/cmd/scytale-p2p if Go is available
        let network_dir = if Path::new("network").exists() {
            PathBuf::from("network")
        } else if Path::new("../network").exists() {
            PathBuf::from("../network")
        } else {
            PathBuf::from("../../network")
        };

        if network_dir.exists() {
            info!("compiling Go P2P daemon from {}", network_dir.display());
            let out_bin = PathBuf::from("/tmp/scytale-p2p");
            let status = Command::new("go")
                .args([
                    "build",
                    "-o",
                    out_bin.to_str().unwrap(),
                    "./cmd/scytale-p2p",
                ])
                .current_dir(&network_dir)
                .status();

            if let Ok(st) = status {
                if st.success() && out_bin.exists() {
                    return Ok(out_bin);
                }
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not locate or compile scytale-p2p executable. Ensure Go is installed or pass --p2p-bin",
        ))
    }

    /// Spawns the supervisor loop and the child Go daemon process.
    pub async fn run(mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let sock_path = self.bridge_socket_path.clone();
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }
        if let Some(parent) = sock_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let listener = UnixListener::bind(&sock_path)?;
        info!(bridge_socket = %sock_path.display(), "P2P bridge listening on Unix socket");

        // Resolve binary and spawn child process
        match self.resolve_p2p_binary() {
            Ok(bin_path) => {
                let mut cmd = Command::new(&bin_path);
                cmd.arg("--bridge-sock").arg(&sock_path);
                cmd.arg("--allow-local-peers");
                if self.fast_sync {
                    cmd.arg("--fast-sync");
                }

                if let Some(ref bind) = self.p2p_bind {
                    cmd.arg("--p2p-bind").arg(bind);
                }
                for peer in &self.peers {
                    cmd.arg("--peer").arg(peer);
                }

                cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());

                match cmd.spawn() {
                    Ok(child) => {
                        info!(pid = child.id(), bin = %bin_path.display(), "spawned Go P2P daemon child process");
                        self.child_process = Some(child);
                    }
                    Err(e) => {
                        error!("failed to spawn Go P2P daemon: {e}");
                    }
                }
            }
            Err(e) => {
                warn!("could not launch child Go P2P daemon: {e}");
            }
        }

        let mut shutdown_rx = self.shutdown_sender.subscribe();

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, _addr)) => {
                            let node = Arc::clone(&self.node);
                            tokio::spawn(async move {
                                if let Err(e) = handle_p2p_bridge_stream(stream, node).await {
                                    error!("P2P bridge session error: {e}");
                                }
                            });
                        }
                        Err(e) => {
                            warn!("P2P bridge accept error: {e}");
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("P2P supervisor received shutdown signal");
                    break;
                }
            }
        }

        // Clean up child process
        if let Some(mut child) = self.child_process.take() {
            info!("stopping Go P2P daemon child process");
            let _ = child.kill();
            let _ = child.wait();
        }

        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }
        Ok(())
    }
}

async fn handle_p2p_bridge_stream(
    stream: UnixStream,
    node: Arc<Node>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = tokio::io::BufReader::new(reader);
    let mut event_rx = node.subscribe_p2p_events();

    loop {
        tokio::select! {
            msg_res = read_ipc_message::<_, P2pBridgeMessage>(&mut buf_reader) => {
                match msg_res? {
                    Some(P2pBridgeMessage::Request { id, request }) => {
                        let response = process_p2p_request(request, &node).await;
                        let resp_msg = P2pBridgeMessage::Response { id, response };
                        write_ipc_message(&mut writer, &resp_msg).await?;
                    }
                    Some(_) => {}
                    None => {
                        info!("P2P bridge Go client disconnected");
                        break;
                    }
                }
            }
            event_res = event_rx.recv() => {
                if let Ok(event) = event_res {
                    let event_msg = P2pBridgeMessage::Event { event };
                    if let Err(e) = write_ipc_message(&mut writer, &event_msg).await {
                        error!("failed to write broadcast event to P2P bridge: {e}");
                        break;
                    }
                }
            }
        }
    }
    Ok(())
}

async fn process_p2p_request(req: P2pBridgeRequest, node: &Node) -> P2pBridgeResponse {
    match req {
        P2pBridgeRequest::SubmitBlock { block_hex } => {
            let bytes = match from_hex(&block_hex) {
                Ok(b) => b,
                Err(e) => {
                    return P2pBridgeResponse::Error {
                        message: format!("invalid block hex: {e}"),
                    }
                }
            };

            let block = match Block::from_canonical_bytes(&bytes) {
                Ok(b) => b,
                Err(e) => {
                    return P2pBridgeResponse::Error {
                        message: format!("block deserialization error: {e}"),
                    }
                }
            };

            match node.submit_external_block(block) {
                Ok(_) => P2pBridgeResponse::Ok,
                Err(e) => P2pBridgeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        P2pBridgeRequest::SubmitTransaction { tx_hex } => {
            let bytes = match from_hex(&tx_hex) {
                Ok(b) => b,
                Err(e) => {
                    return P2pBridgeResponse::Error {
                        message: format!("invalid tx hex: {e}"),
                    }
                }
            };

            let tx = match Transaction::from_canonical_bytes(&bytes) {
                Ok(t) => t,
                Err(e) => {
                    return P2pBridgeResponse::Error {
                        message: format!("tx deserialization error: {e}"),
                    }
                }
            };

            match node.submit_transaction(tx) {
                Ok(_) => P2pBridgeResponse::Ok,
                Err(e) => P2pBridgeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        P2pBridgeRequest::GetBlockLocator => match node.get_block_locator() {
            Ok(hashes) => {
                let hashes_hex = hashes.iter().map(|h| h.to_string()).collect();
                P2pBridgeResponse::BlockLocator { hashes_hex }
            }
            Err(e) => P2pBridgeResponse::Error {
                message: e.to_string(),
            },
        },

        P2pBridgeRequest::GetCanonicalHashes => match node.get_canonical_hashes() {
            Ok(hashes) => {
                let hashes_hex = hashes.iter().map(|h| h.to_string()).collect();
                P2pBridgeResponse::CanonicalHashes { hashes_hex }
            }
            Err(e) => P2pBridgeResponse::Error {
                message: e.to_string(),
            },
        },

        P2pBridgeRequest::GetBlockByHash { hash_hex } => {
            let hash = match Hash256::from_str(&hash_hex) {
                Ok(h) => h,
                Err(e) => {
                    return P2pBridgeResponse::Error {
                        message: format!("invalid hash hex: {e}"),
                    }
                }
            };

            match node.storage_handle().get_block(&hash) {
                Ok(Some(block)) => match block.to_canonical_bytes() {
                    Ok(bytes) => P2pBridgeResponse::BlockData {
                        block_hex: Some(to_hex(&bytes)),
                    },
                    Err(e) => P2pBridgeResponse::Error {
                        message: e.to_string(),
                    },
                },
                Ok(None) => P2pBridgeResponse::BlockData { block_hex: None },
                Err(e) => P2pBridgeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        P2pBridgeRequest::GetTransactionByHash { hash_hex } => {
            let txid = match Hash256::from_str(&hash_hex) {
                Ok(h) => h,
                Err(e) => {
                    return P2pBridgeResponse::Error {
                        message: format!("invalid txid hex: {e}"),
                    }
                }
            };

            match node.lookup_transaction(&txid) {
                Ok(Some(tx)) => match tx.to_canonical_bytes() {
                    Ok(bytes) => P2pBridgeResponse::TransactionData {
                        tx_hex: Some(to_hex(&bytes)),
                    },
                    Err(e) => P2pBridgeResponse::Error {
                        message: e.to_string(),
                    },
                },
                Ok(None) => {
                    // Also check mempool
                    let mempool_txs = node.query_mempool();
                    if let Some(entry) = mempool_txs.iter().find(|e| e.transaction.txid() == txid) {
                        match entry.transaction.to_canonical_bytes() {
                            Ok(bytes) => P2pBridgeResponse::TransactionData {
                                tx_hex: Some(to_hex(&bytes)),
                            },
                            Err(e) => P2pBridgeResponse::Error {
                                message: e.to_string(),
                            },
                        }
                    } else {
                        P2pBridgeResponse::TransactionData { tx_hex: None }
                    }
                }
                Err(e) => P2pBridgeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        P2pBridgeRequest::ExportSnapshotChunk {
            block_hash_hex,
            chunk_index,
            chunk_size,
        } => match node.export_snapshot_chunk(&block_hash_hex, chunk_index, chunk_size) {
            Ok((block_hash_hex, chunk_index, total_chunks, entries)) => {
                P2pBridgeResponse::SnapshotChunk {
                    block_hash_hex,
                    chunk_index,
                    total_chunks,
                    entries,
                }
            }
            Err(e) => P2pBridgeResponse::Error {
                message: e.to_string(),
            },
        },

        P2pBridgeRequest::ApplySnapshot {
            block_hash_hex,
            entries,
        } => match node.apply_snapshot(&block_hash_hex, &entries) {
            Ok(utxo_count) => P2pBridgeResponse::SnapshotApplied {
                block_hash_hex,
                utxo_count,
            },
            Err(e) => P2pBridgeResponse::Error {
                message: e.to_string(),
            },
        },

        P2pBridgeRequest::UpdatePeerCount { count } => {
            node.set_peer_count(count);
            P2pBridgeResponse::Ok
        }
    }
}
