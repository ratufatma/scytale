use crate::error::UtxoError;
use crate::transaction::Transaction;
use scytale_primitives::{Hash256, OutPoint, Quanta, TxOut};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// UtxoEntry: Represents an unspent transaction output with block height metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UtxoEntry {
    pub output: TxOut,
    pub block_height: u64,
    pub is_coinbase: bool,
}

impl UtxoEntry {
    pub fn new(output: TxOut, block_height: u64, is_coinbase: bool) -> Self {
        Self {
            output,
            block_height,
            is_coinbase,
        }
    }
}

/// In-memory UTXO Set tracker and state transition engine.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UtxoSet {
    entries: HashMap<OutPoint, UtxoEntry>,
}

impl UtxoSet {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn contains(&self, outpoint: &OutPoint) -> bool {
        self.entries.contains_key(outpoint)
    }

    pub fn get(&self, outpoint: &OutPoint) -> Option<&UtxoEntry> {
        self.entries.get(outpoint)
    }

    pub fn insert(&mut self, outpoint: OutPoint, entry: UtxoEntry) -> Option<UtxoEntry> {
        self.entries.insert(outpoint, entry)
    }

    pub fn remove(&mut self, outpoint: &OutPoint) -> Option<UtxoEntry> {
        self.entries.remove(outpoint)
    }

    pub fn entries(&self) -> &HashMap<OutPoint, UtxoEntry> {
        &self.entries
    }

    /// Calculates the total value of all unspent outputs in the set (in quanta).
    pub fn total_quanta(&self) -> Result<Quanta, UtxoError> {
        let mut total: Quanta = 0;
        for entry in self.entries.values() {
            total = total
                .checked_add(entry.output.value)
                .ok_or(UtxoError::ArithmeticOverflow)?;
        }
        Ok(total)
    }

    /// Validates and applies a single standard (non-coinbase) transaction atomically.
    ///
    /// Returns the fee in quanta on success, or an error without mutating state on failure.
    pub fn apply_transaction(
        &mut self,
        tx: &Transaction,
        block_height: u64,
    ) -> Result<u64, UtxoError> {
        // 1. Perform stateless validation
        tx.validate_stateless()?;

        // 2. Validate all inputs exist in the UTXO set and calculate total input value
        let mut total_in: u64 = 0;
        for input in &tx.inputs {
            let entry = self
                .entries
                .get(&input.previous_output)
                .ok_or(UtxoError::MissingUtxo(input.previous_output))?;

            total_in = total_in
                .checked_add(entry.output.value)
                .ok_or(UtxoError::ArithmeticOverflow)?;
        }

        // 3. Calculate total output value
        let total_out = tx.total_output_quanta()?;

        // 4. Value conservation check (Inputs >= Outputs)
        if total_out > total_in {
            return Err(UtxoError::ValueDeficit {
                total_in,
                total_out,
            });
        }

        let fee = total_in
            .checked_sub(total_out)
            .ok_or(UtxoError::ArithmeticOverflow)?;

        // 5. Atomic state mutation: remove consumed inputs and insert new outputs
        for input in &tx.inputs {
            self.entries.remove(&input.previous_output);
        }

        let txid = tx.txid();
        for (index, output) in tx.outputs.iter().enumerate() {
            // Consensus rule: OP_RETURN outputs (starting with 0x6a) are provably unspendable data commitments,
            // and must NOT be added to the active UTXO set.
            if output.locking_condition.first() == Some(&0x6a) {
                continue;
            }
            let outpoint = OutPoint::new(txid, index as u32);
            let entry = UtxoEntry::new(output.clone(), block_height, false);
            self.entries.insert(outpoint, entry);
        }

        Ok(fee)
    }

    /// Applies a coinbase transaction by adding its outputs to the UTXO set.
    pub fn apply_coinbase(&mut self, tx: &Transaction, block_height: u64) -> Result<(), UtxoError> {
        if tx.outputs.is_empty() {
            return Err(UtxoError::TxError(
                crate::error::TransactionError::EmptyOutputs,
            ));
        }

        for output in &tx.outputs {
            if output.value == 0 && output.locking_condition.first() != Some(&0x6a) {
                return Err(UtxoError::TxError(
                    crate::error::TransactionError::ZeroOutputValue,
                ));
            }
        }

        // Check overflow
        let _ = tx.total_output_quanta()?;

        let txid = tx.txid();
        for (index, output) in tx.outputs.iter().enumerate() {
            if output.locking_condition.first() == Some(&0x6a) {
                continue;
            }
            let outpoint = OutPoint::new(txid, index as u32);
            let entry = UtxoEntry::new(output.clone(), block_height, true);
            self.entries.insert(outpoint, entry);
        }

        Ok(())
    }

