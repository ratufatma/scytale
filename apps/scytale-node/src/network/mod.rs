mod message;

use futures::StreamExt;
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub use message::{
    decode, encode, BlockRequest, BlocksResponse, HeadersResponse, Heartbeat, Hello,
    LocatorRequest, BLOCKS_SUBJECT, HEARTBEAT_SUBJECT, HELLO_SUBJECT, MAX_SYNC_ITEMS,
    PROTOCOL_VERSION, SYNC_BLOCKS_SUBJECT, SYNC_HEADERS_SUBJECT, SYNC_LOCATOR_SUBJECT,
    TRANSACTIONS_SUBJECT,
};

#[derive(Debug, thiserror::Error)]
pub enum P2pError {
    #[error("NATS broker unreachable: {0}")]
    BrokerUnreachable(String),
    #[error("NATS connection failed: {0}")]
    Connection(String),
    #[error("NATS operation failed: {0}")]
    Operation(String),
}

pub type NetworkResult<T> = Result<T, P2pError>;

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
    pub fn allow(&self, node_id: &str, limit: u32, window: Duration) -> bool {
        let mut requests = self.requests.lock().unwrap();
        let entry = requests
            .entry(node_id.to_owned())
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

pub fn validate_hello(hello: &Hello, network_id: u32, genesis_hash: [u8; 32]) -> NetworkResult<()> {
    if hello.protocol_version != message::PROTOCOL_VERSION {
        return Err(P2pError::Operation(format!(
            "unsupported protocol version {}",
            hello.protocol_version
        )));
    }
    if hello.network_id != network_id {
        return Err(P2pError::Operation("network id mismatch".to_owned()));
    }
    if hello.genesis_hash != genesis_hash {
        return Err(P2pError::Operation("genesis hash mismatch".to_owned()));
    }
    if hello.node_id.trim().is_empty() {
        return Err(P2pError::Operation("empty node id".to_owned()));
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

    pub fn get(&self, node_id: &str) -> Option<PeerInfo> {
        self.peers.lock().unwrap().get(node_id).cloned()
    }

    pub fn contains(&self, node_id: &str) -> bool {
        self.peers.lock().unwrap().contains_key(node_id)
    }

    pub fn len(&self) -> usize {
        self.peers.lock().unwrap().len()
    }
}

#[derive(Clone)]
pub struct P2pEngine {
    client: async_nats::Client,
    jetstream: Option<async_nats::jetstream::Context>,
    node_id: String,
}

impl P2pEngine {
    pub async fn connect(
        nats_url: &str,
        node_id: String,
        credentials_path: Option<String>,
        ca_path: Option<String>,
        token: Option<String>,
    ) -> NetworkResult<Self> {
        let target = Self::resolve_broker_socket(nats_url)?;

        let socket_ok = std::net::TcpStream::connect_timeout(&target, Duration::from_secs(2));
        if let Err(err) = socket_ok {
            return Err(P2pError::BrokerUnreachable(format!(
                "unable to reach NATS broker at {nats_url}: {err}"
            )));
        }

        let mut options = if let Some(path) = credentials_path {
            async_nats::ConnectOptions::with_credentials_file(path)
                .await
                .map_err(|err| {
                    P2pError::Connection(format!("failed to load NATS credentials: {err}"))
                })?
        } else {
            async_nats::ConnectOptions::new()
        };
        if let Some(token) = token {
            options = options.token(token);
        }
        if nats_url.starts_with("tls://") || ca_path.is_some() {
            options = options.require_tls(true);
            if let Some(path) = ca_path {
                options = options.add_root_certificates(PathBuf::from(path));
            }
        }
        let client = options
            .name(node_id.clone())
            .connect(nats_url)
            .await
            .map_err(|err| P2pError::Connection(err.to_string()))?;
        let jetstream = async_nats::jetstream::new(client.clone())
            .get_or_create_stream(async_nats::jetstream::stream::Config {
                name: "SCYTALE_STREAM".to_owned(),
                subjects: vec!["scytale.v1.>".to_owned()],
                storage: async_nats::jetstream::stream::StorageType::File,
                retention: async_nats::jetstream::stream::RetentionPolicy::Limits,
                ..Default::default()
            })
            .await
            .map(|_| async_nats::jetstream::new(client.clone()))
            .map_err(|err| {
                tracing::warn!(error = %err, "JetStream unavailable; falling back to Core NATS");
                err
            })
            .ok();
        Ok(Self {
            client,
            jetstream,
            node_id,
        })
    }

    fn resolve_broker_socket(nats_url: &str) -> Result<SocketAddr, P2pError> {
        let cleaned = nats_url
            .trim()
            .trim_start_matches("nats://")
            .trim_start_matches("tls://")
            .trim_start_matches("ws://")
            .trim_start_matches("wss://");

        let host_port = cleaned.split('/').next().unwrap_or(cleaned);
        let host_port = if host_port.is_empty() {
            "127.0.0.1:4222".to_string()
        } else {
            host_port.to_string()
        };

        let mut resolved = host_port.to_socket_addrs().map_err(|err| {
            P2pError::BrokerUnreachable(format!("invalid NATS address {nats_url}: {err}"))
        })?;

        resolved.next().ok_or_else(|| {
            P2pError::BrokerUnreachable(format!("no socket address resolved for {nats_url}"))
        })
    }

    pub async fn listen_blocks(&self, sender: mpsc::Sender<Vec<u8>>) -> NetworkResult<()> {
        if let Some(js) = &self.jetstream {
            let stream = js
                .get_stream("SCYTALE_STREAM")
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            let consumer: async_nats::jetstream::consumer::PullConsumer = stream
                .get_or_create_consumer(
                    &format!("{}_blocks", self.node_id),
                    async_nats::jetstream::consumer::pull::Config {
                        durable_name: Some(format!("{}_blocks", self.node_id)),
                        filter_subject: BLOCKS_SUBJECT.to_owned(),
                        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                        ..Default::default()
                    },
                )
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            let mut messages = consumer
                .messages()
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            while let Some(message) = messages.next().await {
                let message = message.map_err(|err| P2pError::Operation(err.to_string()))?;
                if sender.send(message.payload.to_vec()).await.is_err() {
                    break;
                }
                message
                    .ack()
                    .await
                    .map_err(|err| P2pError::Operation(err.to_string()))?;
            }
            return Ok(());
        }
        Self::listen_subject(&self.client, BLOCKS_SUBJECT, sender).await
    }

    pub async fn listen_transactions(&self, sender: mpsc::Sender<Vec<u8>>) -> NetworkResult<()> {
        if let Some(js) = &self.jetstream {
            let stream = js
                .get_stream("SCYTALE_STREAM")
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            let consumer: async_nats::jetstream::consumer::PullConsumer = stream
                .get_or_create_consumer(
                    &format!("{}_transactions", self.node_id),
                    async_nats::jetstream::consumer::pull::Config {
                        durable_name: Some(format!("{}_transactions", self.node_id)),
                        filter_subject: TRANSACTIONS_SUBJECT.to_owned(),
                        ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
                        ..Default::default()
                    },
                )
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            let mut messages = consumer
                .messages()
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            while let Some(message) = messages.next().await {
                let message = message.map_err(|err| P2pError::Operation(err.to_string()))?;
                if sender.send(message.payload.to_vec()).await.is_err() {
                    break;
                }
                message
                    .ack()
                    .await
                    .map_err(|err| P2pError::Operation(err.to_string()))?;
            }
            return Ok(());
        }
        Self::listen_subject(&self.client, TRANSACTIONS_SUBJECT, sender).await
    }

    pub async fn publish_hello(&self, hello: &Hello) -> NetworkResult<()> {
        self.client
            .publish(
                HELLO_SUBJECT,
                encode(hello)
                    .map_err(|err| P2pError::Operation(err.to_string()))?
                    .into(),
            )
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))?;
        Ok(())
    }

    pub async fn request(&self, subject: &str, payload: Vec<u8>) -> NetworkResult<Vec<u8>> {
        let response = tokio::time::timeout(
            Duration::from_secs(5),
            self.client.request(subject.to_owned(), payload.into()),
        )
        .await
        .map_err(|_| P2pError::Operation(format!("NATS request timed out: {subject}")))?
        .map_err(|err| P2pError::Operation(err.to_string()))?;
        Ok(response.payload.to_vec())
    }

    pub async fn subscribe(&self, subject: &str) -> NetworkResult<async_nats::Subscriber> {
        self.client
            .subscribe(subject.to_owned())
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))
    }

    pub async fn reply(
        &self,
        reply_subject: Option<async_nats::Subject>,
        payload: Vec<u8>,
    ) -> NetworkResult<()> {
        if let Some(subject) = reply_subject {
            self.client
                .publish(subject, payload.into())
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
        }
        Ok(())
    }

    pub async fn broadcast_block(&self, data: &[u8]) -> NetworkResult<()> {
        if let Some(js) = &self.jetstream {
            js.publish(BLOCKS_SUBJECT, data.to_vec().into())
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            return Ok(());
        }
        self.client
            .publish(BLOCKS_SUBJECT, data.to_vec().into())
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))?;
        Ok(())
    }

    pub async fn broadcast_transaction(&self, data: &[u8]) -> NetworkResult<()> {
        if let Some(js) = &self.jetstream {
            js.publish(TRANSACTIONS_SUBJECT, data.to_vec().into())
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?
                .await
                .map_err(|err| P2pError::Operation(err.to_string()))?;
            return Ok(());
        }
        self.client
            .publish(TRANSACTIONS_SUBJECT, data.to_vec().into())
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))?;
        Ok(())
    }

    pub async fn publish_heartbeat(&self, height: u64, best_hash: [u8; 32]) -> NetworkResult<()> {
        let heartbeat = Heartbeat {
            node_id: self.node_id.clone(),
            height,
            best_hash,
        };
        self.client
            .publish(
                HEARTBEAT_SUBJECT,
                heartbeat
                    .encode()
                    .map_err(|err| P2pError::Operation(err.to_string()))?
                    .into(),
            )
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))?;
        Ok(())
    }

    pub async fn shutdown(&self) -> NetworkResult<()> {
        self.client
            .flush()
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))?;
        Ok(())
    }

    async fn listen_subject(
        client: &async_nats::Client,
        subject: &str,
        sender: mpsc::Sender<Vec<u8>>,
    ) -> NetworkResult<()> {
        let mut subscription = client
            .subscribe(subject.to_owned())
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))?;
        while let Some(message) = subscription.next().await {
            if sender.send(message.payload.to_vec()).await.is_err() {
                break;
            }
        }
        Ok(())
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
        assert!(limiter.allow("node-a", 1, Duration::from_secs(60)));
        assert!(!limiter.allow("node-a", 1, Duration::from_secs(60)));
    }
}
