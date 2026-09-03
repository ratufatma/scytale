use crate::entry::{MempoolEntry, PriorityKey};
use crate::error::MempoolError;
use scytale_core::{
    verify_transaction_authorization, AuthorizationVerifier, Block, Hash256, OutPoint, Transaction,
    UtxoEntry, UtxoSet,
};
use std::collections::{BTreeSet, HashMap, HashSet};

/// Default maximum number of pending transactions in the mempool.
pub const DEFAULT_MAX_MEMPOOL_COUNT: usize = 5_000;

/// Default maximum total canonical serialized bytes in the mempool (~5 MB).
pub const DEFAULT_MAX_MEMPOOL_BYTES: usize = 5_000_000;

/// Default minimum relay fee rate floor in milli-quanta per byte (setara 1 quantum/byte).
pub const DEFAULT_MIN_RELAY_FEE_RATE: u64 = 1_000;

/// In-memory state machine for local unconfirmed pending transactions,
/// prioritized by fee density with deterministic capacity enforcement and eviction.
#[derive(Debug, Clone)]
pub struct Mempool {
    /// Mapping TxID -> MempoolEntry
    entries: HashMap<Hash256, MempoolEntry>,
    /// Priority index ordered by PriorityKey (fee_rate DESC, added_time ASC, txid ASC)
    priority_index: BTreeSet<PriorityKey>,
    /// Index for in-flight double-spend prevention: OutPoint -> TxID consuming it
    spent_outpoints: HashMap<OutPoint, Hash256>,
    /// Dependency tracking: Parent TxID -> Set of Child TxIDs
    parent_to_children: HashMap<Hash256, HashSet<Hash256>>,
    /// Dependency tracking: Child TxID -> Set of Parent TxIDs
    child_to_parents: HashMap<Hash256, HashSet<Hash256>>,
    /// Total canonical serialized bytes of all entries in the pool
    total_bytes: usize,
    /// Maximum count of transactions allowed
    max_count: usize,
    /// Maximum total serialized bytes allowed
    max_bytes: usize,
    /// Minimum relay fee rate floor in milli-quanta per byte
    min_fee_rate: u64,
}

pub type PriorityMempool = Mempool;

impl Default for Mempool {
    fn default() -> Self {
        Self::new()
    }
}

impl Mempool {
    /// Creates a new Mempool with production default capacity boundaries.
    pub fn new() -> Self {
        Self::with_config(
            DEFAULT_MAX_MEMPOOL_COUNT,
            DEFAULT_MAX_MEMPOOL_BYTES,
            DEFAULT_MIN_RELAY_FEE_RATE,
        )
    }

    /// Creates a new Mempool with explicit capacity and fee floor parameters.
    pub fn with_config(max_count: usize, max_bytes: usize, min_fee_rate: u64) -> Self {
        Self {
            entries: HashMap::new(),
            priority_index: BTreeSet::new(),
            spent_outpoints: HashMap::new(),
            parent_to_children: HashMap::new(),
            child_to_parents: HashMap::new(),
            total_bytes: 0,
            max_count,
            max_bytes,
            min_fee_rate,
        }
    }

    /// Returns the number of pending transactions in the mempool.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the mempool contains no transactions.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the current aggregate serialized bytes of all transactions in the pool.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Returns the current aggregate fee of all transactions in the pool in quanta.
    pub fn total_fees(&self) -> u64 {
        self.entries.values().map(|e| e.fee).sum()
    }

    /// Returns the maximum allowed transaction count.
    pub fn max_count(&self) -> usize {
        self.max_count
    }

    /// Returns the maximum allowed total bytes.
    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    /// Returns the minimum relay fee rate floor (milli-quanta per byte).
    pub fn min_fee_rate(&self) -> u64 {
        self.min_fee_rate
    }

    /// Checks if a transaction with the given TxID is present in the pool.
    pub fn contains(&self, txid: &Hash256) -> bool {
        self.entries.contains_key(txid)
    }

