//! Scytale Consensus: Proof-of-Work, emission curve, and validation rules.

pub mod error;
pub mod pow;
pub mod target;

pub use error::{ConsensusError, PowError};
pub use pow::{compute_pow_hash, mine_test_header, verify_pow};
pub use scytale_core::{Quanta, QUANTA_PER_SCY};
pub use target::Target;

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
