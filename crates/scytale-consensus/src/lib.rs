//! Scytale Consensus: Proof-of-Work, emission curve, and validation rules.

use scytale_core::Hash;
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

pub const INITIAL_REWARD: u64 = 50 * 100_000_000; // 50 coins (with 8 decimals)
pub const HALVING_INTERVAL: u64 = 210_000;

/// Computes the block subsidy based on block height.
pub fn calculate_block_reward(height: u64) -> u64 {
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
        assert_eq!(calculate_block_reward(0), 50 * 100_000_000);
        assert_eq!(calculate_block_reward(1), 50 * 100_000_000);
        assert_eq!(calculate_block_reward(HALVING_INTERVAL), 25 * 100_000_000);
    }
}
