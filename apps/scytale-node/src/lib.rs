//! Scytale Node: CLI application logic and node daemon orchestration.

pub mod config;
pub mod error;
pub mod node;

pub use config::NodeConfig;
pub use error::{NodeError, NodeState};
pub use node::{Node, PermissiveVerifier};

pub use scytale_bridge as bridge;
pub use scytale_consensus as consensus;
pub use scytale_core as core;
pub use scytale_mempool as mempool;
pub use scytale_mining as mining;
pub use scytale_primitives as primitives;
pub use scytale_storage as storage;
