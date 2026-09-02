use thiserror::Error;

/// Strongly-typed errors returned by the mining lifecycle.
#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum MiningError {
    #[error("canonical chain tip is missing or not yet initialized")]
    CanonicalTipMissing,
    #[error("stale candidate aborted: height {height} was superseded by an arriving block")]
    Cancelled { height: u64 },
    #[error(
        "nonce range exhausted without solution at height {height} after {searched} iterations"
    )]
    ExhaustedNonce { height: u64, searched: u64 },
    #[error("local pre-broadcast consensus validation failed: {0}")]
    LocalValidationFailed(String),
    #[error("mempool is unavailable for transaction selection")]
    MempoolUnavailable,
    #[error("arithmetic overflow during coinbase value calculation")]
    ArithmeticOverflow,
    #[error("worker panicked: {0}")]
    WorkerPanic(String),
}

/// Lifecycle state of the background mining worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MinerState {
    Stopped,
    Starting,
    Mining { height: u64 },
    Refreshing,
    Stopping,
    Failed(String),
}
