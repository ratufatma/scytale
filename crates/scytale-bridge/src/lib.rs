//! Scytale Bridge: IPC framing and event exchange with Go P2P network daemon.

use scytale_core::{Block, Hash, Transaction};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BridgeError {
    #[error("IPC channel disconnected")]
    ChannelDisconnected,
    #[error("Message serialization error: {0}")]
    Serialization(String),
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
}
