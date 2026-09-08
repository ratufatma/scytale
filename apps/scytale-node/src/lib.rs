//! Scytale Node: CLI application logic and node daemon orchestration.

pub mod config;
pub mod error;
pub mod http_gateway;
pub mod indexer;
pub mod ipc;
pub mod node;
pub mod network;
pub mod passbook;
pub mod p2p_supervisor;

pub use config::NodeConfig;
pub use error::{NodeError, NodeState};
pub use http_gateway::{run_http_gateway, DEFAULT_HTTP_BIND};
pub use indexer::{start_indexer, BlockPayload, IndexerHandle};
pub use ipc::{IpcServer, DEFAULT_SOCKET_PATH};
pub use node::{commit_block, Node, PermissiveVerifier};
pub use network::P2pEngine;
pub use p2p_supervisor::{is_self_address, P2pSupervisor};
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
