use crate::network::sync::SyncState;
use futures_util::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use futures_util::StreamExt;
use libp2p::{
    autonat, connection_limits, gossipsub, identify, identity, kad, noise, ping,
    relay::client as relay_client, request_response, swarm::NetworkBehaviour, swarm::SwarmEvent,
    tcp, yamux, Multiaddr, PeerId, Swarm, Transport,
};
use scytale_core::{Block, CanonicalDeserialize, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use super::config::P2pConfig;

pub const BLOCKS_TOPIC: &str = "scytale/v1/blocks";
pub const TRANSACTIONS_TOPIC: &str = "scytale/v1/txs";
pub const P2P_PROTOCOL: &str = "/scytale/1.0.0";
pub const SYNC_PROTOCOL: &str = "/scytale/sync/1.0.0";
const MAX_SYNC_MESSAGE: usize = 16 * 1024 * 1024;
pub const MAX_BLOCK_PAYLOAD: usize = 2 * 1024 * 1024;
pub const MAX_TRANSACTION_PAYLOAD: usize = 100 * 1024;
const MAX_SYNC_REQUEST: usize = 64 * 1024;
const MAX_REQUESTS_PER_PEER_PER_SECOND: usize = 5;
const MAX_GET_BLOCKS_HASHES: usize = 100;
const INVALID_GOSSIP_MESSAGES_BEFORE_DISCONNECT: u8 = 3;
const MAX_KNOWN_PEERS: usize = 50;
const MAX_PEER_FAILURES: u8 = 3;
const MIN_DISCOVERY_PEERS: usize = 4;
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(30);

pub fn max_peers_per_subnet() -> usize {
    std::env::var("SCYTALE_MAX_PEERS_PER_SUBNET")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16)
}

pub fn subnet_key(address: &Multiaddr) -> Option<Vec<u8>> {
    let ip = address.iter().find_map(|protocol| match protocol {
        libp2p::multiaddr::Protocol::Ip4(ip) => Some(IpAddr::V4(ip)),
        libp2p::multiaddr::Protocol::Ip6(ip) => Some(IpAddr::V6(ip)),
        _ => None,
    })?;
    match ip {
        IpAddr::V4(ip) => {
            if ip.is_loopback() || ip.is_private() {
                return None;
            }
            Some(ip.octets()[..3].to_vec())
        }
        IpAddr::V6(ip) => {
            if ip.is_loopback() {
                return None;
            }
            Some(ip.octets()[..6].to_vec())
        }
    }
}

pub fn subnet_connection_allowed(addresses: &[Multiaddr], candidate: &Multiaddr) -> bool {
    let Some(candidate_subnet) = subnet_key(candidate) else {
        return true;
    };
    addresses
        .iter()
        .filter_map(subnet_key)
        .filter(|subnet| *subnet == candidate_subnet)
        .count()
        < max_peers_per_subnet()
}

#[derive(Clone, Debug)]
pub struct P2pStatus {
    pub connected_peers: usize,
    pub connected_peer_ids: Vec<PeerId>,
    pub sync_state: SyncState,
    pub listen_addrs: Vec<Multiaddr>,
}

