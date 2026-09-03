//! Scytale Bridge: IPC framing and event exchange for CLI, Node daemon, and P2P bridge.

use scytale_core::{Block, Hash, Transaction};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("IPC channel disconnected or EOF")]
    ChannelDisconnected,
    #[error("Message serialization error: {0}")]
    Serialization(String),
    #[error("IPC IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Messages exchanged across the IPC bridge between Rust Core and Go P2P.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BridgeMessage {
    /// Broadcast a newly discovered local or relayed transaction to peers.
    BroadcastTransaction(Transaction),
    /// Broadcast a newly mined or validated canonical block to peers.
    BroadcastBlock(Block),
    /// Ingress transaction received from network peer.
    IngressTransaction(Transaction),
    /// Ingress block received from network peer.
    IngressBlock(Block),
    /// Request block headers starting from a specific hash.
    GetHeaders { locator: Vec<Hash>, count: u32 },
}

// ─────────────────────────────────────────────────────────────────────────────
// Node CLI <-> Daemon Local IPC Protocol
// ─────────────────────────────────────────────────────────────────────────────

/// Requests sent from `scytale-cli` to `scytale-node`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeRequest {
    /// Query node runtime state, chain tip, height, mempool count, and mining status.
    GetStatus,
    /// Query the canonical passbook view for a specific locking condition script hex.
    GetPassbook { locking_script_hex: String },
    /// Create, sign (permissive), and admit a transaction into mempool.
    SendTransaction {
        recipient_script_hex: String,
        amount_quanta: u64,
        fee_quanta: u64,
        #[serde(default)]
        sender_script_hex: Option<String>,
    },
    /// Start or stop the background Proof-of-Work mining worker.
    SetMining { enabled: bool },
    /// Trace the value provenance lineage of a specific outpoint.
    TraceProvenance {
        txid_hex: String,
        index: u32,
        #[serde(default)]
        max_depth: Option<usize>,
    },
    /// Dynamically connect to a network peer at runtime.
    ConnectPeer { addr: String },
    /// Request graceful shutdown of the node daemon.
    StopNode,
    /// Query all active unspent transaction outputs matching a locking condition script.
    GetUtxosByLock { locking_script: Vec<u8> },
    /// Submit an externally assembled and signed raw transaction into the mempool.
    SubmitRawTransaction { tx: Box<Transaction> },
    /// Export a chunk of the authenticated UTXO snapshot.
    ExportSnapshotChunk {
        block_hash_hex: String,
        chunk_index: u32,
        chunk_size: u32,
    },
    /// Apply an authenticated UTXO snapshot to the node state.
    ApplySnapshot {
        block_hash_hex: String,
        entries: Vec<UtxoWireEntryDto>,
    },
}

