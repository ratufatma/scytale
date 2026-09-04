//! Scytale Passbook: read-only financial presentation layer.
//!
//! The Passbook projects the canonical ledger (UTXO set + confirmed chain +
//! pending mempool) into a human-friendly bank-passbook view: confirmed and
//! pending balances derived on demand, sequential entry numbers, transaction
//! type classification, and value-provenance lineage.
//!
//! Architectural invariants:
//! - The Passbook *displays* ledger state; it never stores or mutates state.
//! - Every projection re-derives from the node's query interface on each call.
//! - All monetary math is strict `u64` quanta; zero floating-point arithmetic.
//! - Wallet key management and signing are strictly out of scope.

use crate::error::{NodeError, NodeState};
use crate::node::Node;
use scytale_core::{Block, Hash256, OutPoint, Transaction, TxOut, UtxoSet};
use scytale_mempool::MempoolEntry;

/// Conversion constant: 1 SCY = 100,000,000 quanta.
pub const QUANTA_PER_SCY: u64 = 100_000_000;

/// Broad classification of a passbook entry's financial direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryType {
    /// Funds arriving at a user-owned output.
    Received,
    /// Funds leaving a user-owned output.
    Sent,
    /// Block subsidy / coinbase issuance credited to the user.
    MiningReward,
    /// Residual output returned to the user within a spending transaction.
    Change,
}

/// Confirmation state of a passbook entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStatus {
    /// Included in a canonical block.
    Confirmed { confirmations: u64 },
    /// Unconfirmed, awaiting inclusion in a block.
    Pending,
    /// Dropped from canonical status by a chain reorganization.
    Reorganized,
}

/// A single human-readable financial journal entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassbookEntry {
    /// Human-readable sequential number (1, 2, 3, ...).
    pub entry_number: u64,
    /// Block timestamp (confirmed) or mempool arrival time (pending), in seconds.
    pub timestamp: u64,
    pub entry_type: EntryType,
    /// Value in quanta moved to/from the user.
    pub amount_quanta: u64,
    /// Transaction fee in quanta attributable to the entry.
    pub fee_quanta: u64,
    pub status: EntryStatus,
    pub txid: Hash256,
    /// The specific outpoint this entry refers to, when it maps to a single output.
    pub outpoint: Option<OutPoint>,
    /// Canonical block height the entry was confirmed at, if confirmed.
    pub block_height: Option<u64>,
}

/// The complete projected passbook view for a given user identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassbookView {
    /// Sum of all spendable user-owned canonical UTXOs (integer quanta).
    pub confirmed_balance_quanta: u64,
    /// Net unconfirmed delta: pending inflows minus pending outflows (can be negative).
    pub pending_balance_quanta: i64,
    pub total_entries: usize,
    pub entries: Vec<PassbookEntry>,
}

/// Category of a step in a value-provenance lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceCategory {
    /// Issued by a Proof-of-Work coinbase subsidy.
    Coinbase,
    /// Issued at the Genesis Block 0 bootstrap transaction.
    Genesis,
    /// Propagated through a normal spending transaction.
    Transfer,
}

/// One hop in a value-provenance lineage path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenanceStep {
    pub txid: Hash256,
    pub block_height: u64,
    pub category: ProvenanceCategory,
    pub value_quanta: u64,
}

/// Domain errors returned by the Passbook projection layer.
#[derive(Debug, thiserror::Error)]
pub enum PassbookError {
    #[error("node runtime is not ready: state {0:?}")]
    NodeNotReady(NodeState),
    #[error("node subsystem query failed: {0}")]
    NodeError(#[from] NodeError),
    #[error("UTXO lookup failed: {0}")]
    UtxoLookupFailed(String),
    #[error("transaction not found: {txid:?}")]
    TransactionNotFound { txid: Hash256 },
    #[error("provenance lineage broken at outpoint {0:?}")]
    ProvenanceLineageBroken(OutPoint),
    #[error("passbook projected over stale ledger state")]
    StaleLedgerState,
}

/// Read-only projection engine over the node's canonical + pending state.
///
/// A `Passbook` is stateless: it holds only the user's locking conditions
/// (owner scripts) that identify which outputs belong to the presented user.
/// Every query re-derives the view from the live node, so the passbook never
/// caches or owns a balance ledger.
#[derive(Debug, Clone)]
pub struct Passbook {
    /// Locking-condition scripts owned by this user.
    owned_locks: Vec<Vec<u8>>,
}

#[allow(clippy::result_large_err)]
impl Passbook {
    /// Creates a projection engine for the given user-owned locking conditions.
    pub fn new(owned_locks: Vec<Vec<u8>>) -> Self {
        Self { owned_locks }
    }

