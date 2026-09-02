//! Node runtime configuration.

use std::path::PathBuf;

/// Runtime configuration for the Scytale node orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeConfig {
    /// Filesystem path where the embedded `redb` database is stored.
    pub data_dir: PathBuf,
    /// Network identifier used to guard against cross-network block acceptance.
    pub network_id: u32,
    /// Whether the background Proof-of-Work mining worker is enabled.
    pub mining_enabled: bool,
    /// Locking condition script embedded in the mining coinbase payout.
    pub miner_payout_script: Vec<u8>,
    /// Maximum duration (seconds) to await background worker termination during shutdown.
    pub shutdown_timeout_secs: u64,
    /// Genesis difficulty compact target baked into the chain head.
    pub genesis_difficulty_target: u32,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            network_id: 0x5343_5901,
            mining_enabled: false,
            miner_payout_script: vec![0x01, 0x02, 0x03],
            shutdown_timeout_secs: 10,
            genesis_difficulty_target: 0x1d00_ffff,
        }
    }
}

impl NodeConfig {
    /// Convenience constructor for ephemeral, in-memory test nodes.
    pub fn in_memory() -> Self {
        Self {
            data_dir: PathBuf::from(":memory:"),
            ..Self::default()
        }
    }
}
