mod message;

use futures::StreamExt;
use std::net::{SocketAddr, ToSocketAddrs};
use std::time::Duration;
use tokio::sync::mpsc;

pub use message::{Heartbeat, BLOCKS_SUBJECT, HEARTBEAT_SUBJECT, TRANSACTIONS_SUBJECT};

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

#[derive(Clone)]
pub struct P2pEngine {
    client: async_nats::Client,
    node_id: String,
}

impl P2pEngine {
    pub async fn connect(nats_url: &str, node_id: String) -> NetworkResult<Self> {
        let target = Self::resolve_broker_socket(nats_url)?;

        let socket_ok = std::net::TcpStream::connect_timeout(&target, Duration::from_secs(2));
        if let Err(err) = socket_ok {
            return Err(P2pError::BrokerUnreachable(format!(
                "unable to reach NATS broker at {nats_url}: {err}"
            )));
        }

        let client = async_nats::ConnectOptions::new()
            .name(node_id.clone())
            .connect(nats_url)
            .await
            .map_err(|err| P2pError::Connection(err.to_string()))?;
        Ok(Self { client, node_id })
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

        let mut resolved = host_port
            .to_socket_addrs()
            .map_err(|err| P2pError::BrokerUnreachable(format!("invalid NATS address {nats_url}: {err}")))?;

        resolved
            .next()
            .ok_or_else(|| P2pError::BrokerUnreachable(format!("no socket address resolved for {nats_url}")))
    }

    pub async fn listen_blocks(&self, sender: mpsc::Sender<Vec<u8>>) -> NetworkResult<()> {
        Self::listen_subject(&self.client, BLOCKS_SUBJECT, sender).await
    }

    pub async fn listen_transactions(&self, sender: mpsc::Sender<Vec<u8>>) -> NetworkResult<()> {
        Self::listen_subject(&self.client, TRANSACTIONS_SUBJECT, sender).await
    }

    pub async fn broadcast_block(&self, data: &[u8]) -> NetworkResult<()> {
        self.client
            .publish(BLOCKS_SUBJECT, data.to_vec().into())
            .await
            .map_err(|err| P2pError::Operation(err.to_string()))?;
        Ok(())
    }

    pub async fn broadcast_transaction(&self, data: &[u8]) -> NetworkResult<()> {
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
            .publish(HEARTBEAT_SUBJECT, heartbeat.encode().map_err(|err| P2pError::Operation(err.to_string()))?.into())
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