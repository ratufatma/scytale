use scytale_core::{CanonicalSerialize, Hash256, Transaction};
use serde::{Deserialize, Serialize};

/// Metadata for an unconfirmed transaction residing in the mempool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolEntry {
    pub transaction: Transaction,
    pub txid: Hash256,
    pub fee: u64,          // Quanta
    pub fee_rate: u64,     // Quanta per byte
    pub size_bytes: usize, // Canonical binary serialized size
    pub added_time: u64,   // Unix timestamp seconds
}

impl MempoolEntry {
    /// Creates a new MempoolEntry, automatically computing canonical size, TxID, and fee-rate.
    pub fn new(transaction: Transaction, fee: u64, added_time: u64) -> Self {
        let txid = transaction.txid();
        let size_bytes = transaction
            .to_canonical_bytes()
            .map(|b| b.len())
            .unwrap_or(1)
            .max(1);
        let fee_rate = fee / (size_bytes as u64);

        Self {
            transaction,
            txid,
            fee,
            fee_rate,
            size_bytes,
            added_time,
        }
    }
}
