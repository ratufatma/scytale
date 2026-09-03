use scytale_core::{CanonicalSerialize, Hash256, Transaction};
use serde::{Deserialize, Serialize};

/// Composite key for deterministic mempool priority ordering.
///
/// Priority criteria:
/// 1. `fee_rate` (DESC): higher fee density has higher priority.
/// 2. `added_time` (ASC): earlier arrival has higher priority via `std::cmp::Reverse`.
/// 3. `txid` (ASC): deterministic tie-breaker.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PriorityKey {
    pub fee_rate: u64,                      // milli-quanta per byte
    pub added_time: std::cmp::Reverse<u64>, // Earlier timestamp => greater priority key
    pub txid: Hash256,
}

impl PriorityKey {
    pub fn new(fee_rate: u64, added_time: u64, txid: Hash256) -> Self {
        Self {
            fee_rate,
            added_time: std::cmp::Reverse(added_time),
            txid,
        }
    }
}

/// Metadata for an unconfirmed transaction residing in the mempool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolEntry {
    pub transaction: Transaction,
    pub txid: Hash256,
    pub fee: u64,          // Total fee in quanta
    pub fee_rate: u64,     // Fee rate in milli-quanta per byte: (fee * 1000) / size_bytes
    pub size_bytes: usize, // Canonical binary serialized size
    pub added_time: u64,   // Unix timestamp seconds
}

impl MempoolEntry {
    /// Creates a new MempoolEntry, automatically computing canonical size, TxID, and integer fee-rate.
    pub fn new(transaction: Transaction, fee: u64, added_time: u64) -> Self {
        let txid = transaction.txid();
        let size_bytes = transaction
            .to_canonical_bytes()
            .map(|b| b.len())
            .unwrap_or(1)
            .max(1);
        let fee_rate = fee.saturating_mul(1000) / (size_bytes as u64);

        Self {
            transaction,
            txid,
            fee,
            fee_rate,
            size_bytes,
            added_time,
        }
    }

    /// Alias for fee in quanta.
    #[inline]
    pub fn fee_quanta(&self) -> u64 {
        self.fee
    }

    /// Returns the composite PriorityKey for this entry.
    pub fn priority_key(&self) -> PriorityKey {
        PriorityKey::new(self.fee_rate, self.added_time, self.txid)
    }
}
