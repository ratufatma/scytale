//! Node lifecycle state machine and error taxonomy.

/// Discrete states of the node runtime state machine.
///
/// The node progresses deterministically through these states during startup
/// and shutdown: `Starting -> Initializing -> Recovering -> Syncing -> Ready
/// -> Running -> Stopping -> Stopped`, or `Failed` on a fatal error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    /// CLI entry point received, node about to acquire resources.
    Starting,
    /// Runtime config loaded; embedded storage being opened.
    Initializing,
    /// Persistent canonical state and active UTXO set being recovered.
    Recovering,
    /// P2P sync / Initial Block Download in progress.
    Syncing,
    /// All subsystems healthy and synchronized.
    Ready,
    /// Steady-state event loop active (mining if enabled).
    Running,
    /// Shutdown signal received; background workers being cancelled.
    Stopping,
    /// Clean shutdown complete; all resources released.
    Stopped,
    /// Fatal error aborted the node.
    Failed(String),
}

impl NodeState {
    /// Returns `true` when the node is accepting new external transactions.
    pub fn accepting_transactions(&self) -> bool {
        matches!(self, NodeState::Ready | NodeState::Running)
    }
}

/// Strongly-typed errors returned by the node runtime orchestrator.
#[allow(clippy::result_large_err)]
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("storage initialization error: {0}")]
    Storage(#[from] scytale_storage::StorageError),
    #[error("consensus error: {0}")]
    Consensus(#[from] scytale_consensus::ChainError),
    #[error("mempool error: {0}")]
    Mempool(#[from] scytale_mempool::MempoolError),
    #[error("mining error: {0}")]
    Mining(#[from] scytale_mining::MiningError),
    #[error("node shutdown timeout exceeded")]
    ShutdownTimeout,
    #[error("background mining worker was not running")]
    MiningNotRunning,
    #[error("chain state inconsistent: {0}")]
    InconsistentState(String),
    #[error("P2P bridge startup failed: {0}")]
    P2PStartupFailed(String),
}