    /// Applies a block of transactions atomically (coinbase + standard transactions).
    ///
    /// If any transaction fails, the entire state transition rolls back completely.
    /// Returns the total fees collected from all transactions in the block.
    pub fn apply_block_transactions(
        &mut self,
        coinbase: &Transaction,
        txs: &[Transaction],
        block_height: u64,
    ) -> Result<u64, UtxoError> {
        let mut staging = self.clone();

        staging.apply_coinbase(coinbase, block_height)?;

        let mut total_fees: u64 = 0;
        for tx in txs {
            let fee = staging.apply_transaction(tx, block_height)?;
            total_fees = total_fees
                .checked_add(fee)
                .ok_or(UtxoError::ArithmeticOverflow)?;
        }

        // All succeeded, commit atomically
        *self = staging;
        Ok(total_fees)
    }

    /// Computes the canonical Merkle root for the entire in-memory UTXO set.
    /// Sorts outpoints strictly lexicographically by (txid ASC, index ASC).
    pub fn compute_utxo_root(&self) -> Hash256 {
        let mut sorted_entries: Vec<(&OutPoint, &UtxoEntry)> = self.entries.iter().collect();
        sorted_entries.sort_by(|(a_op, _), (b_op, _)| {
            a_op.txid
                .cmp(&b_op.txid)
                .then_with(|| a_op.index.cmp(&b_op.index))
        });
        let leaves: Vec<Hash256> = sorted_entries
            .into_iter()
            .map(|(op, entry)| compute_utxo_leaf(op, &entry.output))
            .collect();
        compute_utxo_merkle_root(leaves)
    }
}

/// Computes the 32-byte BLAKE3 canonical leaf hash for an active UTXO entry:
/// `BLAKE3("SCYTALE_UTXO_LEAF_V1" || txid (32B) || index (4B LE) || value (8B LE) || locking_condition)`
pub fn compute_utxo_leaf(outpoint: &OutPoint, output: &TxOut) -> Hash256 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"SCYTALE_UTXO_LEAF_V1");
    hasher.update(outpoint.txid.as_bytes());
    hasher.update(&outpoint.index.to_le_bytes());
    hasher.update(&output.value.to_le_bytes());
    hasher.update(&output.locking_condition);
    Hash256::new(*hasher.finalize().as_bytes())
}