    /// Retrieves a reference to a MempoolEntry by its TxID.
    pub fn get(&self, txid: &Hash256) -> Option<&MempoolEntry> {
        self.entries.get(txid)
    }

    /// Returns all mempool entries sorted descending by priority (fee-rate DESC, added_time ASC).
    pub fn get_entries_sorted_by_fee_rate(&self) -> Vec<MempoolEntry> {
        self.priority_index
            .iter()
            .rev()
            .filter_map(|key| self.entries.get(&key.txid).cloned())
            .collect()
    }

    /// Selects transactions for a block template up to `max_bytes`, prioritizing highest fee-rate
    /// while strictly respecting topological in-mempool parent-child dependencies.
    ///
    /// Returns `(selected_transactions, total_fees_quanta)`.
    pub fn select_transactions_for_block(&self, max_bytes: usize) -> (Vec<Transaction>, u64) {
        let mut selected_txs = Vec::new();
        let mut total_fees: u64 = 0;
        let mut current_bytes: usize = 0;
        let mut included_txids = HashSet::new();

        for key in self.priority_index.iter().rev() {
            if let Some(entry) = self.entries.get(&key.txid) {
                // If this transaction depends on mempool parents, ensure they are already included
                if let Some(parents) = self.child_to_parents.get(&key.txid) {
                    if !parents.iter().all(|p| included_txids.contains(p)) {
                        continue;
                    }
                }

                if current_bytes.saturating_add(entry.size_bytes) <= max_bytes {
                    current_bytes = current_bytes.saturating_add(entry.size_bytes);
                    total_fees = total_fees.saturating_add(entry.fee);
                    included_txids.insert(key.txid);
                    selected_txs.push(entry.transaction.clone());
                }
            }
        }

        (selected_txs, total_fees)
    }

    /// Directly inserts a validated `MempoolEntry`, enforcing minimum relay fee rate
    /// and dynamic capacity eviction.
    ///
    /// Returns `Ok(Some(evicted_txid))` if an entry was evicted to make room,
    /// or `Ok(None)` if inserted without eviction.
    pub fn insert(&mut self, entry: MempoolEntry) -> Result<Option<Hash256>, MempoolError> {
        if entry.fee_rate < self.min_fee_rate {
            return Err(MempoolError::FeeTooLow {
                fee_rate: entry.fee_rate,
                min_relay_fee: self.min_fee_rate,
            });
        }

        let mut evicted_txid = None;

        while self.entries.len() >= self.max_count
            || self.total_bytes.saturating_add(entry.size_bytes) > self.max_bytes
        {
            let lowest_key = match self.priority_index.iter().next() {
                Some(k) => k.clone(),
                None => break,
            };

            if entry.fee_rate > lowest_key.fee_rate {
                self.remove_transaction_and_descendants(&lowest_key.txid);
                evicted_txid = Some(lowest_key.txid);
            } else {
                return Err(MempoolError::MempoolFull {
                    fee_rate: entry.fee_rate,
                    lowest_fee_rate: lowest_key.fee_rate,
                });
            }
        }

        for input in &entry.transaction.inputs {
            self.spent_outpoints
                .insert(input.previous_output, entry.txid);
        }

        self.priority_index.insert(entry.priority_key());
        self.total_bytes = self.total_bytes.saturating_add(entry.size_bytes);
        self.entries.insert(entry.txid, entry);

        Ok(evicted_txid)
    }

