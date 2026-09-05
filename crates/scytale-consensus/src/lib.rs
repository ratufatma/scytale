//! Scytale Consensus: Proof-of-Work, emission curve, and validation rules.

pub mod chain;
pub mod difficulty;
pub mod error;
pub mod pow;
pub mod target;
pub mod work;

pub use chain::{BlockNode, ChainTree, ReorgResult};
pub use difficulty::{
    calculate_next_target, scale_target_by_ratio, validate_block_target, DifficultyConfig,
    CLAMPING_FACTOR, DEFAULT_DIFFICULTY_EPOCH_BLOCKS, TARGET_BLOCK_INTERVAL_SECS,
};
pub use error::{ChainError, ConsensusError, DifficultyError, PowError};
pub use pow::{compute_pow_hash, mine_test_header, verify_pow};
pub use scytale_core::{Quanta, QUANTA_PER_SCY};
pub use target::Target;
pub use work::{block_work, CumulativeWork};

pub const INITIAL_REWARD: Quanta = 10 * QUANTA_PER_SCY; // 10 SCY (1,000,000,000 quanta)
pub const HALVING_INTERVAL: u64 = 2_100_000;

/// Public Proof-of-Work Mining Reserve: 69% of 42,000,000 SCY (28,980,000 SCY / 2,898,000,000,000,000 quanta).
pub const MINING_RESERVE_QUANTA: Quanta = 2_898_000_000_000_000;

/// Terminal block height where the mining emission reserve is fully exhausted:
/// - Epoch 0 (height 0..2,099,999): 2,100,000 blocks * 10 SCY = 21,000,000 SCY
/// - Epoch 1 (height 2,100,000..3,695,999): 1,596,000 blocks * 5 SCY = 7,980,000 SCY
///
/// Total: 28,980,000 SCY (2,898,000,000,000,000 quanta).
/// Blocks with height >= MINING_REWARD_END_HEIGHT emit 0 subsidy.
pub const MINING_REWARD_END_HEIGHT: u64 = 3_696_000;

/// Computes the block subsidy based on block height.
/// Enforces deterministic reward cessation at height 3,696,000 to maintain the 42M SCY hard cap.
pub fn calculate_block_reward(height: u64) -> Quanta {
    if height >= MINING_REWARD_END_HEIGHT {
        return 0;
    }
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
        assert_eq!(calculate_block_reward(MINING_REWARD_END_HEIGHT - 1), 5 * QUANTA_PER_SCY);
        assert_eq!(calculate_block_reward(MINING_REWARD_END_HEIGHT), 0);
        assert_eq!(calculate_block_reward(MINING_REWARD_END_HEIGHT + 100), 0);
    }

    #[test]
    fn test_exact_mining_reserve_issuance() {
        let epoch_0_blocks = HALVING_INTERVAL;
        let epoch_0_quanta = epoch_0_blocks * (10 * QUANTA_PER_SCY);

        let epoch_1_blocks = MINING_REWARD_END_HEIGHT - HALVING_INTERVAL;
        let epoch_1_quanta = epoch_1_blocks * (5 * QUANTA_PER_SCY);

        assert_eq!(epoch_0_quanta + epoch_1_quanta, MINING_RESERVE_QUANTA);
        assert_eq!(MINING_RESERVE_QUANTA, 28_980_000 * QUANTA_PER_SCY);
    }
}

