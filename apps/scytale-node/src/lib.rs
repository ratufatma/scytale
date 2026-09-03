//! Scytale Node: CLI application logic and node daemon orchestration.

pub mod config;
pub mod error;
pub mod http_gateway;
pub mod ipc;
pub mod node;
pub mod p2p_supervisor;
pub mod passbook;

pub use config::NodeConfig;
pub use error::{NodeError, NodeState};
pub use http_gateway::{run_http_gateway, DEFAULT_HTTP_BIND};
pub use ipc::{IpcServer, DEFAULT_SOCKET_PATH};
pub use node::{Node, PermissiveVerifier};
pub use p2p_supervisor::P2pSupervisor;
pub use passbook::{
    EntryStatus, EntryType, Passbook, PassbookEntry, PassbookError, PassbookView,
    ProvenanceCategory, ProvenanceStep,
};
pub use scytale_bridge as bridge;
pub use scytale_consensus as consensus;
pub use scytale_core as core;
pub use scytale_mempool as mempool;
pub use scytale_mining as mining;
pub use scytale_primitives as primitives;
pub use scytale_storage as storage;
