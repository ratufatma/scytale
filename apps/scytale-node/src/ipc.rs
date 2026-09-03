//! IPC Server: Unix domain socket request-response dispatcher for scytale-node.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::node::Node;
use crate::passbook::{
    EntryStatus, EntryType, Passbook, PassbookView, ProvenanceCategory, ProvenanceStep,
};
use scytale_bridge::{
    read_ipc_message, write_ipc_message, EntryStatusDto, EntryTypeDto, NodeRequest, NodeResponse,
    PassbookEntryDto, PassbookViewDto, ProvenanceCategoryDto, ProvenanceStepDto,
    ProvenanceTraceDto, UtxoDto,
};
use scytale_core::{Hash256, OutPoint};
use scytale_primitives::from_hex;

pub const DEFAULT_SOCKET_PATH: &str = "/tmp/scytale.sock";

/// IPC server listening for commands from `scytale-cli`.
pub struct IpcServer {
    socket_path: PathBuf,
    node: Arc<Node>,
    shutdown_sender: broadcast::Sender<()>,
}

impl IpcServer {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        node: Arc<Node>,
        shutdown_sender: broadcast::Sender<()>,
    ) -> Self {
        Self {
            socket_path: socket_path.into(),
            node,
            shutdown_sender,
        }
    }

    /// Runs the IPC listener loop until cancellation.
    pub async fn run(self) -> Result<(), std::io::Error> {
        let path = &self.socket_path;
        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }

        let listener = UnixListener::bind(path)?;
        info!(socket = %path.display(), "IPC server listening on Unix socket");

        let mut shutdown_rx = self.shutdown_sender.subscribe();

        loop {
            tokio::select! {
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((stream, _addr)) => {
                            let node = Arc::clone(&self.node);
                            let shutdown_tx = self.shutdown_sender.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_client(stream, node, shutdown_tx).await {
                                    error!("IPC client handling error: {e}");
                                }
                            });
                        }
                        Err(e) => {
                            warn!("IPC socket accept error: {e}");
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("IPC server received shutdown signal");
                    break;
                }
            }
        }

        if path.exists() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
    }
}

async fn handle_client(
    stream: UnixStream,
    node: Arc<Node>,
    shutdown_tx: broadcast::Sender<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = tokio::io::BufReader::new(reader);

    while let Some(req) = read_ipc_message::<_, NodeRequest>(&mut buf_reader).await? {
        let resp = process_request(req, &node, &shutdown_tx).await;
        write_ipc_message(&mut writer, &resp).await?;
    }
    Ok(())
}