    /// Adds an additional owner locking condition.
    pub fn add_owned_lock(&mut self, lock: Vec<u8>) {
        if !self.owned_locks.contains(&lock) {
            self.owned_locks.push(lock);
        }
    }

    /// Returns `true` if the given locking condition belongs to the user.
    pub fn owns(&self, locking_condition: &[u8]) -> bool {
        self.owned_locks
            .iter()
            .any(|l| l.as_slice() == locking_condition)
    }

    /// Asserts the node is in a query-ready state (`Ready` or `Running`).
    fn require_ready(&self, node: &Node) -> Result<(), PassbookError> {
        match node.state() {
            NodeState::Ready | NodeState::Running => Ok(()),
            other => Err(PassbookError::NodeNotReady(other)),
        }
    }

    /// Derives the confirmed balance by summing all spendable canonical UTXOs
    /// owned by the user. Zero synthetic balances: only real unspent outputs
    /// contribute. Returns 0 quanta for a fresh user with no funds.
    pub fn confirmed_balance_quanta(&self, node: &Node) -> Result<u64, PassbookError> {
        self.require_ready(node)?;
        let utxos = node.query_utxo_set();
        sum_owned(&self.owned_locks, &utxos)
    }

    /// Derives the net unconfirmed balance delta from the mempool:
    /// pending inflows (user-owned outputs) minus pending outflows (spending of
    /// confirmed user-owned inputs). Does not affect the confirmed balance.
    pub fn pending_balance_delta(&self, node: &Node) -> Result<i64, PassbookError> {
        self.require_ready(node)?;
        let confirmed_utxos = node.query_utxo_set();
        let pending = node.query_mempool();
        pending_delta(&self.owned_locks, &confirmed_utxos, &pending)
    }

    /// Projects the full passbook view (confirmed + pending entries and balances)
    /// for the user. Always re-derives from live node state.
    pub fn view(&self, node: &Node) -> Result<PassbookView, PassbookError> {
        self.require_ready(node)?;

        let confirmed_utxos = node.query_utxo_set();
        let confirmed_balance = sum_owned(&self.owned_locks, &confirmed_utxos)?;

        let chain = node.query_canonical_chain()?;
        let tip_height = chain.last().map(|(_, h)| *h).unwrap_or(0);
        let pending = node.query_mempool();

        let mut entries = Vec::new();
        project_confirmed_history(&self.owned_locks, &chain, tip_height, &mut entries)?;
        project_pending_entries(&self.owned_locks, &confirmed_utxos, &pending, &mut entries);

        assign_entry_numbers(&mut entries);

        let pending_balance = pending_delta(&self.owned_locks, &confirmed_utxos, &pending)?;

        Ok(PassbookView {
            confirmed_balance_quanta: confirmed_balance,
            pending_balance_quanta: pending_balance,
            total_entries: entries.len(),
            entries,
        })
    }

