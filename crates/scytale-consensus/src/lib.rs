//! Scytale Consensus: Proof-of-Work, emission curve, and validation rules.

pub mod chain;
pub mod difficulty;
pub mod error;
pub mod pow;
pub mod target;
pub mod work;

pub use chain::{
    calculate_median_time_past, transaction_commitment, validate_block_authoritative,
    validate_block_authoritative_with_headers, AuthoritativeTransactionVerifier, BlockNode,
    BlockTransactionVerifier, ChainTree, ReorgResult, DEFAULT_MAX_REORG_DEPTH,
};

pub use difficulty::{
    calculate_next_target, scale_target_by_ratio, validate_block_target, DifficultyConfig,
    CLAMPING_FACTOR, DEFAULT_DIFFICULTY_EPOCH_BLOCKS, TARGET_BLOCK_INTERVAL_SECS,
};
pub use error::{ChainError, ConsensusError, DifficultyError, PowError};
pub use pow::{compute_pow_hash, mine_test_header, verify_pow};
pub use scytale_core::{Quanta, QUANTA_PER_SCY};
pub use target::Target;
pub use work::{block_work, CumulativeWork};

pub const COIN: u64 = QUANTA_PER_SCY;
pub const MAX_SUPPLY: u64 = 66_000_000 * COIN;
pub const INITIAL_SUBSIDY: Quanta = 25 * COIN;
/// Backwards-compatible name used by the node CLI for the initial subsidy.
pub const INITIAL_REWARD: Quanta = INITIAL_SUBSIDY;
pub const HALVING_INTERVAL: u64 = 660_000;

pub fn get_block_subsidy(height: u64) -> Quanta {
    let halvings = height / HALVING_INTERVAL;
    if halvings >= 64 {
        return 0;
    }
    INITIAL_SUBSIDY >> halvings
}

pub fn calculate_block_reward(height: u64) -> Quanta {
    get_block_subsidy(height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_block_reward() {
        assert_eq!(get_block_subsidy(0), INITIAL_SUBSIDY);
        assert_eq!(get_block_subsidy(1), INITIAL_SUBSIDY);
        assert_eq!(get_block_subsidy(HALVING_INTERVAL - 1), INITIAL_SUBSIDY);
        assert_eq!(get_block_subsidy(HALVING_INTERVAL), INITIAL_SUBSIDY >> 1);
        assert_eq!(
            get_block_subsidy(HALVING_INTERVAL * 2),
            INITIAL_SUBSIDY >> 2
        );
        assert_eq!(get_block_subsidy(63 * HALVING_INTERVAL), 0);
    }
}
