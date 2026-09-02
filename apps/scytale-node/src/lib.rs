//! Scytale Node: CLI application logic and node daemon orchestration.

pub use scytale_consensus as consensus;
pub use scytale_core as core;
pub use scytale_mempool as mempool;
pub use scytale_network as network;
pub use scytale_storage as storage;

#[derive(Debug, Clone)]
pub struct NodeConfig {
    pub data_dir: String,
    pub p2p_port: u16,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            data_dir: ".scytale".to_string(),
            p2p_port: 8333,
        }
    }
}