async fn process_request(
    req: NodeRequest,
    node: &Node,
    shutdown_tx: &broadcast::Sender<()>,
) -> NodeResponse {
    match req {
        NodeRequest::GetStatus => NodeResponse::Status {
            state: format!("{:?}", node.state()),
            canonical_height: node.canonical_height(),
            canonical_tip_hash: node.canonical_tip().to_string(),
            mempool_count: node.mempool_len(),
            mining_active: node.mining_running(),
        },

        NodeRequest::GetPassbook { locking_script_hex } => {
            let lock_bytes = match from_hex(&locking_script_hex) {
                Ok(b) => b,
                Err(e) => {
                    return NodeResponse::Error {
                        message: format!("Invalid locking script hex: {e}"),
                    }
                }
            };

            let passbook = Passbook::new(vec![lock_bytes]);
            match passbook.view(node) {
                Ok(view) => NodeResponse::Passbook(map_passbook_view(locking_script_hex, view)),
                Err(e) => NodeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        NodeRequest::SendTransaction {
            recipient_script_hex,
            amount_quanta,
            fee_quanta,
            sender_script_hex,
        } => {
            let recipient_bytes = match from_hex(&recipient_script_hex) {
                Ok(b) => b,
                Err(e) => {
                    return NodeResponse::Error {
                        message: format!("Invalid recipient script hex: {e}"),
                    }
                }
            };

            let sender_bytes = if let Some(ref s) = sender_script_hex {
                match from_hex(s) {
                    Ok(b) => Some(b),
                    Err(e) => {
                        return NodeResponse::Error {
                            message: format!("Invalid sender script hex: {e}"),
                        }
                    }
                }
            } else {
                None
            };

            match node.create_and_submit_transaction(
                recipient_bytes,
                amount_quanta,
                fee_quanta,
                sender_bytes,
            ) {
                Ok(txid) => NodeResponse::TransactionSubmitted {
                    txid: txid.to_string(),
                },
                Err(e) => NodeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        NodeRequest::SetMining { enabled } => {
            let res = if enabled {
                node.start_mining()
            } else {
                node.stop_mining()
            };

            match res {
                Ok(_) => NodeResponse::MiningToggled {
                    active: node.mining_running(),
                },
                Err(e) => NodeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        NodeRequest::TraceProvenance {
            txid_hex,
            index,
            max_depth: _,
        } => {
            let txid = match Hash256::from_str(&txid_hex) {
                Ok(h) => h,
                Err(e) => {
                    return NodeResponse::Error {
                        message: format!("Invalid txid hex: {e}"),
                    }
                }
            };

            let outpoint = OutPoint::new(txid, index);
            let passbook = Passbook::new(vec![]);
            match passbook.provenance(node, &outpoint) {
                Ok(steps) => NodeResponse::Provenance(map_provenance_trace(outpoint, steps)),
                Err(e) => NodeResponse::Error {
                    message: e.to_string(),
                },
            }
        }

        NodeRequest::ConnectPeer { addr } => {
            node.connect_peer(addr.clone());
            NodeResponse::Success {
                message: format!("Initiated peer connection to {addr}"),
            }
        }

        NodeRequest::StopNode => {
            let _ = shutdown_tx.send(());
            NodeResponse::Success {
                message: "Node shutdown signal sent".into(),
            }
        }

        NodeRequest::GetUtxosByLock { locking_script } => {
            let utxos = node.query_utxo_set();
            let dtos = utxos
                .entries()
                .iter()
                .filter(|(_, entry)| entry.output.locking_condition == locking_script)
                .map(|(outpoint, entry)| UtxoDto {
                    txid_hex: outpoint.txid.to_string(),
                    index: outpoint.index,
                    value_quanta: entry.output.value,
                    locking_script_hex: scytale_primitives::to_hex(&entry.output.locking_condition),
                    block_height: entry.block_height,
                    is_coinbase: entry.is_coinbase,
                })
                .collect();
            NodeResponse::Utxos(dtos)
        }

        NodeRequest::SubmitRawTransaction { tx } => match node.submit_transaction(*tx) {
            Ok(txid) => NodeResponse::TransactionSubmitted {
                txid: txid.to_string(),
            },
            Err(e) => NodeResponse::Error {
                message: e.to_string(),
            },
        },

        NodeRequest::ExportSnapshotChunk {
            block_hash_hex,
            chunk_index,
            chunk_size,
        } => match node.export_snapshot_chunk(&block_hash_hex, chunk_index, chunk_size) {
            Ok((block_hash_hex, chunk_index, total_chunks, entries)) => {
                NodeResponse::SnapshotChunk {
                    block_hash_hex,
                    chunk_index,
                    total_chunks,
                    entries,
                }
            }
            Err(e) => NodeResponse::Error {
                message: e.to_string(),
            },
        },

        NodeRequest::ApplySnapshot {
            block_hash_hex,
            entries,
        } => match node.apply_snapshot(&block_hash_hex, &entries) {
            Ok(utxo_count) => NodeResponse::SnapshotApplied {
                block_hash_hex,
                utxo_count,
            },
            Err(e) => NodeResponse::Error {
                message: e.to_string(),
            },
        },
    }
}

pub(crate) fn map_passbook_view(lock_hex: String, view: PassbookView) -> PassbookViewDto {
    let entries = view
        .entries
        .into_iter()
        .map(|e| PassbookEntryDto {
            entry_number: e.entry_number,
            timestamp: e.timestamp,
            entry_type: match e.entry_type {
                EntryType::Received => EntryTypeDto::Received,
                EntryType::Sent => EntryTypeDto::Sent,
                EntryType::MiningReward => EntryTypeDto::MiningReward,
                EntryType::Change => EntryTypeDto::Change,
            },
            amount_quanta: e.amount_quanta,
            fee_quanta: e.fee_quanta,
            status: match e.status {
                EntryStatus::Confirmed { confirmations } => {
                    EntryStatusDto::Confirmed { confirmations }
                }
                EntryStatus::Pending => EntryStatusDto::Pending,
                EntryStatus::Reorganized => EntryStatusDto::Reorganized,
            },
            txid_hex: e.txid.to_string(),
            outpoint: e.outpoint.map(|op| format!("{}:{}", op.txid, op.index)),
            block_height: e.block_height,
        })
        .collect();

    PassbookViewDto {
        account_lock_hex: lock_hex,
        confirmed_balance_quanta: view.confirmed_balance_quanta,
        pending_balance_quanta: view.pending_balance_quanta,
        total_entries: view.total_entries,
        entries,
    }
}

pub(crate) fn map_provenance_trace(
    outpoint: OutPoint,
    steps: Vec<ProvenanceStep>,
) -> ProvenanceTraceDto {
    let steps_dto = steps
        .into_iter()
        .map(|s| ProvenanceStepDto {
            txid_hex: s.txid.to_string(),
            block_height: s.block_height,
            category: match s.category {
                ProvenanceCategory::Coinbase => ProvenanceCategoryDto::Coinbase,
                ProvenanceCategory::Genesis => ProvenanceCategoryDto::Genesis,
                ProvenanceCategory::Transfer => ProvenanceCategoryDto::Transfer,
            },
            value_quanta: s.value_quanta,
        })
        .collect();

    ProvenanceTraceDto {
        target_outpoint: format!("{}:{}", outpoint.txid, outpoint.index),
        steps: steps_dto,
    }
}
