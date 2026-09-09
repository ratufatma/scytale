//! Scytale Node: CLI application logic and node daemon orchestration.

pub mod config;
pub mod error;
pub mod http_gateway;
pub mod indexer;
pub mod ipc;
pub mod network;
pub mod node;
pub mod passbook;

pub use config::NodeConfig;
pub use error::{NodeError, NodeState};
pub use http_gateway::{run_http_gateway, DEFAULT_HTTP_BIND};
pub use indexer::{start_indexer, BlockPayload, IndexerHandle};
pub use ipc::{IpcServer, DEFAULT_SOCKET_PATH};
pub use network::{
    decode, encode, validate_hello, BlockRequest, BlocksResponse, HeadersResponse, Heartbeat,
    Hello, LocatorRequest, P2pEngine, PeerRegistry, RateLimiter, BLOCKS_SUBJECT, HEARTBEAT_SUBJECT,
    HELLO_SUBJECT, MAX_SYNC_ITEMS, PROTOCOL_VERSION, SYNC_BLOCKS_SUBJECT, SYNC_HEADERS_SUBJECT,
    SYNC_LOCATOR_SUBJECT, TRANSACTIONS_SUBJECT,
};
pub use node::{commit_block, Node, PermissiveVerifier};
pub use passbook::{
    EntryStatus, EntryType, Passbook, PassbookAction, PassbookAsset, PassbookEntry, PassbookError,
    PassbookStatement, PassbookView, ProvenanceCategory, ProvenanceStep,
};
pub use scytale_bridge as bridge;
pub use scytale_consensus as consensus;
pub use scytale_core as core;
pub use scytale_mempool as mempool;
pub use scytale_mining as mining;
pub use scytale_primitives as primitives;
pub use scytale_storage as storage;
