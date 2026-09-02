//! Scytale Core: Transaction, Block, and UTXO Set definitions.

pub use scytale_primitives::{Hash, OutPoint, PrimitiveError, Quanta, TxOut, QUANTA_PER_SCY};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Primitive error: {0}")]
    Primitive(#[from] PrimitiveError),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(String),
}

/// TxIn: References an unspent output and provides cryptographic authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxIn {
    pub previous_output: OutPoint,
    pub authorization: Vec<u8>,
}

impl TxIn {
    pub fn new(previous_output: OutPoint, authorization: Vec<u8>) -> Self {
        Self {
            previous_output,
            authorization,
        }
    }
}

/// Transaction: Represents an atomic state transition in Scytale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub version: u32,
    pub inputs: Vec<TxIn>,
    pub outputs: Vec<TxOut>,
    pub lock_time: u64,
}

impl Transaction {
    pub fn new(version: u32, inputs: Vec<TxIn>, outputs: Vec<TxOut>, lock_time: u64) -> Self {
        Self {
            version,
            inputs,
            outputs,
            lock_time,
        }
    }

    pub fn is_coinbase(&self) -> bool {
        self.inputs.is_empty()
    }
}

/// BlockHeader: Fixed-size header containing Proof-of-Work and state commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub previous_block_hash: Hash,
    pub transaction_commitment: Hash,
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

/// UtxoEntry: Represents an unspent transaction output with block height metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtxoEntry {
    pub output: TxOut,
    pub block_height: u64,
    pub is_coinbase: bool,
}

/// In-memory UTXO Set tracker.
#[derive(Debug, Clone, Default)]
pub struct UtxoSet {
    entries: HashMap<OutPoint, UtxoEntry>,
}

impl UtxoSet {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn insert(&mut self, outpoint: OutPoint, entry: UtxoEntry) {
        self.entries.insert(outpoint, entry);
    }

    pub fn get(&self, outpoint: &OutPoint) -> Option<&UtxoEntry> {
        self.entries.get(outpoint)
    }

    pub fn remove(&mut self, outpoint: &OutPoint) -> Option<UtxoEntry> {
        self.entries.remove(outpoint)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coinbase_transaction_detection() {
        let coinbase_tx = Transaction::new(1, vec![], vec![TxOut::new(1_000_000_000, vec![])], 0);
        assert!(coinbase_tx.is_coinbase());

        let regular_tx = Transaction::new(
            1,
            vec![TxIn::new(OutPoint::new(Hash::ZERO, 0), vec![])],
            vec![TxOut::new(500_000_000, vec![])],
            0,
        );
        assert!(!regular_tx.is_coinbase());
    }

    #[test]
    fn test_utxo_set_mutations() {
        let mut set = UtxoSet::new();
        let op = OutPoint::new(Hash::hash(b"tx_example"), 0);
        let entry = UtxoEntry {
            output: TxOut::new(100_000_000, vec![]),
            block_height: 1,
            is_coinbase: false,
        };

        set.insert(op, entry.clone());
        assert_eq!(set.len(), 1);
        assert_eq!(set.get(&op), Some(&entry));

        let removed = set.remove(&op);
        assert_eq!(removed, Some(entry));
        assert!(set.is_empty());
    }
}
