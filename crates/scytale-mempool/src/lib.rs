//! Scytale Mempool: In-memory transaction pool and prioritization.

use scytale_core::{Hash, Transaction};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum MempoolError {
    #[error("Transaction already exists in pool: {0:?}")]
    DuplicateTx(Hash),
    #[error("Mempool capacity exceeded")]
    CapacityExceeded,
}

pub struct Mempool {
    transactions: HashMap<Hash, Transaction>,
    capacity: usize,
}

impl Mempool {
    pub fn new(capacity: usize) -> Self {
        Self {
            transactions: HashMap::new(),
            capacity,
        }
    }

    pub fn insert(&mut self, tx_hash: Hash, tx: Transaction) -> Result<(), MempoolError> {
        if self.transactions.contains_key(&tx_hash) {
            return Err(MempoolError::DuplicateTx(tx_hash));
        }
        if self.transactions.len() >= self.capacity {
            return Err(MempoolError::CapacityExceeded);
        }
        self.transactions.insert(tx_hash, tx);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    pub fn remove(&mut self, tx_hash: &Hash) -> Option<Transaction> {
        self.transactions.remove(tx_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mempool_basic() {
        let mut pool = Mempool::new(10);
        let tx = Transaction::new(1, vec![], vec![], 0);
        let h1 = Hash::hash(b"tx_1");
        assert!(pool.insert(h1, tx.clone()).is_ok());
        assert_eq!(pool.len(), 1);
        assert!(matches!(
            pool.insert(h1, tx),
            Err(MempoolError::DuplicateTx(_))
        ));
    }
}
