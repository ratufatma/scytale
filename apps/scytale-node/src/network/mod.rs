pub mod config;
mod message;
pub mod p2p;
pub mod sync;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub use message::{
    decode, encode, BlockRequest, BlocksResponse, HeadersResponse, Heartbeat, Hello,
    LocatorRequest, MAX_SYNC_ITEMS, PROTOCOL_VERSION,
};

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub hello: Hello,
    pub last_seen: Instant,
    pub score: i32,
}

#[derive(Clone, Default)]
pub struct PeerRegistry {
    peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
}

#[derive(Clone, Default)]
pub struct RateLimiter {
    requests: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
}

impl RateLimiter {
    pub fn allow(&self, peer_id: &str, limit: u32, window: Duration) -> bool {
        let mut requests = self.requests.lock().unwrap();
        let entry = requests
            .entry(peer_id.to_owned())
            .or_insert_with(|| (Instant::now(), 0));
        if entry.0.elapsed() >= window {
            *entry = (Instant::now(), 0);
        }
        if entry.1 >= limit {
            return false;
        }
        entry.1 += 1;
        true
    }
}

pub fn validate_hello(
    hello: &Hello,
    network_id: u32,
    genesis_hash: [u8; 32],
) -> Result<(), String> {
    if hello.protocol_version != PROTOCOL_VERSION {
        return Err(format!(
            "unsupported protocol version {}",
            hello.protocol_version
        ));
    }
    if hello.network_id != network_id {
        return Err("network id mismatch".to_owned());
    }
    if hello.genesis_hash != genesis_hash {
        return Err("genesis hash mismatch".to_owned());
    }
    if hello.node_id.trim().is_empty() {
        return Err("empty node id".to_owned());
    }
    Ok(())
}

impl PeerRegistry {
    pub fn upsert(&self, hello: Hello) {
        self.peers.lock().unwrap().insert(
            hello.node_id.clone(),
            PeerInfo {
                hello,
                last_seen: Instant::now(),
                score: 0,
            },
        );
    }

    pub fn remove_expired(&self, timeout: Duration) -> usize {
        let mut peers = self.peers.lock().unwrap();
        let before = peers.len();
        peers.retain(|_, peer| peer.last_seen.elapsed() <= timeout);
        before - peers.len()
    }

    pub fn get(&self, peer_id: &str) -> Option<PeerInfo> {
        self.peers.lock().unwrap().get(peer_id).cloned()
    }

    pub fn contains(&self, peer_id: &str) -> bool {
        self.peers.lock().unwrap().contains_key(peer_id)
    }

    pub fn len(&self) -> usize {
        self.peers.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_hello, Hello, PeerRegistry, RateLimiter};
    use crate::network::message::PROTOCOL_VERSION;
    use std::time::Duration;

    fn hello() -> Hello {
        Hello {
            protocol_version: PROTOCOL_VERSION,
            network_id: 7,
            genesis_hash: [9; 32],
            node_id: "node-a".to_owned(),
            best_height: 3,
            best_hash: [8; 32],
        }
    }

    #[test]
    fn hello_rejects_network_mismatch() {
        let mut peer = hello();
        peer.network_id = 8;
        assert!(validate_hello(&peer, 7, [9; 32]).is_err());
    }

    #[test]
    fn registry_expires_stale_peers() {
        let registry = PeerRegistry::default();
        registry.upsert(hello());
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.remove_expired(Duration::ZERO), 1);
    }

    #[test]
    fn rate_limiter_rejects_after_limit() {
        let limiter = RateLimiter::default();
        assert!(limiter.allow("peer", 1, Duration::from_secs(60)));
        assert!(!limiter.allow("peer", 1, Duration::from_secs(60)));
    }
}
