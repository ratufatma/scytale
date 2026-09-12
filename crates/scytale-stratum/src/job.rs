use crate::header::RawBlockHeader120;
use scytale_core::Hash256;
use serde::{Deserialize, Serialize};

/// Stratum mining job template distributed to workers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumJob {
    pub job_id: String,
    pub prev_hash: Hash256,
    pub coinbase1: Vec<u8>,
    pub coinbase2: Vec<u8>,
    pub merkle_branches: Vec<Hash256>,
    pub version: u32,
    pub bits: u32,
    pub utxo_root: Hash256,
    pub curtime: u64,
    pub clean_jobs: bool,
}

impl StratumJob {
    pub fn new(
        job_id: impl Into<String>,
        prev_hash: Hash256,
        coinbase1: Vec<u8>,
        coinbase2: Vec<u8>,
        merkle_branches: Vec<Hash256>,
        version: u32,
        bits: u32,
        utxo_root: Hash256,
        curtime: u64,
        clean_jobs: bool,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            prev_hash,
            coinbase1,
            coinbase2,
            merkle_branches,
            version,
            bits,
            utxo_root,
            curtime,
            clean_jobs,
        }
    }

    /// Reconstructs the complete Merkle Root when a worker submits a share with its extranonces.
    pub fn build_merkle_root(&self, extranonce1: &[u8], extranonce2: &[u8]) -> Hash256 {
        // 1. Reconstruct raw coinbase byte payload
        let mut raw_coinbase = Vec::with_capacity(
            self.coinbase1.len() + extranonce1.len() + extranonce2.len() + self.coinbase2.len(),
        );
        raw_coinbase.extend_from_slice(&self.coinbase1);
        raw_coinbase.extend_from_slice(extranonce1);
        raw_coinbase.extend_from_slice(extranonce2);
        raw_coinbase.extend_from_slice(&self.coinbase2);

        // 2. Hash Coinbase via BLAKE3
        let mut current_hash = Hash256::hash(&raw_coinbase);

        // 3. Fold along the Merkle branch path (coinbase is the leftmost leaf at index 0)
        for branch in &self.merkle_branches {
            let mut hasher = blake3::Hasher::new();
            hasher.update(current_hash.as_bytes());
            if branch != &Hash256::ZERO {
                hasher.update(branch.as_bytes());
            }
            current_hash = Hash256::new(*hasher.finalize().as_bytes());
        }

        current_hash
    }

    /// Reconstructs the full 120-byte block header from worker submission parameters.
    pub fn build_header(
        &self,
        merkle_root: Hash256,
        timestamp: u64,
        nonce: u64,
    ) -> RawBlockHeader120 {
        RawBlockHeader120 {
            version: self.version,
            prev_hash: self.prev_hash,
            merkle_root,
            utxo_root: self.utxo_root,
            timestamp,
            bits: self.bits,
            nonce,
        }
    }

    /// Computes Merkle branches for index 0 (coinbase) from a list of non-coinbase transaction hashes.
    pub fn calculate_merkle_branches(other_tx_hashes: &[Hash256]) -> Vec<Hash256> {
        if other_tx_hashes.is_empty() {
            return Vec::new();
        }

        // Tree leaves include a placeholder for coinbase at index 0
        let mut leaves: Vec<Hash256> = Vec::with_capacity(other_tx_hashes.len() + 1);
        leaves.push(Hash256::ZERO); // Placeholder for coinbase
        leaves.extend_from_slice(other_tx_hashes);

        let mut branches = Vec::new();
        let mut current_level = leaves;

        while current_level.len() > 1 {
            // If odd number of elements, duplicate the last element
            if current_level.len() % 2 != 0 {
                let last = *current_level.last().unwrap();
                current_level.push(last);
            }

            // The sibling for index 0 is at index 1
            branches.push(current_level[1]);

            // Form next parent level
            let mut next_level = Vec::with_capacity(current_level.len() / 2);
            for chunk in current_level.chunks_exact(2) {
                let mut hasher = blake3::Hasher::new();
                hasher.update(chunk[0].as_bytes());
                hasher.update(chunk[1].as_bytes());
                next_level.push(Hash256::new(*hasher.finalize().as_bytes()));
            }
            current_level = next_level;
        }

        branches
    }
}
