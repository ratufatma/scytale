use crate::target::Target;
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

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("Proof of work error: {0}")]
    Pow(#[from] PowError),
    #[error("Difficulty error: {0}")]
    Difficulty(#[from] DifficultyError),
    #[error("Invalid target/difficulty")]
    InvalidTarget,
    #[error("Block hash does not meet target")]
    BlockPoWInvalid,
    #[error("Invalid block reward")]
    InvalidReward,
}
