//! Scytale Network: P2P network primitives and message exchange (Stub).

use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Connection failed to peer: {0}")]
    ConnectionFailed(SocketAddr),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Ping,
    Pong,
    GetBlocks { from_height: u64, count: usize },
    Transactions(Vec<Vec<u8>>),
}

pub struct PeerNode {
    pub addr: SocketAddr,
}

impl PeerNode {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_node_creation() {
        let addr = "127.0.0.1:8333".parse().unwrap();
        let peer = PeerNode::new(addr);
        assert_eq!(peer.addr.port(), 8333);
    }
}