impl Default for P2pStatus {
    fn default() -> Self {
        Self {
            connected_peers: 0,
            connected_peer_ids: Vec::new(),
            sync_state: SyncState::Synced,
            listen_addrs: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KnownPeer {
    peer_id: String,
    address: String,
    failures: u8,
    last_seen: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum P2pError {
    #[error("identity I/O failed: {0}")]
    IdentityIo(#[from] std::io::Error),
    #[error("identity encoding failed: {0}")]
    IdentityEncoding(String),
    #[error("known-peer state I/O failed: {0}")]
    PeerStateIo(#[source] std::io::Error),
    #[error("known-peer state encoding failed: {0}")]
    PeerStateEncoding(String),
    #[error("transport setup failed: {0}")]
    Transport(String),
    #[error("network behavior setup failed: {0}")]
    Behaviour(String),
    #[error("invalid multiaddress {0}")]
    InvalidAddress(String),
}

#[derive(Debug)]
pub enum P2pInbound {
    PeerStatus {
        peer: PeerId,
        best_height: u64,
    },
    Block(Vec<u8>),
    Transaction(Vec<u8>),
    SyncRequest {
        peer: PeerId,
        request: SyncRequest,
        response: request_response::ResponseChannel<Vec<u8>>,
    },
    SyncResponse {
        peer: PeerId,
        response: SyncResponse,
    },
}

#[derive(Debug)]
pub enum P2pOutbound {
    Block(Vec<u8>),
    Transaction(Vec<u8>),
    SyncRequest { peer: PeerId, request: SyncRequest },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncRequest {
    GetHeaders { start_height: u64, limit: u32 },
    GetBlocks { hashes: Vec<[u8; 32]> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncResponse {
    Headers { hashes: Vec<[u8; 32]> },
    Blocks { blocks: Vec<Vec<u8>> },
}

#[derive(Debug)]
pub struct P2pHandle {
    pub local_peer_id: PeerId,
    pub inbound: mpsc::Receiver<P2pInbound>,
    pub outbound: mpsc::Sender<P2pOutbound>,
    pub shutdown: mpsc::Sender<()>,
    pub task: tokio::task::JoinHandle<()>,
    pub sync_responses: mpsc::Sender<(request_response::ResponseChannel<Vec<u8>>, Vec<u8>)>,
    pub status: Arc<RwLock<P2pStatus>>,
}

#[derive(Clone, Debug)]
pub struct SyncProtocol;

impl AsRef<str> for SyncProtocol {
    fn as_ref(&self) -> &str {
        SYNC_PROTOCOL
    }
}

#[derive(Clone, Default)]
pub struct SyncCodec;

#[async_trait::async_trait]
impl request_response::Codec for SyncCodec {
    type Protocol = SyncProtocol;
    type Request = Vec<u8>;
    type Response = Vec<u8>;

    async fn read_request<T>(&mut self, _: &SyncProtocol, io: &mut T) -> io::Result<Vec<u8>>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame(io, MAX_SYNC_REQUEST).await
    }

    async fn read_response<T>(&mut self, _: &SyncProtocol, io: &mut T) -> io::Result<Vec<u8>>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_frame(io, MAX_SYNC_MESSAGE).await
    }

    async fn write_request<T>(
        &mut self,
        _: &SyncProtocol,
        io: &mut T,
        request: Vec<u8>,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame(io, &request, MAX_SYNC_REQUEST).await
    }

    async fn write_response<T>(
        &mut self,
        _: &SyncProtocol,
        io: &mut T,
        response: Vec<u8>,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_frame(io, &response, MAX_SYNC_MESSAGE).await
    }
}

async fn read_frame<T: AsyncRead + Unpin>(io: &mut T, max_size: usize) -> io::Result<Vec<u8>> {
    let mut header = [0u8; 4];
    io.read_exact(&mut header).await?;
    let length = u32::from_be_bytes(header) as usize;
    if length > max_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "sync frame too large",
        ));
    }
    let mut data = vec![0; length];
    io.read_exact(&mut data).await?;
    Ok(data)
}

async fn write_frame<T: AsyncWrite + Unpin>(
    io: &mut T,
    data: &[u8],
    max_size: usize,
) -> io::Result<()> {
    if data.len() > max_size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sync frame too large",
        ));
    }
    io.write_all(&(data.len() as u32).to_be_bytes()).await?;
    io.write_all(data).await?;
    io.flush().await
}

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ScytaleEvent")]
pub struct ScytaleBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub request_response: request_response::Behaviour<SyncCodec>,
    pub kad: kad::Behaviour<kad::store::MemoryStore>,
    pub ping: ping::Behaviour,
    pub identify: identify::Behaviour,
    pub connection_limits: connection_limits::Behaviour,
    pub autonat: autonat::Behaviour,
    pub relay: relay_client::Behaviour,
}

#[derive(Debug)]
pub enum ScytaleEvent {
    Gossipsub(gossipsub::Event),
    RequestResponse(request_response::Event<Vec<u8>, Vec<u8>>),
    Kad(kad::Event),
    Ping(ping::Event),
    Identify(identify::Event),
    Autonat(autonat::Event),
    Relay(relay_client::Event),
}

impl From<gossipsub::Event> for ScytaleEvent {
    fn from(event: gossipsub::Event) -> Self {
        Self::Gossipsub(event)
    }
}

impl From<ping::Event> for ScytaleEvent {
    fn from(event: ping::Event) -> Self {
        Self::Ping(event)
    }
}

impl From<request_response::Event<Vec<u8>, Vec<u8>>> for ScytaleEvent {
    fn from(event: request_response::Event<Vec<u8>, Vec<u8>>) -> Self {
        Self::RequestResponse(event)
    }
}

