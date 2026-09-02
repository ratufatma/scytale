use scytale_primitives::{OutPoint, TxOut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    use scytale_primitives::Hash256;

    #[test]
    fn test_utxo_set_mutations() {
        let mut set = UtxoSet::new();
        let op = OutPoint::new(Hash256::hash(b"tx_example"), 0);
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