/// Computes the canonical balanced binary Merkle root from a list of UTXO leaf hashes.
/// - If leaves is empty, returns `Hash256::ZERO`.
/// - If odd number of leaves, duplicates the last leaf.
/// - Merges pairs: `Parent = BLAKE3(Left || Right)`.
pub fn compute_utxo_merkle_root(mut leaves: Vec<Hash256>) -> Hash256 {
    if leaves.is_empty() {
        return Hash256::ZERO;
    }
    while leaves.len() > 1 {
        if leaves.len() % 2 == 1 {
            let last = *leaves.last().unwrap();
            leaves.push(last);
        }
        let mut next_level = Vec::with_capacity(leaves.len() / 2);
        for chunk in leaves.chunks_exact(2) {
            let mut hasher = blake3::Hasher::new();
            hasher.update(chunk[0].as_bytes());
            hasher.update(chunk[1].as_bytes());
            next_level.push(Hash256::new(*hasher.finalize().as_bytes()));
        }
        leaves = next_level;
    }
    leaves[0]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{TxIn, TRANSACTION_VERSION_1};
    use scytale_primitives::Hash256;

    #[test]
    fn test_outpoint_primary_key_uniqueness() {
        let txid1 = Hash256::hash(b"txid_1");
        let txid2 = Hash256::hash(b"txid_2");

        let op1 = OutPoint::new(txid1, 0);
        let op2 = OutPoint::new(txid1, 1);
        let op3 = OutPoint::new(txid2, 0);

        let mut set = UtxoSet::new();
        set.insert(op1, UtxoEntry::new(TxOut::new(100, vec![]), 1, false));
        set.insert(op2, UtxoEntry::new(TxOut::new(200, vec![]), 1, false));
        set.insert(op3, UtxoEntry::new(TxOut::new(300, vec![]), 1, false));

        assert_eq!(set.len(), 3);
        assert_eq!(set.get(&op1).unwrap().output.value, 100);
        assert_eq!(set.get(&op2).unwrap().output.value, 200);
        assert_eq!(set.get(&op3).unwrap().output.value, 300);
    }

    #[test]
    fn test_utxo_creation_and_spend() {
        let mut set = UtxoSet::new();

        // Seed an initial UTXO
        let prev_txid = Hash256::hash(b"genesis");
        let initial_op = OutPoint::new(prev_txid, 0);
        set.insert(
            initial_op,
            UtxoEntry::new(TxOut::new(1_000_000_000, vec![1, 2, 3]), 0, true),
        );
        assert_eq!(set.len(), 1);

        // Spend the UTXO in a new transaction
        let input = TxIn::new(initial_op, vec![9, 9]);
        let output = TxOut::new(950_000_000, vec![4, 5, 6]);
        let tx = Transaction::new(TRANSACTION_VERSION_1, vec![input], vec![output], 0);

        let fee = set.apply_transaction(&tx, 1).unwrap();
        assert_eq!(fee, 50_000_000);

        // Old UTXO consumed, new UTXO created
        assert!(!set.contains(&initial_op));
        let new_op = OutPoint::new(tx.txid(), 0);
        assert!(set.contains(&new_op));
        assert_eq!(set.get(&new_op).unwrap().output.value, 950_000_000);
        assert_eq!(set.get(&new_op).unwrap().block_height, 1);
        assert!(!set.get(&new_op).unwrap().is_coinbase);
    }

    #[test]
    fn test_reject_double_spend() {
        let mut set = UtxoSet::new();

        let initial_op = OutPoint::new(Hash256::hash(b"funding"), 0);
        set.insert(
            initial_op,
            UtxoEntry::new(TxOut::new(1_000_000_000, vec![]), 1, false),
        );

        // First spend (valid)
        let tx1 = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(initial_op, vec![])],
            vec![TxOut::new(900_000_000, vec![])],
            0,
        );
        assert!(set.apply_transaction(&tx1, 2).is_ok());

        // Second spend attempting to consume the same input (double spend)
        let tx2 = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(initial_op, vec![])],
            vec![TxOut::new(800_000_000, vec![])],
            0,
        );
        let err = set.apply_transaction(&tx2, 2).unwrap_err();
        assert_eq!(err, UtxoError::MissingUtxo(initial_op));
    }

    #[test]
    fn test_reject_missing_outpoint() {
        let mut set = UtxoSet::new();
        let phantom_op = OutPoint::new(Hash256::hash(b"non_existent"), 0);

        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(phantom_op, vec![])],
            vec![TxOut::new(100_000_000, vec![])],
            0,
        );

        let err = set.apply_transaction(&tx, 1).unwrap_err();
        assert_eq!(err, UtxoError::MissingUtxo(phantom_op));
    }

    #[test]
    fn test_reject_value_deficit() {
        let mut set = UtxoSet::new();
        let op = OutPoint::new(Hash256::hash(b"tx_fund"), 0);
        set.insert(
            op,
            UtxoEntry::new(TxOut::new(100_000_000, vec![]), 1, false),
        );

        // Outputs (150M) exceed input (100M)
        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op, vec![])],
            vec![TxOut::new(150_000_000, vec![])],
            0,
        );

        let err = set.apply_transaction(&tx, 2).unwrap_err();
        assert_eq!(
            err,
            UtxoError::ValueDeficit {
                total_in: 100_000_000,
                total_out: 150_000_000,
            }
        );
        // State remains untouched
        assert!(set.contains(&op));
    }

    #[test]
    fn test_atomic_transition_rollback() {
        let mut set = UtxoSet::new();
        let op1 = OutPoint::new(Hash256::hash(b"tx_1"), 0);
        let op2 = OutPoint::new(Hash256::hash(b"tx_2"), 0);
        set.insert(
            op1,
            UtxoEntry::new(TxOut::new(500_000_000, vec![]), 1, false),
        );
        set.insert(
            op2,
            UtxoEntry::new(TxOut::new(500_000_000, vec![]), 1, false),
        );

        let initial_snapshot = set.clone();

        // Coinbase tx
        let coinbase = Transaction::new_coinbase(0, vec![TxOut::new(1_000_000_000, vec![])]);

        // Tx 1: valid
        let tx1 = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op1, vec![])],
            vec![TxOut::new(450_000_000, vec![])],
            0,
        );

        // Tx 2: invalid (references phantom input)
        let phantom_op = OutPoint::new(Hash256::hash(b"phantom"), 0);
        let tx2 = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(phantom_op, vec![])],
            vec![TxOut::new(100_000_000, vec![])],
            0,
        );

        let result = set.apply_block_transactions(&coinbase, &[tx1, tx2], 2);
        assert!(result.is_err());

        // Assert zero state mutation (complete rollback)
        assert_eq!(set, initial_snapshot);
        assert!(set.contains(&op1));
        assert!(set.contains(&op2));
    }

    #[test]
    fn test_partial_split_conservation() {
        let mut set = UtxoSet::new();
        let op = OutPoint::new(Hash256::hash(b"source"), 0);
        set.insert(op, UtxoEntry::new(TxOut::new(1_000_000, vec![]), 1, false));

        // Split 1,000,000 into 600,000 + 399,000 (fee = 1,000)
        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(op, vec![])],
            vec![TxOut::new(600_000, vec![1]), TxOut::new(399_000, vec![2])],
            0,
        );

        let fee = set.apply_transaction(&tx, 2).unwrap();
        assert_eq!(fee, 1_000);
        assert_eq!(set.len(), 2);
        assert_eq!(set.total_quanta().unwrap(), 999_000);
    }

    #[test]
    fn test_coinbase_application() {
        let mut set = UtxoSet::new();
        let coinbase = Transaction::new_coinbase(0, vec![TxOut::new(1_000_000_000, vec![1, 2, 3])]);

        assert!(set.apply_coinbase(&coinbase, 0).is_ok());
        assert_eq!(set.len(), 1);
        let op = OutPoint::new(coinbase.txid(), 0);
        assert_eq!(set.get(&op).unwrap().block_height, 0);
        assert!(set.get(&op).unwrap().is_coinbase);
        assert_eq!(set.total_quanta().unwrap(), 1_000_000_000);
    }

    #[test]
    fn test_empty_utxo_root_is_zero() {
        let set = UtxoSet::new();
        assert_eq!(set.compute_utxo_root(), Hash256::ZERO);
    }

    #[test]
    fn test_single_utxo_root() {
        let mut set = UtxoSet::new();
        let op = OutPoint::new(Hash256::hash(b"tx"), 0);
        let out = TxOut::new(500, vec![1, 2, 3]);
        set.insert(op, UtxoEntry::new(out.clone(), 0, true));

        let expected_leaf = compute_utxo_leaf(&op, &out);
        assert_eq!(set.compute_utxo_root(), expected_leaf);
    }

    #[test]
    fn test_utxo_root_deterministic_ordering() {
        let mut set1 = UtxoSet::new();
        let mut set2 = UtxoSet::new();

        let op1 = OutPoint::new(Hash256::new([1u8; 32]), 0);
        let op2 = OutPoint::new(Hash256::new([2u8; 32]), 1);
        let op3 = OutPoint::new(Hash256::new([1u8; 32]), 1);

        let out1 = TxOut::new(100, vec![1]);
        let out2 = TxOut::new(200, vec![2]);
        let out3 = TxOut::new(300, vec![3]);

        // Insert in different order
        set1.insert(op1, UtxoEntry::new(out1.clone(), 0, false));
        set1.insert(op2, UtxoEntry::new(out2.clone(), 0, false));
        set1.insert(op3, UtxoEntry::new(out3.clone(), 0, false));

        set2.insert(op3, UtxoEntry::new(out3, 0, false));
        set2.insert(op1, UtxoEntry::new(out1, 0, false));
        set2.insert(op2, UtxoEntry::new(out2, 0, false));

        assert_eq!(set1.compute_utxo_root(), set2.compute_utxo_root());
        assert_ne!(set1.compute_utxo_root(), Hash256::ZERO);
    }
}
