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

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("Proof of work error: {0}")]
    Pow(#[from] PowError),
    #[error("Invalid target/difficulty")]
    InvalidTarget,
    #[error("Block hash does not meet target")]
    BlockPoWInvalid,
    #[error("Invalid block reward")]
    InvalidReward,
}
