use crate::entry::MempoolEntry;
use crate::error::MempoolError;
use scytale_core::{
    verify_transaction_authorization, AuthorizationVerifier, Block, Hash256, OutPoint, Transaction,
    UtxoEntry, UtxoSet,
};
use std::collections::{HashMap, HashSet};

/// In-memory state machine for local unconfirmed pending transactions.
#[derive(Debug, Clone, Default)]
pub struct Mempool {
    /// Mapping TxID -> MempoolEntry
    entries: HashMap<Hash256, MempoolEntry>,
    /// Index for in-flight double-spend prevention: OutPoint -> TxID consuming it
    spent_outpoints: HashMap<OutPoint, Hash256>,
    /// Dependency tracking: Parent TxID -> Set of Child TxIDs
    parent_to_children: HashMap<Hash256, HashSet<Hash256>>,
    /// Dependency tracking: Child TxID -> Set of Parent TxIDs
    child_to_parents: HashMap<Hash256, HashSet<Hash256>>,
}

impl Mempool {
    /// Creates a new empty Mempool instance.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            spent_outpoints: HashMap::new(),
            parent_to_children: HashMap::new(),
            child_to_parents: HashMap::new(),
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

    /// Checks if a transaction with the given TxID is present in the pool.
    pub fn contains(&self, txid: &Hash256) -> bool {
        self.entries.contains_key(txid)
    }

    /// Retrieves a reference to a MempoolEntry by its TxID.
    pub fn get(&self, txid: &Hash256) -> Option<&MempoolEntry> {
        self.entries.get(txid)
    }

    /// Returns all mempool entries sorted descending by fee-rate.
    pub fn get_entries_sorted_by_fee_rate(&self) -> Vec<MempoolEntry> {
        let mut entries: Vec<MempoolEntry> = self.entries.values().cloned().collect();
        entries.sort_by_key(|b| std::cmp::Reverse(b.fee_rate));
        entries
    }

    /// Admits a new transaction into the mempool through the verification pipeline:
    /// 1. Stateless structural validation
    /// 2. Deduplication check
    /// 3. In-flight double spend check against spent_outpoints
    /// 4. Input UTXO resolution (canonical UTXO set + pending mempool outputs)
    /// 5. Authorization verification
    /// 6. Value conservation & fee calculation
    /// 7. Insertion and dependency registration
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

        // 7. Commit to Mempool
        let entry = MempoolEntry::new(tx.clone(), fee, current_timestamp);

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
