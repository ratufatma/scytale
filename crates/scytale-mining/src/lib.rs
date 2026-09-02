//! Scytale Mining: Block candidate builder and Proof-of-Work hashing worker.

use scytale_consensus::{verify_pow, Target};
use scytale_core::{BlockHeader, Hash, Transaction};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MiningError {
    #[error("Mining cancelled by stale state or interrupt")]
    Cancelled,
    #[error("No valid candidate block template")]
    NoTemplate,
}

pub struct BlockTemplate {
    pub previous_block_hash: Hash,
    pub height: u64,
    pub transactions: Vec<Transaction>,
    pub difficulty_target: [u8; 32],
}

impl BlockTemplate {
    pub fn new(
        previous_block_hash: Hash,
        height: u64,
        transactions: Vec<Transaction>,
        difficulty_target: [u8; 32],
    ) -> Self {
        Self {
            previous_block_hash,
            height,
            transactions,
            difficulty_target,
        }
    }
}

pub struct Miner;

impl Miner {
    /// Attempts to solve the Proof-of-Work puzzle for a given candidate block.
    pub fn mine_single_step(
        header: &mut BlockHeader,
        target: &[u8; 32],
        max_iterations: u64,
    ) -> Option<Hash> {
        let target_obj = Target::from_be_bytes(*target);
        for _ in 0..max_iterations {
            if verify_pow(header, &target_obj).is_ok() {
                return Some(header.hash());
            }
            header.nonce = header.nonce.wrapping_add(1);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_miner_finds_easy_target() {
        let mut header = BlockHeader {
            version: 1,
            previous_block_hash: Hash::ZERO,
            transaction_commitment: Hash::ZERO,
            timestamp: 1700000000,
            difficulty_target: 0x1f00ffff,
            nonce: 0,
        };
        let easy_target = [0xff; 32];
        let solution = Miner::mine_single_step(&mut header, &easy_target, 100);
        assert!(solution.is_some());
    }
}
