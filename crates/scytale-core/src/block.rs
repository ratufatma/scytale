use crate::codec::CanonicalSerialize;
use crate::error::BlockError;
use crate::transaction::Transaction;
use scytale_primitives::Hash256;
use serde::{Deserialize, Serialize};

/// BlockHeader: Fixed-size header containing Proof-of-Work and state commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub previous_block_hash: Hash256,
    pub transaction_commitment: Hash256,
    pub utxo_root: Hash256,
    pub timestamp: u64,
    pub difficulty_target: u32, // Compact target bits
    pub nonce: u64,
}

impl BlockHeader {
    pub fn new(
        version: u32,
        previous_block_hash: Hash256,
        transaction_commitment: Hash256,
        utxo_root: Hash256,
        timestamp: u64,
        difficulty_target: u32,
        nonce: u64,
    ) -> Self {
        Self {
            version,
            previous_block_hash,
            transaction_commitment,
            utxo_root,
            timestamp,
            difficulty_target,
            nonce,
        }
    }

    /// Computes the 32-byte BLAKE3 header hash.
    pub fn hash(&self) -> Hash256 {
        let bytes = self
            .to_canonical_bytes()
            .expect("header canonical serialization cannot fail");
        Hash256::hash(&bytes)
    }
}

/// Block: Container comprising a BlockHeader and its confirmed transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    pub fn new(header: BlockHeader, transactions: Vec<Transaction>) -> Self {
        Self {
            header,
            transactions,
        }
    }

    /// Performs stateless structural validation of the block.
    pub fn validate_structure(&self) -> Result<(), BlockError> {
        if self.transactions.is_empty() {
            return Err(BlockError::EmptyTransactionVector);
        }
        if !self.transactions[0].is_coinbase() {
            return Err(BlockError::MissingCoinbase);
        }
        for (idx, tx) in self.transactions.iter().enumerate().skip(1) {
            if tx.is_coinbase() {
                return Err(BlockError::DuplicateCoinbase(idx));
            }
            tx.validate_stateless()?;
        }
        Ok(())
    }
}
