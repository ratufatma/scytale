//! Scytale Mempool: In-memory transaction pool and prioritization.

use std::collections::HashMap;
use scytale_core::Hash;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MempoolError {
    #[error("Transaction already exists in pool: {0:?}")]
    DuplicateTx(Hash),
    #[error("Mempool capacity exceeded")]
    CapacityExceeded,
}

pub struct Mempool {
    transactions: HashMap<Hash, Vec<u8>>,
    capacity: usize,
}

impl Mempool {
    pub fn new(capacity: usize) -> Self {
        Self {
            transactions: HashMap::new(),
            capacity,
        }
    }

    pub fn insert(&mut self, tx_hash: Hash, raw_tx: Vec<u8>) -> Result<(), MempoolError> {
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::DuplicateTx(tx_hash));
        }
        if self.transactions.len() >= self.capacity {
            return Err(MempoolError::CapacityExceeded);
        }
        self.transactions.insert(tx_hash, raw_tx);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mempool_basic() {
        let mut pool = Mempool::new(10);
        let h1 = Hash::hash(b"tx_1");
        assert!(pool.insert(h1, vec![1, 2, 3]).is_ok());
        assert_eq!(pool.len(), 1);
        assert!(matches!(pool.insert(h1, vec![1, 2, 3]), Err(MempoolError::DuplicateTx(_))));
    }
}