impl From<kad::Event> for ScytaleEvent {
    fn from(event: kad::Event) -> Self {
        Self::Kad(event)
    }
}

impl From<std::convert::Infallible> for ScytaleEvent {
    fn from(event: std::convert::Infallible) -> Self {
        match event {}
    }
}

impl From<identify::Event> for ScytaleEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl From<autonat::Event> for ScytaleEvent {
    fn from(event: autonat::Event) -> Self {
        Self::Autonat(event)
    }
}

impl From<relay_client::Event> for ScytaleEvent {
    fn from(event: relay_client::Event) -> Self {
        Self::Relay(event)
    }
}

fn load_known_peers(path: &Path) -> Result<Vec<KnownPeer>, P2pError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes = std::fs::read(path).map_err(P2pError::PeerStateIo)?;
    serde_json::from_slice(&bytes).map_err(|error| P2pError::PeerStateEncoding(error.to_string()))
}

fn save_known_peers(path: &Path, peers: &mut Vec<KnownPeer>) -> Result<(), P2pError> {
    peers.retain(|peer| peer.failures < MAX_PEER_FAILURES);
    peers.sort_by_key(|peer| (peer.failures, std::cmp::Reverse(peer.last_seen)));
    peers.truncate(MAX_KNOWN_PEERS);
    let parent = path
        .parent()
        .ok_or_else(|| P2pError::PeerStateIo(std::io::Error::other("peer state has no parent")))?;
    std::fs::create_dir_all(parent).map_err(P2pError::PeerStateIo)?;
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(peers)
        .map_err(|error| P2pError::PeerStateEncoding(error.to_string()))?;
    std::fs::write(&temporary, bytes).map_err(P2pError::PeerStateIo)?;
    std::fs::rename(temporary, path).map_err(P2pError::PeerStateIo)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

pub async fn start(
    p2p_port: u16,
    bootnodes: &[String],
    identity_path: &Path,
) -> Result<P2pHandle, P2pError> {
    let config = P2pConfig {
        listen_addr: format!("/ip4/0.0.0.0/tcp/{p2p_port}"),
        bootnodes: bootnodes.to_vec(),
        key_path: identity_path.to_path_buf(),
        known_peers_path: identity_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("known_peers.json"),
        best_height: 0,
    };
    start_with_config(config).await
}

pub async fn start_default() -> Result<P2pHandle, P2pError> {
    start_with_config(P2pConfig::default()).await
}

pub async fn start_with_config(config: P2pConfig) -> Result<P2pHandle, P2pError> {
    let keypair = load_identity(&config.key_path)?;
    let local_peer_id = PeerId::from(keypair.public());
    let public_key = keypair.public();

    let (relay_transport, relay_behaviour) = relay_client::new(local_peer_id);
    let relay_transport = relay_transport
        .upgrade(libp2p::core::upgrade::Version::V1Lazy)
        .authenticate(
            noise::Config::new(&keypair).map_err(|error| P2pError::Transport(error.to_string()))?,
        )
        .multiplex(yamux::Config::default())
        .map(|(peer_id, muxer), _| (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)));
    let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default());
    let dns_tcp_transport = libp2p::dns::tokio::Transport::system(tcp_transport)
        .map_err(|error| P2pError::Transport(error.to_string()))?;
    let base_transport = dns_tcp_transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(
            noise::Config::new(&keypair).map_err(|error| P2pError::Transport(error.to_string()))?,
        )
        .multiplex(yamux::Config::default())
        .map(|(peer_id, muxer), _| (peer_id, libp2p::core::muxing::StreamMuxerBox::new(muxer)));
    let transport = relay_transport
        .or_transport(base_transport)
        .map(|either, _| either.into_inner())
        .boxed();

    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .validation_mode(gossipsub::ValidationMode::Strict)
        .validate_messages()
        .max_transmit_size(MAX_BLOCK_PAYLOAD)
        .build()
        .map_err(|error| P2pError::Behaviour(error.to_string()))?;
    let mut gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(keypair.clone()),
        gossipsub_config,
    )
    .map_err(|error| P2pError::Behaviour(error.to_string()))?;
    gossipsub
        .with_peer_score(
            gossipsub::PeerScoreParams::default(),
            gossipsub::PeerScoreThresholds::default(),
        )
        .map_err(P2pError::Behaviour)?;
    let blocks_topic = gossipsub::IdentTopic::new(BLOCKS_TOPIC);
    let transactions_topic = gossipsub::IdentTopic::new(TRANSACTIONS_TOPIC);
    gossipsub
        .subscribe(&blocks_topic)
        .map_err(|error| P2pError::Behaviour(error.to_string()))?;
    gossipsub
        .subscribe(&transactions_topic)
        .map_err(|error| P2pError::Behaviour(error.to_string()))?;

    let request_response = request_response::Behaviour::with_codec(
        SyncCodec,
        [(SyncProtocol, request_response::ProtocolSupport::Full)],
        request_response::Config::default(),
    );
    let mut kad = kad::Behaviour::new(local_peer_id, kad::store::MemoryStore::new(local_peer_id));
    kad.set_mode(Some(kad::Mode::Server));

    let identify_agent = format!("scytale/0.3.0/height/{}", config.best_height);
    let behaviour = ScytaleBehaviour {
        gossipsub,
        request_response,
        kad,
        ping: ping::Behaviour::new(ping::Config::new()),
        identify: identify::Behaviour::new(
            identify::Config::new(P2P_PROTOCOL.to_owned(), public_key)
                .with_agent_version(identify_agent),
        ),
        connection_limits: connection_limits::Behaviour::new(
            connection_limits::ConnectionLimits::default()
                .with_max_established_incoming(Some(30))
                .with_max_established_outgoing(Some(20))
                .with_max_established_per_peer(Some(1)),
        ),
        autonat: autonat::Behaviour::new(local_peer_id, autonat::Config::default()),
        relay: relay_behaviour,
    };
    let mut swarm = Swarm::new(
        transport,
        behaviour,
        local_peer_id,
        libp2p::swarm::Config::with_tokio_executor(),
    );
    let listen_addr: Multiaddr = config
        .listen_addr
        .parse()
        .map_err(|_| P2pError::InvalidAddress(config.listen_addr.clone()))?;
    swarm
        .listen_on(listen_addr)
        .map_err(|error| P2pError::Transport(error.to_string()))?;

    let mut dialed_bootnode_peers = std::collections::HashSet::new();
    for bootnode in &config.bootnodes {
        let address: Multiaddr = match bootnode.parse() {
            Ok(addr) => addr,
            Err(error) => {
                tracing::warn!(%bootnode, %error, "Invalid bootnode multiaddr format; skipping");
                continue;
            }
        };

        // If the bootnode specifies a peer ID, do not dial self if this node is the bootnode
        if let Some(libp2p::multiaddr::Protocol::P2p(peer_id)) = address.iter().last() {
            if peer_id == local_peer_id {
                tracing::info!(%address, "Current node is this bootnode; skipping self-dial");
                continue;
            }
            swarm.behaviour_mut().kad.add_address(&peer_id, address.clone());
            if !dialed_bootnode_peers.insert(peer_id) {
                tracing::debug!(%address, "Alternative multiaddr for peer already queued; skipping duplicate dial");
                continue;
            }
        }

        if let Err(error) = swarm.dial(address) {
            tracing::warn!(%bootnode, %error, "Initial bootnode dial failed (will retry via discovery/DHT)");
        }
    }

    let (inbound_tx, inbound) = mpsc::channel(256);
    let (outbound, mut outbound_rx) = mpsc::channel(256);
    let (sync_response_tx, mut sync_response_rx) = mpsc::channel(64);
    let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
    let blocks_topic_for_task = blocks_topic.clone();
    let transactions_topic_for_task = transactions_topic.clone();
    let status = Arc::new(RwLock::new(P2pStatus::default()));
    let status_for_task = Arc::clone(&status);
    let known_peers_path = config.known_peers_path.clone();
    let mut known_peers = load_known_peers(&known_peers_path)?;
    let bootnode_addresses: Vec<Multiaddr> = config
        .bootnodes
        .iter()
        .filter_map(|address| address.parse().ok())
        .collect();
    for known_peer in &known_peers {
        if let Ok(address) = known_peer.address.parse::<Multiaddr>() {
            if let Err(error) = swarm.dial(address) {
                tracing::debug!(peer_id = %known_peer.peer_id, %error, "known peer dial failed");
            }
        }
    }

    let task = tokio::spawn(async move {
        let mut request_windows: HashMap<PeerId, VecDeque<Instant>> = HashMap::new();
        let mut invalid_gossip: HashMap<PeerId, u8> = HashMap::new();
        let mut connected_addresses: Vec<(PeerId, Multiaddr)> = Vec::new();
        let mut discovery_timer = tokio::time::interval(DISCOVERY_INTERVAL);
        loop {
            tokio::select! {
                event = swarm.select_next_some() => { match event {
                    SwarmEvent::Behaviour(ScytaleEvent::Gossipsub(gossipsub::Event::Message {
                        propagation_source,
                        message_id,
                        message,
                    })) => {
                        let is_block = message.topic == blocks_topic_for_task.hash();
                        let is_transaction = message.topic == transactions_topic_for_task.hash();
                        let valid = if is_block {
                            message.data.len() <= MAX_BLOCK_PAYLOAD
                                && Block::from_canonical_bytes(&message.data).is_ok()
                        } else if is_transaction {
                            message.data.len() <= MAX_TRANSACTION_PAYLOAD
                                && Transaction::from_canonical_bytes(&message.data).is_ok()
                        } else {
                            false
                        };
                        swarm.behaviour_mut().gossipsub.report_message_validation_result(
                            &message_id,
                            &propagation_source,
                            if valid {
                                gossipsub::MessageAcceptance::Accept
                            } else {
                                gossipsub::MessageAcceptance::Reject
                            },
                        );
                        if !valid {
                            let failures = invalid_gossip.entry(propagation_source).or_default();
                            *failures = failures.saturating_add(1);
                            if *failures >= INVALID_GOSSIP_MESSAGES_BEFORE_DISCONNECT {
                                let _ = swarm.disconnect_peer_id(propagation_source);
                            }
                            continue;
                        }
                        let result = if is_block {
                            inbound_tx.send(P2pInbound::Block(message.data)).await
                        } else if is_transaction {
                            inbound_tx.send(P2pInbound::Transaction(message.data)).await
                        } else {
                            Ok(())
                        };
                        if result.is_err() { break; }
                    }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        tracing::info!(%address, "libp2p listener active");
                        let mut status = status_for_task.write().unwrap();
                        if !status.listen_addrs.contains(&address) {
                            status.listen_addrs.push(address);
                        }
                    }
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        tracing::info!(%peer_id, "libp2p peer connected");
                        let remote_address = endpoint.get_remote_address().clone();
                        let is_bootnode = bootnode_addresses
                            .iter()
                            .any(|address| address == &remote_address);
                        let connected_only: Vec<_> = connected_addresses
                            .iter()
                            .map(|(_, address)| address.clone())
                            .collect();
                        if !is_bootnode
                            && !subnet_connection_allowed(&connected_only, &remote_address)
                        {
                            tracing::warn!(%peer_id, %remote_address, "connection rejected: subnet limit reached");
                            let _ = swarm.disconnect_peer_id(peer_id);
                            continue;
                        }
                        connected_addresses.push((peer_id, remote_address.clone()));
                        {
                            let mut status = status_for_task.write().unwrap();
                            if !status.connected_peer_ids.contains(&peer_id) {
                                status.connected_peer_ids.push(peer_id);
                            }
                            status.connected_peers = status.connected_peer_ids.len();
                        }
                        let address = remote_address;
                        if let Some(known) = known_peers.iter_mut().find(|known| known.peer_id == peer_id.to_string()) {
                            known.address = address.to_string();
                            known.failures = 0;
                            known.last_seen = now_secs();
                        } else {
                            known_peers.push(KnownPeer {
                                peer_id: peer_id.to_string(),
                                address: address.to_string(),
                                failures: 0,
                                last_seen: now_secs(),
                            });
                        }
                        if let Err(error) = save_known_peers(&known_peers_path, &mut known_peers) {
                            tracing::error!(%error, "failed to persist known peers");
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        tracing::info!(%peer_id, "libp2p peer disconnected");
                        connected_addresses.retain(|(connected_peer, _)| connected_peer != &peer_id);
                        let mut status = status_for_task.write().unwrap();
                        status.connected_peer_ids.retain(|connected| connected != &peer_id);
                        status.connected_peers = status.connected_peer_ids.len();
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        tracing::warn!(?peer_id, %error, "libp2p outbound connection failed");
                        if let Some(peer_id) = peer_id {
                            if let Some(known) = known_peers.iter_mut().find(|known| known.peer_id == peer_id.to_string()) {
                                known.failures = known.failures.saturating_add(1);
                            }
                            if let Err(error) = save_known_peers(&known_peers_path, &mut known_peers) {
                                tracing::error!(%error, "failed to persist known peers");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(ScytaleEvent::RequestResponse(
                        request_response::Event::Message { peer, message, .. }
                    )) => {
                        match message {
                            request_response::Message::Request { request, channel, .. } => {
                                match bincode::deserialize::<SyncRequest>(&request) {
                                    Ok(request) => {
                                        let now = Instant::now();
                                        let window = request_windows.entry(peer).or_default();
                                        window.retain(|timestamp| now.duration_since(*timestamp) < Duration::from_secs(1));
                                        let within_rate = window.len() < MAX_REQUESTS_PER_PEER_PER_SECOND;
                                        let within_batch = match &request {
                                            SyncRequest::GetBlocks { hashes } => hashes.len() <= MAX_GET_BLOCKS_HASHES,
                                            SyncRequest::GetHeaders { limit, .. } => *limit <= MAX_GET_BLOCKS_HASHES as u32,
                                        };
                                        if within_rate && within_batch {
                                            window.push_back(now);
                                            if inbound_tx.send(P2pInbound::SyncRequest {
                                                peer,
                                                request,
                                                response: channel,
                                            }).await.is_err() { break; }
                                        } else {
                                            let empty = match request {
                                                SyncRequest::GetHeaders { .. } => SyncResponse::Headers { hashes: Vec::new() },
                                                SyncRequest::GetBlocks { .. } => SyncResponse::Blocks { blocks: Vec::new() },
                                            };
                                            if let Ok(payload) = bincode::serialize(&empty) {
                                                let _ = swarm.behaviour_mut().request_response.send_response(channel, payload);
                                            }
                                        }
                                    }
                                    Err(error) => tracing::debug!(%peer, %error, "invalid sync request"),
                                }
                            }
                            request_response::Message::Response { response, .. } => {
                                match bincode::deserialize::<SyncResponse>(&response) {
                                    Ok(response) => {
                                        if inbound_tx.send(P2pInbound::SyncResponse { peer, response }).await.is_err() { break; }
                                    }
                                    Err(error) => tracing::debug!(%peer, %error, "invalid sync response"),
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(ScytaleEvent::Identify(
                        identify::Event::Received { peer_id, info, .. }
                    )) => {
                        for address in info.listen_addrs {
                            swarm.behaviour_mut().kad.add_address(&peer_id, address);
                        }
                        if let Some(remote_height) = info.agent_version
                            .split("/height/")
                            .nth(1)
                            .and_then(|height| height.parse::<u64>().ok())
                        {
                            if inbound_tx
                                .send(P2pInbound::PeerStatus {
                                    peer: peer_id,
                                    best_height: remote_height,
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    SwarmEvent::Behaviour(ScytaleEvent::Autonat(
                        autonat::Event::StatusChanged { old, new },
                    )) => {
                        tracing::info!(?old, ?new, "AutoNAT reachability status changed");
                    }
                    SwarmEvent::Behaviour(ScytaleEvent::Relay(event)) => {
                        tracing::debug!(?event, "circuit relay client event");
                    }
                    _ => {}
                } },
                outbound_message = outbound_rx.recv() => { match outbound_message {
                    Some(P2pOutbound::Block(data)) => {
                        if let Err(error) = swarm.behaviour_mut().gossipsub.publish(blocks_topic_for_task.clone(), data) {
                            tracing::warn!(%error, "failed to publish block over libp2p");
                        }
                    }
                    Some(P2pOutbound::Transaction(data)) => {
                        if let Err(error) = swarm.behaviour_mut().gossipsub.publish(transactions_topic_for_task.clone(), data) {
                            tracing::warn!(%error, "failed to publish transaction over libp2p");
                        }
                    }
                    Some(P2pOutbound::SyncRequest { peer, request }) => {
                        match bincode::serialize(&request) {
                            Ok(payload) => { swarm.behaviour_mut().request_response.send_request(&peer, payload); }
                            Err(error) => tracing::warn!(%error, "failed to encode sync request"),
                        }
                    }
                    None => break,
                } }
                Some((channel, response)) = sync_response_rx.recv() => {
                    if let Err(error) = swarm.behaviour_mut().request_response.send_response(channel, response) {
                        tracing::debug!(?error, "failed to send sync response");
                    }
                }
                _ = discovery_timer.tick() => {
                    if status_for_task.read().unwrap().connected_peers < MIN_DISCOVERY_PEERS {
                        if let Err(error) = swarm.behaviour_mut().kad.bootstrap() {
                            tracing::debug!(%error, "kademlia bootstrap unavailable");
                        }
                    }
                }
                Some(()) = shutdown_rx.recv() => break,
            }
        }
        if let Err(error) = save_known_peers(&known_peers_path, &mut known_peers) {
            tracing::error!(%error, "failed to persist known peers during shutdown");
        }
    });

    Ok(P2pHandle {
        local_peer_id,
        inbound,
        outbound,
        shutdown: shutdown_tx,
        task,
        sync_responses: sync_response_tx,
        status,
    })
}

fn load_identity(path: &Path) -> Result<identity::Keypair, P2pError> {
    if path.exists() {
        let bytes = std::fs::read(path)?;
        return identity::Keypair::from_protobuf_encoding(&bytes)
            .map_err(|error| P2pError::IdentityEncoding(error.to_string()));
    }
    let keypair = identity::Keypair::generate_ed25519();
    let bytes = keypair
        .to_protobuf_encoding()
        .map_err(|error| P2pError::IdentityEncoding(error.to_string()))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(temporary, path)?;
    Ok(keypair)
}

#[cfg(test)]
mod tests {
    use super::{
        load_identity, load_known_peers, save_known_peers, subnet_connection_allowed, KnownPeer,
        BLOCKS_TOPIC, TRANSACTIONS_TOPIC,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn topics_are_stable() {
        assert_eq!(BLOCKS_TOPIC, "scytale/v1/blocks");
        assert_eq!(TRANSACTIONS_TOPIC, "scytale/v1/txs");
    }

    #[test]
    fn identity_persists() {
        let path = std::env::temp_dir().join(format!(
            "scytale-libp2p-{}.key",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = load_identity(&path).unwrap();
        let second = load_identity(&path).unwrap();
        assert_eq!(first.public(), second.public());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn known_peer_state_round_trips_and_missing_state_is_empty() {
        let path = std::env::temp_dir().join(format!(
            "scytale-known-peers-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(load_known_peers(&path).unwrap().is_empty());
        let mut peers = vec![KnownPeer {
            peer_id: "peer-1".into(),
            address: "/ip4/127.0.0.1/tcp/9000".into(),
            failures: 0,
            last_seen: 1,
        }];
        save_known_peers(&path, &mut peers).unwrap();
        assert_eq!(load_known_peers(&path).unwrap().len(), 1);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn corrupt_known_peer_state_is_rejected() {
        let path = std::env::temp_dir().join(format!(
            "scytale-known-peers-corrupt-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, b"not-json").unwrap();
        assert!(load_known_peers(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_subnet_connection_limit() {
        std::env::set_var("SCYTALE_MAX_PEERS_PER_SUBNET", "2");
        let connected = vec![
            "/ip4/192.0.2.10/tcp/1000".parse().unwrap(),
            "/ip4/192.0.2.11/tcp/1001".parse().unwrap(),
        ];
        let third = "/ip4/192.0.2.12/tcp/1002".parse().unwrap();
        assert!(!subnet_connection_allowed(&connected, &third));
        let different = "/ip4/192.0.3.12/tcp/1002".parse().unwrap();
        assert!(subnet_connection_allowed(&connected, &different));

        // Private and loopback IPs are unrestricted
        let private1 = "/ip4/172.28.0.10/tcp/1000".parse().unwrap();
        let private2 = "/ip4/172.28.0.20/tcp/1000".parse().unwrap();
        let private3 = "/ip4/172.28.0.30/tcp/1000".parse().unwrap();
        let privates = vec![private1, private2];
        assert!(subnet_connection_allowed(&privates, &private3));

        let v6 = vec![
            "/ip6/2001:db8::1/tcp/1000".parse().unwrap(),
            "/ip6/2001:db8::2/tcp/1001".parse().unwrap(),
        ];
        let third_v6 = "/ip6/2001:db8::3/tcp/1002".parse().unwrap();
        assert!(!subnet_connection_allowed(&v6, &third_v6));
        std::env::remove_var("SCYTALE_MAX_PEERS_PER_SUBNET");
    }
}