    /// Admits a new transaction into the mempool through the verification pipeline:
    /// 1. Stateless structural validation
    /// 2. Deduplication check
    /// 3. In-flight double spend check against spent_outpoints
    /// 4. Input UTXO resolution (canonical UTXO set + pending mempool outputs)
    /// 5. Authorization verification
    /// 6. Value conservation & fee rate calculation
    /// 7. Minimum relay fee rate verification
    /// 8. Capacity boundary check & lowest-fee eviction
    /// 9. Insertion and dependency registration
    pub fn admit_transaction<V: AuthorizationVerifier>(
        &mut self,
        tx: Transaction,
        canonical_utxos: &UtxoSet,
        verifier: &V,
        current_timestamp: u64,
    ) -> Result<Hash256, MempoolError> {
        // 1. Stateless validation
        tx.validate_stateless().map_err(MempoolError::from)?;

        if tx.is_coinbase() {
            return Err(MempoolError::CoinbaseNotAllowed);
        }

        let txid = tx.txid();

        // 2. Duplicate check
        if self.entries.contains_key(&txid) {
            return Err(MempoolError::DuplicateTx(txid));
        }

        // 3. In-flight double spend check
        for input in &tx.inputs {
            if let Some(&conflicting_tx) = self.spent_outpoints.get(&input.previous_output) {
                return Err(MempoolError::ConflictDoubleSpend {
                    outpoint: input.previous_output,
                    conflicting_tx,
                });
            }
        }

        // 4. Resolve input UTXOs (canonical + unconfirmed mempool outputs)
        let mut utxo_entries = Vec::new();
        let mut parents = HashSet::new();

        for input in &tx.inputs {
            if let Some(canonical_entry) = canonical_utxos.get(&input.previous_output) {
                utxo_entries.push(canonical_entry.clone());
            } else if let Some(parent_entry) = self.entries.get(&input.previous_output.txid) {
                let out_idx = input.previous_output.index as usize;
                if let Some(parent_out) = parent_entry.transaction.outputs.get(out_idx) {
                    utxo_entries.push(UtxoEntry::new(parent_out.clone(), 0, false));
                    parents.insert(input.previous_output.txid);
                } else {
                    return Err(MempoolError::MissingInputUtxo(input.previous_output));
                }
            } else {
                return Err(MempoolError::MissingInputUtxo(input.previous_output));
            }
        }

        // 5. Authorization verification
        verify_transaction_authorization(&tx, &utxo_entries, verifier)
            .map_err(MempoolError::from)?;

        // 6. Value conservation & fee calculation
        let mut total_in: u64 = 0;
        for entry in &utxo_entries {
            total_in = total_in
                .checked_add(entry.output.value)
                .ok_or(MempoolError::ArithmeticOverflow)?;
        }

        let total_out = tx
            .total_output_quanta()
            .map_err(|e| MempoolError::StructuralError(e.to_string()))?;

        if total_in < total_out {
            return Err(MempoolError::ValueDeficit {
                total_in,
                total_out,
            });
        }

        let fee = total_in
            .checked_sub(total_out)
            .ok_or(MempoolError::ArithmeticOverflow)?;

        let entry = MempoolEntry::new(tx.clone(), fee, current_timestamp);

        // 7. Minimum relay fee rate verification
        if entry.fee_rate < self.min_fee_rate {
            return Err(MempoolError::FeeTooLow {
                fee_rate: entry.fee_rate,
                min_relay_fee: self.min_fee_rate,
            });
        }

        // 8. Capacity boundary check & lowest-fee eviction
        while self.entries.len() >= self.max_count
            || self.total_bytes.saturating_add(entry.size_bytes) > self.max_bytes
        {
            let lowest_key = match self.priority_index.iter().next() {
                Some(k) => k.clone(),
                None => break,
            };

            if entry.fee_rate > lowest_key.fee_rate {
                self.remove_transaction_and_descendants(&lowest_key.txid);
            } else {
                return Err(MempoolError::MempoolFull {
                    fee_rate: entry.fee_rate,
                    lowest_fee_rate: lowest_key.fee_rate,
                });
            }
        }

        // 9. Commit to Mempool
        for input in &tx.inputs {
            self.spent_outpoints.insert(input.previous_output, txid);
        }

        for parent_id in &parents {
            self.parent_to_children
                .entry(*parent_id)
                .or_default()
                .insert(txid);
        }

        if !parents.is_empty() {
            self.child_to_parents.insert(txid, parents);
        }

        self.priority_index.insert(entry.priority_key());
        self.total_bytes = self.total_bytes.saturating_add(entry.size_bytes);
        self.entries.insert(txid, entry);

        Ok(txid)
    }

