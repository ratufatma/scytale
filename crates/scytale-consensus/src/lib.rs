//! Scytale Consensus: Proof-of-Work, emission curve, and validation rules.

use scytale_core::{Hash, Quanta, QUANTA_PER_SCY};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("Invalid target/difficulty")]
    InvalidTarget,
    #[error("Block hash does not meet target")]
    BlockPoWInvalid,
    #[error("Invalid block reward")]
    InvalidReward,
}

pub const INITIAL_REWARD: Quanta = 10 * QUANTA_PER_SCY; // 10 SCY (1,000,000,000 quanta)
pub const HALVING_INTERVAL: u64 = 2_100_000;

/// Computes the block subsidy based on block height.
pub fn calculate_block_reward(height: u64) -> Quanta {
    let halvings = height / HALVING_INTERVAL;
    if halvings >= 64 {
        0
    } else {
        INITIAL_REWARD >> halvings
    }
}

/// Verifies that a given hash meets the target difficulty.
pub fn verify_pow(hash: &Hash, target: &[u8; 32]) -> bool {
    hash.as_bytes() <= target
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_block_reward() {
        assert_eq!(calculate_block_reward(0), 10 * QUANTA_PER_SCY);
        assert_eq!(calculate_block_reward(1), 10 * QUANTA_PER_SCY);
        assert_eq!(calculate_block_reward(HALVING_INTERVAL), 5 * QUANTA_PER_SCY);
        assert_eq!(calculate_block_reward(HALVING_INTERVAL * 2), 250_000_000);
    }
}
