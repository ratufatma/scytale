use crate::target::Target;
use crate::work::CumulativeWork;
use scytale_primitives::Hash256;
use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum PowError {
    #[error("insufficient proof of work: hash {hash:?} exceeds target {target:?}")]
    InsufficientWork { hash: Hash256, target: Target },
    #[error("invalid target threshold: {0}")]
    InvalidTarget(String),
    #[error("block header error: {0}")]
    HeaderError(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum DifficultyError {
    #[error("target mismatch: expected {expected:?}, got {actual:?}")]
    TargetMismatch { expected: Target, actual: Target },
    #[error("invalid epoch window: start height {start_height} >= end height {end_height}")]
    InvalidEpochWindow { start_height: u64, end_height: u64 },
    #[error("negative or zero observed time: start_time={start_time}, end_time={end_time}")]
    InvalidObservedTime { start_time: u64, end_time: u64 },
    #[error("arithmetic overflow occurred during retargeting calculation")]
    ArithmeticOverflow,
}

#[derive(Debug, PartialEq, Eq, Clone, Error)]
pub enum ChainError {
    #[error("invalid block in branch: hash {hash:?}, reason: {reason}")]
    InvalidBranchBlock { hash: Hash256, reason: String },
    #[error("common ancestor not found between tip A {tip_a:?} and tip B {tip_b:?}")]
    CommonAncestorNotFound { tip_a: Hash256, tip_b: Hash256 },
    #[error(
        "insufficient cumulative work: candidate work {candidate:?} <= active tip work {active:?}"
    )]
    InsufficientWork {
        candidate: CumulativeWork,
        active: CumulativeWork,
    },
    #[error("reorganization execution failed at block {hash:?}: {error}")]
    ReorgFailed { hash: Hash256, error: String },
    #[error("chain reorganization depth exceeded: attempted {depth} blocks, maximum allowed is {max}")]
    ReorgDepthExceeded { depth: u64, max: u64 },
    #[error("corrupted block linkage: parent hash {parent:?} not found")]
    CorruptedLinkage { parent: Hash256 },
    #[error("arithmetic overflow calculating cumulative work")]
    WorkOverflow,
    #[error("block structural validation failed: {0}")]
    BlockError(#[from] scytale_core::BlockError),
    #[error("utxo error: {0}")]
    UtxoError(#[from] scytale_core::UtxoError),
}

#[derive(Debug, Clone, Error)]
pub enum ConsensusError {
    #[error("Proof of work error: {0}")]
    Pow(#[from] PowError),
    #[error("Difficulty error: {0}")]
    Difficulty(#[from] DifficultyError),
    #[error("Chain error: {0}")]
    Chain(#[from] ChainError),
    #[error("Invalid target/difficulty")]
    InvalidTarget,
    #[error("Block hash does not meet target")]
    BlockPoWInvalid,
    #[error("Invalid block reward")]
    InvalidReward,
    #[error("Transaction verification failed: {0}")]
    TransactionVerification(String),
}