    /// Removes a transaction and cascades removal to all its dependent child transactions.
    pub fn remove_transaction_and_descendants(&mut self, txid: &Hash256) -> Vec<Hash256> {
        let mut to_remove = Vec::new();
        let mut queue = vec![*txid];

        while let Some(current_id) = queue.pop() {
            if to_remove.contains(&current_id) {
                continue;
            }
            to_remove.push(current_id);
            if let Some(children) = self.parent_to_children.get(&current_id) {
                for child_id in children {
                    queue.push(*child_id);
                }
            }
        }

        for id in &to_remove {
            if let Some(entry) = self.entries.remove(id) {
                self.priority_index.remove(&entry.priority_key());
                self.total_bytes = self.total_bytes.saturating_sub(entry.size_bytes);
                for input in &entry.transaction.inputs {
                    self.spent_outpoints.remove(&input.previous_output);
                }
            }
            if let Some(parents) = self.child_to_parents.remove(id) {
                for parent_id in parents {
                    if let Some(children) = self.parent_to_children.get_mut(&parent_id) {
                        children.remove(id);
                    }
                }
            }
            self.parent_to_children.remove(id);
        }

        to_remove
    }

    /// Handles a new block connection:
    /// 1. Removes transactions confirmed in the block from the mempool.
    /// 2. Evicts remaining pending transactions whose inputs are no longer valid.
    pub fn on_block_connected(&mut self, block: &Block, updated_canonical_utxos: &UtxoSet) {
        // 1. Remove transactions that are confirmed in the block
        for tx in block.transactions.iter().skip(1) {
            let txid = tx.txid();
            if let Some(entry) = self.entries.remove(&txid) {
                self.priority_index.remove(&entry.priority_key());
                self.total_bytes = self.total_bytes.saturating_sub(entry.size_bytes);
                for input in &entry.transaction.inputs {
                    self.spent_outpoints.remove(&input.previous_output);
                }
            }
            if let Some(parents) = self.child_to_parents.remove(&txid) {
                for parent_id in parents {
                    if let Some(children) = self.parent_to_children.get_mut(&parent_id) {
                        children.remove(&txid);
                    }
                }
            }
        }

        // 2. Scan remaining pending transactions for broken/spent inputs
        let mut invalid_txs = Vec::new();
        for (txid, entry) in &self.entries {
            for input in &entry.transaction.inputs {
                let exists_in_canonical = updated_canonical_utxos.contains(&input.previous_output);
                let exists_in_mempool_parent = self
                    .entries
                    .get(&input.previous_output.txid)
                    .map(|p| (input.previous_output.index as usize) < p.transaction.outputs.len())
                    .unwrap_or(false);

                if !exists_in_canonical && !exists_in_mempool_parent {
                    invalid_txs.push(*txid);
                    break;
                }
            }
        }

        // 3. Evict invalid transactions and their descendants
        for txid in invalid_txs {
            self.remove_transaction_and_descendants(&txid);
        }
    }

    /// Re-admits valid transactions following a chain reorganization.
    pub fn on_reorg<V: AuthorizationVerifier>(
        &mut self,
        disconnected_txs: Vec<Transaction>,
        canonical_utxos: &UtxoSet,
        verifier: &V,
        current_timestamp: u64,
    ) {
        for tx in disconnected_txs {
            if tx.is_coinbase() {
                continue;
            }
            let _ = self.admit_transaction(tx, canonical_utxos, verifier, current_timestamp);
        }
    }
}
