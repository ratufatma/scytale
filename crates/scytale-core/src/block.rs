use crate::transaction::Transaction;
use scytale_primitives::Hash256;
use serde::{Deserialize, Serialize};

/// BlockHeader: Fixed-size header containing Proof-of-Work and state commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub previous_block_hash: Hash256,
    pub transaction_commitment: Hash256,
    pub timestamp: u64,
    pub difficulty_target: [u8; 32],
    pub nonce: u64,
}

/// Block: Container comprising a BlockHeader and its confirmed transactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}