    /// Traces the value provenance lineage for a given outpoint back to its
    /// issuance origin (coinbase subsidy or genesis bootstrap).
    pub fn provenance(
        &self,
        node: &Node,
        outpoint: &OutPoint,
    ) -> Result<Vec<ProvenanceStep>, PassbookError> {
        self.require_ready(node)?;

        let chain = node.query_canonical_chain()?;
        let mut tx_height: std::collections::HashMap<Hash256, u64> =
            std::collections::HashMap::new();
        for (block, height) in &chain {
            for tx in &block.transactions {
                tx_height.insert(tx.txid(), *height);
            }
        }

        let mut rev: Vec<ProvenanceStep> = Vec::new();
        let mut cur = *outpoint;
        loop {
            let tx = node
                .lookup_transaction(&cur.txid)?
                .ok_or_else(|| PassbookError::TransactionNotFound { txid: cur.txid })?;
            let height = tx_height.get(&cur.txid).copied().unwrap_or(0);
            let value = tx
                .outputs
                .get(cur.index as usize)
                .map(|o| o.value)
                .unwrap_or(0);

            if tx.is_coinbase() || height == 0 {
                let category = if height == 0 {
                    ProvenanceCategory::Genesis
                } else {
                    ProvenanceCategory::Coinbase
                };
                rev.push(ProvenanceStep {
                    txid: cur.txid,
                    block_height: height,
                    category,
                    value_quanta: value,
                });
                break;
            }

            rev.push(ProvenanceStep {
                txid: cur.txid,
                block_height: height,
                category: ProvenanceCategory::Transfer,
                value_quanta: value,
            });

            let next = tx.inputs.first().map(|i| i.previous_output);
            match next {
                Some(op) if !op.is_null() => {
                    if op == cur {
                        return Err(PassbookError::ProvenanceLineageBroken(cur));
                    }
                    if node.lookup_transaction(&op.txid)?.is_none() {
                        break;
                    }
                    cur = op;
                }
                _ => break,
            }
        }

        // Reverse to present origin -> current.
        rev.reverse();
        Ok(rev)
    }
}

/// Sums the value of all user-owned outputs in a UTXO set (integer quanta).
#[allow(clippy::result_large_err)]
fn sum_owned(owned_locks: &[Vec<u8>], utxos: &UtxoSet) -> Result<u64, PassbookError> {
    let owned = &owned_locks;
    let mut total: u64 = 0;
    for entry in utxos.entries().values() {
        if is_owned(owned, &entry.output.locking_condition) {
            total = total
                .checked_add(entry.output.value)
                .ok_or_else(|| PassbookError::UtxoLookupFailed("balance overflow".into()))?;
        }
    }
    Ok(total)
}

/// Computes the net pending delta (inflows minus outflows) as a signed `i64`.
#[allow(clippy::result_large_err)]
fn pending_delta(
    owned_locks: &[Vec<u8>],
    confirmed_utxos: &UtxoSet,
    pending: &[MempoolEntry],
) -> Result<i64, PassbookError> {
    let owned = &owned_locks;
    let mut inflow: u64 = 0;
    let mut outflow: u64 = 0;
    for entry in pending {
        for out in &entry.transaction.outputs {
            if is_owned(owned, &out.locking_condition) {
                inflow = inflow
                    .checked_add(out.value)
                    .ok_or_else(|| PassbookError::UtxoLookupFailed("inflow overflow".into()))?;
            }
        }
        for input in &entry.transaction.inputs {
            if let Some(spent) = confirmed_utxos.get(&input.previous_output) {
                if is_owned(owned, &spent.output.locking_condition) {
                    outflow = outflow.checked_add(spent.output.value).ok_or_else(|| {
                        PassbookError::UtxoLookupFailed("outflow overflow".into())
                    })?;
                }
            }
        }
    }

    let delta = (inflow as i128) - (outflow as i128);
    if delta > i64::MAX as i128 || delta < i64::MIN as i128 {
        return Err(PassbookError::UtxoLookupFailed(
            "pending delta overflow".into(),
        ));
    }
    Ok(delta as i64)
}

/// Returns `true` if the given locking condition matches any owned lock.
fn is_owned(owned_locks: &[Vec<u8>], locking_condition: &[u8]) -> bool {
    owned_locks
        .iter()
        .any(|l| l.as_slice() == locking_condition)
}

/// Walks the canonical chain chronologically, projecting confirmed entries and
/// tracking which outpoints the user currently owns.
#[allow(clippy::result_large_err)]
fn project_confirmed_history(
    owned_locks: &[Vec<u8>],
    chain: &[(Block, u64)],
    tip_height: u64,
    out: &mut Vec<PassbookEntry>,
) -> Result<(), PassbookError> {
    // outpoint -> value currently owned by the user (confirmed spendable).
    let mut owned: std::collections::HashMap<OutPoint, u64> = std::collections::HashMap::new();

    for (block, height) in chain {
        let timestamp = block.header.timestamp;
        for tx in &block.transactions {
            let txid = tx.txid();

            if tx.is_coinbase() {
                for (idx, output) in tx.outputs.iter().enumerate() {
                    if is_owned(owned_locks, &output.locking_condition) {
                        let op = OutPoint::new(txid, idx as u32);
                        owned.insert(op, output.value);
                        out.push(confirmed_entry(
                            timestamp,
                            EntryType::MiningReward,
                            output.value,
                            0,
                            *height,
                            tip_height,
                            txid,
                            Some(op),
                        ));
                    }
                }
                continue;
            }

            // Identify user-owned consumed inputs.
            let mut user_input_value: u64 = 0;
            let mut consumed_user_inputs: usize = 0;
            for input in &tx.inputs {
                if let Some(value) = owned.get(&input.previous_output).copied() {
                    user_input_value = user_input_value.checked_add(value).ok_or_else(|| {
                        PassbookError::UtxoLookupFailed("input value overflow".into())
                    })?;
                    consumed_user_inputs += 1;
                }
            }
            let owns_input = consumed_user_inputs > 0;

            let total_output = tx
                .total_output_quanta()
                .map_err(|e| PassbookError::UtxoLookupFailed(e.to_string()))?;

            if owns_input {
                for input in &tx.inputs {
                    owned.remove(&input.previous_output);
                }
            }

            // Collect user-owned outputs.
            let mut owned_outputs: Vec<(usize, &TxOut)> = Vec::new();
            for (idx, output) in tx.outputs.iter().enumerate() {
                if is_owned(owned_locks, &output.locking_condition) {
                    owned_outputs.push((idx, output));
                }
            }

            if owned_outputs.is_empty() {
                // No output returned to the user -> Sent (if the user funded it).
                if owns_input {
                    let fee = user_input_value.saturating_sub(total_output);
                    out.push(confirmed_entry(
                        timestamp,
                        EntryType::Sent,
                        user_input_value,
                        fee,
                        *height,
                        tip_height,
                        txid,
                        None,
                    ));
                }
                continue;
            }

            let fee = if owns_input {
                user_input_value.saturating_sub(total_output)
            } else {
                0
            };
            for (idx, output) in owned_outputs {
                let op = OutPoint::new(txid, idx as u32);
                owned.insert(op, output.value);
                let entry_type = if owns_input {
                    EntryType::Change
                } else {
                    EntryType::Received
                };
                out.push(confirmed_entry(
                    timestamp,
                    entry_type,
                    output.value,
                    fee,
                    *height,
                    tip_height,
                    txid,
                    Some(op),
                ));
            }
        }
    }

    Ok(())
}

/// Constructs a confirmed passbook entry with the given confirmations.
#[allow(clippy::too_many_arguments)]
fn confirmed_entry(
    timestamp: u64,
    entry_type: EntryType,
    amount: u64,
    fee: u64,
    height: u64,
    tip_height: u64,
    txid: Hash256,
    outpoint: Option<OutPoint>,
) -> PassbookEntry {
    PassbookEntry {
        entry_number: 0,
        timestamp,
        entry_type,
        amount_quanta: amount,
        fee_quanta: fee,
        status: EntryStatus::Confirmed {
            confirmations: tip_height + 1 - height,
        },
        txid,
        outpoint,
        block_height: Some(height),
    }
}

/// Projects pending (unconfirmed) entries from the mempool for the user.
fn project_pending_entries(
    owned_locks: &[Vec<u8>],
    confirmed_utxos: &UtxoSet,
    pending: &[MempoolEntry],
    out: &mut Vec<PassbookEntry>,
) {
    for entry in pending {
        let tx: &Transaction = &entry.transaction;
        let txid = entry.txid;
        let timestamp = entry.added_time;

        let owns_input = tx.inputs.iter().any(|input| {
            confirmed_utxos
                .get(&input.previous_output)
                .map(|spent| is_owned(owned_locks, &spent.output.locking_condition))
                .unwrap_or(false)
        });

        let mut has_user_output = false;
        for (idx, output) in tx.outputs.iter().enumerate() {
            if is_owned(owned_locks, &output.locking_condition) {
                has_user_output = true;
                let op = OutPoint::new(txid, idx as u32);
                let entry_type = if owns_input {
                    EntryType::Change
                } else {
                    EntryType::Received
                };
                out.push(PassbookEntry {
                    entry_number: 0,
                    timestamp,
                    entry_type,
                    amount_quanta: output.value,
                    fee_quanta: entry.fee,
                    status: EntryStatus::Pending,
                    txid,
                    outpoint: Some(op),
                    block_height: None,
                });
            }
        }

        if !has_user_output && owns_input {
            let total_output = tx.total_output_quanta().unwrap_or(0);
            out.push(PassbookEntry {
                entry_number: 0,
                timestamp,
                entry_type: EntryType::Sent,
                amount_quanta: entry.fee.saturating_add(total_output),
                fee_quanta: entry.fee,
                status: EntryStatus::Pending,
                txid,
                outpoint: None,
                block_height: None,
            });
        }
    }
}

/// Assigns ascending human-readable entry numbers after final ordering.
///
/// Entries are ordered chronologically (by canonical height, then insertion
/// order for same-height rows) and numbered 1..n for easy reference.
fn assign_entry_numbers(entries: &mut [PassbookEntry]) {
    entries.sort_by_key(|e| (e.block_height, e.idx()));
    for (i, entry) in entries.iter_mut().enumerate() {
        entry.entry_number = (i + 1) as u64;
    }
}

impl PassbookEntry {
    /// Stable ordering key: pending (height = None) sorts last.
    fn idx(&self) -> (bool, u64) {
        match self.block_height {
            Some(h) => (false, h),
            None => (true, 0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scytale_core::{OutPoint, TxOut, UtxoSet, QUANTA_PER_SCY};

    #[test]
    fn conversion_constant_is_100m() {
        assert_eq!(QUANTA_PER_SCY, 100_000_000);
    }

    #[test]
    fn owns_matches_exact_lock() {
        let p = Passbook::new(vec![vec![1, 2, 3]]);
        assert!(p.owns(&[1, 2, 3]));
        assert!(!p.owns(&[1, 2]));
    }

    #[test]
    fn zero_balance_initialization() {
        let p = Passbook::new(vec![vec![7, 7, 7]]);
        let utxos = UtxoSet::new();
        assert_eq!(sum_owned(&p.owned_locks.clone(), &utxos).unwrap(), 0);
    }

    #[test]
    fn balance_summation_multiple_utxos() {
        let mut utxos = UtxoSet::new();
        let lock = vec![9, 9, 9];
        for (i, v) in [100u64, 200u64, 300u64].iter().enumerate() {
            let txid = Hash256::hash(format!("tx{i}").as_bytes());
            utxos.insert(
                OutPoint::new(txid, 0),
                scytale_core::UtxoEntry::new(TxOut::new(*v, lock.clone()), 1, false),
            );
        }
        let p = Passbook::new(vec![lock]);
        assert_eq!(sum_owned(&p.owned_locks.clone(), &utxos).unwrap(), 600);
    }

    #[test]
    fn pending_delta_separates_inflows_and_outflows() {
        use scytale_core::transaction::TxIn;
        use scytale_core::TRANSACTION_VERSION_1;
        let lock = vec![1, 1, 1];
        let other = vec![2, 2, 2];

        let mut confirmed = UtxoSet::new();
        let fund_txid = Hash256::hash(b"fund");
        let fund_op = OutPoint::new(fund_txid, 0);
        confirmed.insert(
            fund_op,
            scytale_core::UtxoEntry::new(TxOut::new(1_000_000, lock.clone()), 1, false),
        );

        // Pending tx paying 300k to user and spending 1M from user (change 600k back).
        let input = TxIn::new(fund_op, vec![]);
        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![input],
            vec![
                TxOut::new(600_000, lock.clone()),
                TxOut::new(400_000, other),
            ],
            0,
        );
        let pending = vec![MempoolEntry::new(tx, 0, 100)];

        let delta = pending_delta(std::slice::from_ref(&lock), &confirmed, &pending).unwrap();
        // inflow = 600k (change back to user); outflow = 1M (user's confirmed input)
        assert_eq!(delta, -400_000);
    }
}