/// Responses returned from `scytale-node` to `scytale-cli`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeResponse {
    Status {
        state: String,
        canonical_height: u64,
        canonical_tip_hash: String,
        mempool_count: usize,
        mining_active: bool,
    },
    Passbook(PassbookViewDto),
    TransactionSubmitted {
        txid: String,
    },
    Utxos(Vec<UtxoDto>),
    SnapshotChunk {
        block_hash_hex: String,
        chunk_index: u32,
        total_chunks: u32,
        entries: Vec<UtxoWireEntryDto>,
    },
    SnapshotApplied {
        block_hash_hex: String,
        utxo_count: usize,
    },
    MiningToggled {
        active: bool,
    },
    Provenance(ProvenanceTraceDto),
    Success {
        message: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtxoDto {
    pub txid_hex: String,
    pub index: u32,
    pub value_quanta: u64,
    pub locking_script_hex: String,
    pub block_height: u64,
    pub is_coinbase: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtxoWireEntryDto {
    pub txid_hex: String,
    pub index: u32,
    pub value_quanta: u64,
    pub locking_script_hex: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryTypeDto {
    Received,
    Sent,
    MiningReward,
    Change,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryStatusDto {
    Confirmed { confirmations: u64 },
    Pending,
    Reorganized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassbookEntryDto {
    pub entry_number: u64,
    pub timestamp: u64,
    pub entry_type: EntryTypeDto,
    pub amount_quanta: u64,
    pub fee_quanta: u64,
    pub status: EntryStatusDto,
    pub txid_hex: String,
    pub outpoint: Option<String>,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassbookViewDto {
    pub account_lock_hex: String,
    pub confirmed_balance_quanta: u64,
    pub pending_balance_quanta: i64,
    pub total_entries: usize,
    pub entries: Vec<PassbookEntryDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenanceCategoryDto {
    Coinbase,
    Genesis,
    Transfer,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceStepDto {
    pub txid_hex: String,
    pub block_height: u64,
    pub category: ProvenanceCategoryDto,
    pub value_quanta: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceTraceDto {
    pub target_outpoint: String,
    pub steps: Vec<ProvenanceStepDto>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Rust Node <-> Go P2P Daemon Bridge Protocol
// ─────────────────────────────────────────────────────────────────────────────

/// RPC requests sent from the Go P2P daemon to the Rust node supervisor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload")]
pub enum P2pBridgeRequest {
    /// Submit raw canonical block bytes (hex) received from a network peer.
    SubmitBlock { block_hex: String },
    /// Submit raw canonical transaction bytes (hex) received from a network peer.
    SubmitTransaction { tx_hex: String },
    /// Request block locator hashes (exponential spacing from canonical tip to genesis).
    GetBlockLocator,
    /// Request all canonical chain block hashes in ascending order (genesis to tip).
    GetCanonicalHashes,
    /// Request raw block bytes (hex) by 32-byte hash.
    GetBlockByHash { hash_hex: String },
    /// Request raw transaction bytes (hex) by 32-byte hash.
    GetTransactionByHash { hash_hex: String },
    /// Request an exported snapshot chunk for a target block hash.
    ExportSnapshotChunk {
        block_hash_hex: String,
        chunk_index: u32,
        chunk_size: u32,
    },
    /// Apply an authenticated UTXO snapshot to local storage.
    ApplySnapshot {
        block_hash_hex: String,
        entries: Vec<UtxoWireEntryDto>,
    },
    /// Update the count of connected network peers.
    UpdatePeerCount { count: usize },
}

/// RPC responses sent from the Rust node supervisor to the Go P2P daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload")]
pub enum P2pBridgeResponse {
    /// Operation succeeded.
    Ok,
    /// Operation failed with an error message.
    Error { message: String },
    /// Block locator hash list response.
    BlockLocator { hashes_hex: Vec<String> },
    /// Full canonical chain block hash list response.
    CanonicalHashes { hashes_hex: Vec<String> },
    /// Block data response (None if not found).
    BlockData { block_hex: Option<String> },
    /// Transaction data response (None if not found).
    TransactionData { tx_hex: Option<String> },
    /// Snapshot chunk data response.
    SnapshotChunk {
        block_hash_hex: String,
        chunk_index: u32,
        total_chunks: u32,
        entries: Vec<UtxoWireEntryDto>,
    },
    /// Snapshot applied response.
    SnapshotApplied {
        block_hash_hex: String,
        utxo_count: usize,
    },
}

/// Asynchronous broadcast events emitted from the Rust node to the Go P2P daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "payload")]
pub enum P2pBridgeEvent {
    /// Broadcast a newly mined or validated canonical block to network peers.
    BroadcastBlock { block_hex: String, hash_hex: String },
    /// Broadcast a newly admitted transaction to network peers.
    BroadcastTransaction { tx_hex: String, txid_hex: String },
    /// Instruct the P2P daemon to connect to a new peer address dynamically.
    ConnectPeer { addr: String },
}

/// Message envelope multiplexed across the Go-Rust bridge Unix domain socket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind")]
pub enum P2pBridgeMessage {
    Request {
        id: u64,
        request: P2pBridgeRequest,
    },
    Response {
        id: u64,
        response: P2pBridgeResponse,
    },
    Event {
        event: P2pBridgeEvent,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Async JSON Framing over IPC Streams
// ─────────────────────────────────────────────────────────────────────────────

/// Asynchronously writes a newline-delimited JSON message to the stream.
pub async fn write_ipc_message<W, T>(writer: &mut W, msg: &T) -> Result<(), BridgeError>
where
    W: tokio::io::AsyncWriteExt + Unpin,
    T: Serialize,
{
    let mut payload =
        serde_json::to_vec(msg).map_err(|e| BridgeError::Serialization(e.to_string()))?;
    payload.push(b'\n');
    writer.write_all(&payload).await?;
    writer.flush().await?;
    Ok(())
}

/// Asynchronously reads a newline-delimited JSON message from the stream.
pub async fn read_ipc_message<R, T>(reader: &mut R) -> Result<Option<T>, BridgeError>
where
    R: tokio::io::AsyncBufReadExt + Unpin,
    T: serde::de::DeserializeOwned,
{
    let mut line = String::new();
    let bytes_read = reader.read_line(&mut line).await?;
    if bytes_read == 0 {
        return Ok(None);
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let msg: T = serde_json::from_str(trimmed).map_err(|e| {
        BridgeError::Serialization(format!("deserialization error: {e}, payload: {trimmed}"))
    })?;
    Ok(Some(msg))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_message_creation() {
        let msg = BridgeMessage::GetHeaders {
            locator: vec![Hash::ZERO],
            count: 500,
        };
        assert!(matches!(msg, BridgeMessage::GetHeaders { count: 500, .. }));
    }

    #[tokio::test]
    async fn test_ipc_framing_roundtrip() {
        let req = NodeRequest::GetStatus;
        let mut buffer = Vec::new();
        write_ipc_message(&mut buffer, &req).await.unwrap();

        let mut cursor = tokio::io::BufReader::new(buffer.as_slice());
        let decoded: Option<NodeRequest> = read_ipc_message(&mut cursor).await.unwrap();
        assert_eq!(decoded, Some(req));
    }

    #[tokio::test]
    async fn test_p2p_bridge_framing_roundtrip() {
        let msg = P2pBridgeMessage::Request {
            id: 42,
            request: P2pBridgeRequest::GetBlockLocator,
        };
        let mut buffer = Vec::new();
        write_ipc_message(&mut buffer, &msg).await.unwrap();

        let mut cursor = tokio::io::BufReader::new(buffer.as_slice());
        let decoded: Option<P2pBridgeMessage> = read_ipc_message(&mut cursor).await.unwrap();
        assert_eq!(decoded, Some(msg));
    }
}
