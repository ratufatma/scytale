//! Scytale Passbook: read-only financial presentation layer.
//!
//! The Passbook projects the canonical ledger (UTXO set + confirmed chain +
//! pending mempool) into a human-friendly bank-passbook view: confirmed and
//! pending balances derived on demand, sequential entry numbers, transaction
//! classification, token tracking, and value-provenance lineage.
//!
//! Architectural invariants:
//! - The Passbook *displays* ledger state; it never stores or mutates state.
//! - Every projection re-derives from the node's query interface on each call.
//! - All monetary math is strict integer arithmetic (quanta); zero floating-point arithmetic.
//! - Wallet key management and signing are strictly out of scope.

use crate::error::{NodeError, NodeState};
use crate::node::Node;
use scytale_core::{
    Address, Hash256, OutPoint, OutputLock, Transaction, TxOut, UtxoMerkleProof, UtxoSet,
};
use scytale_mempool::MempoolEntry;
use scytale_storage::AddressTxRecord;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Conversion constant: 1 SCY = 100,000,000 quanta.
pub const QUANTA_PER_SCY: u64 = 100_000_000;

/// Classification of the financial asset tracked by a passbook entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PassbookAsset {
    /// Native Scytale base currency (SCY / quanta).
    Native,
    /// Fungible token adhering to the SCY-20 standard.
    Scy20 { token_id: Hash256 },
}

/// Comprehensive classification of financial mutations and smart contract interactions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PassbookAction {
    /// Funds arriving at a user-owned output.
    Received,
    /// Funds leaving a user-owned output to an external recipient.
    Sent,
    /// Block subsidy / coinbase issuance credited to the user.
    MiningReward,
    /// Residual output returned to the user within a spending transaction.
    Change,
    /// SCY-20 token minted into user custody.
    Scy20Mint,
    /// SCY-20 token transferred between accounts.
    Scy20Transfer,
    /// SCY-20 token burned / destroyed from user custody.
    Scy20Burn,
    /// Generic smart contract interaction with an optional datum commitment hash.
    ContractInteraction { datum_hash: Option<Hash256> },
    /// Deposit into a time-locked vault contract.
    VaultDeposit { timelock_until: u64 },
    /// Withdrawal / redemption from a vault contract.
    VaultWithdrawal,
}

/// Backward compatibility alias for legacy API callers.
pub type EntryType = PassbookAction;

/// Confirmation state of a passbook entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryStatus {
    /// Included in a canonical block.
    Confirmed { confirmations: u64 },
    /// Unconfirmed, awaiting inclusion in a block.
    Pending,
    /// Dropped from canonical status by a chain reorganization.
    Reorganized,
}

/// A single human-readable financial journal entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassbookEntry {
    /// Human-readable sequential number (1, 2, 3, ...).
    pub entry_number: u64,
    /// Block timestamp (confirmed) or mempool arrival time (pending), in seconds.
    pub timestamp: u64,
    /// Asset type (Native or Scy20).
    pub asset: PassbookAsset,
    /// Financial action / mutation type.
    pub action: PassbookAction,
    /// Value in quanta or token units moved to/from the user.
    pub amount_quanta: u64,
    /// Transaction fee in quanta attributable to the entry.
    pub fee_quanta: u64,
    pub status: EntryStatus,
    pub txid: Hash256,
    /// The specific outpoint this entry refers to, when it maps to a single output.
    pub outpoint: Option<OutPoint>,
    /// Canonical block height the entry was confirmed at, if confirmed.
    pub block_height: Option<u64>,
    /// Optional datum commitment hash for smart contract or eUTXO locks.
    pub datum_hash: Option<Hash256>,
}

impl PassbookEntry {
    /// Backward-compatible getter for entry_type.
    pub fn entry_type(&self) -> &PassbookAction {
        &self.action
    }

    /// Stable ordering key: pending (height = None) sorts last.
    fn idx(&self) -> (bool, u64) {
        match self.block_height {
            Some(h) => (false, h),
            None => (true, 0),
        }
    }
}

/// The complete projected passbook view for a given user identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassbookView {
    /// Sum of all spendable user-owned canonical native UTXOs (integer quanta).
    pub confirmed_native_balance_quanta: u64,
    /// Map of token balances owned by the user (token_id -> balance).
    pub token_balances: BTreeMap<Hash256, u64>,
    /// Net unconfirmed native delta: pending inflows minus pending outflows (can be negative).
    pub pending_native_balance_quanta: i64,
    /// Sequential financial journal entries.
    pub entries: Vec<PassbookEntry>,
}

impl PassbookView {
    /// Backward-compatible getter for total entries.
    pub fn total_entries(&self) -> usize {
        self.entries.len()
    }

    /// Backward-compatible getter for confirmed native balance quanta.
    pub fn confirmed_balance_quanta(&self) -> u64 {
        self.confirmed_native_balance_quanta
    }

    /// Backward-compatible getter for pending native balance quanta.
    pub fn pending_balance_quanta(&self) -> i64 {
        self.pending_native_balance_quanta
    }
}

/// A cryptographically verifiable snapshot of an account's passbook statement,
/// complete with balanced binary Merkle inclusion proofs for all active spendable native UTXOs
/// against the canonical tip block's `utxo_root`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassbookStatement {
    /// Account address for which this statement was generated.
    pub account: Address,
    /// Canonical chain height at which this statement was projected.
    pub generated_at_height: u64,
    /// Canonical tip block hash corresponding to this statement.
    pub block_hash: Hash256,
    /// Merkle root of the active UTXO set committed in the block header.
    pub utxo_root: Hash256,
    /// Sum of all confirmed spendable native quanta owned by this account.
    pub confirmed_native_balance_quanta: u64,
    /// Balances of all SCY-20 tokens owned by this account.
    pub token_balances: BTreeMap<Hash256, u64>,
    /// Chronological list of confirmed and pending journal entries.
    pub entries: Vec<PassbookEntry>,
    /// Cryptographic Merkle proofs proving inclusion of each active native UTXO in `utxo_root`.
    pub active_utxo_proofs: Vec<UtxoMerkleProof>,
}

impl PassbookStatement {
    /// Verifies the cryptographic integrity of this statement offline:
    /// 1. Validates that every active UTXO Merkle proof hashes up to `self.utxo_root`.
    /// 2. Ensures the sum of `value_quanta` across all proofs exactly matches `self.confirmed_native_balance_quanta`.
    pub fn verify_integrity(&self) -> bool {
        let mut sum: u64 = 0;
        for proof in &self.active_utxo_proofs {
            if !proof.verify(&self.utxo_root) {
                return false;
            }
            sum = match sum.checked_add(proof.value_quanta) {
                Some(v) => v,
                None => return false,
            };
        }
        sum == self.confirmed_native_balance_quanta
    }
}

/// Category of a step in a value-provenance lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenanceCategory {
    /// Issued by a Proof-of-Work coinbase subsidy.
    Coinbase,
    /// Issued at the Genesis Block 0 bootstrap transaction.
    Genesis,
    /// Propagated through a normal spending transaction.
    Transfer,
}

/// One hop in a value-provenance lineage path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    NodeError(Box<NodeError>),
    #[error("storage engine error: {0}")]
    StorageError(Box<scytale_storage::StorageError>),
    #[error("UTXO lookup failed: {0}")]
    UtxoLookupFailed(String),
    #[error("transaction not found: {txid:?}")]
    TransactionNotFound { txid: Hash256 },
    #[error("provenance lineage broken at outpoint {0:?}")]
    ProvenanceLineageBroken(OutPoint),
    #[error("passbook projected over stale ledger state")]
    StaleLedgerState,
}

impl From<NodeError> for PassbookError {
    fn from(err: NodeError) -> Self {
        PassbookError::NodeError(Box::new(err))
    }
}

impl From<scytale_storage::StorageError> for PassbookError {
    fn from(err: scytale_storage::StorageError) -> Self {
        PassbookError::StorageError(Box::new(err))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Contract Payload Deserialization Helpers
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scy20DatumPayload {
    pub token_id: [u8; 32],
    pub owner: [u8; 32],
    pub amount: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultDatumPayload {
    pub owner_pubkey: [u8; 32],
    pub unlock_time: u64,
    pub emergency_key: [u8; 32],
    pub penalty_fee: u64,
}

pub enum ParsedContractCondition {
    Scy20(Scy20DatumPayload),
    Vault(VaultDatumPayload),
    GenericScript { datum_hash: Option<Hash256> },
    Standard,
}

pub fn parse_locking_condition(script: &[u8]) -> ParsedContractCondition {
    if let Some(lock) = OutputLock::from_locking_condition(script) {
        match lock {
            OutputLock::PublicKey(_) => ParsedContractCondition::Standard,
            OutputLock::Script { script_hash: _, datum } => {
                if let Ok(scy20) = bincode::deserialize::<Scy20DatumPayload>(&datum) {
                    return ParsedContractCondition::Scy20(scy20);
                }
                if let Ok(vault) = bincode::deserialize::<VaultDatumPayload>(&datum) {
                    return ParsedContractCondition::Vault(vault);
                }
                let datum_hash = if !datum.is_empty() {
                    Some(Hash256::hash(&datum))
                } else {
                    None
                };
                ParsedContractCondition::GenericScript { datum_hash }
            }
        }
    } else {
        ParsedContractCondition::Standard
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Read-only projection engine
// ─────────────────────────────────────────────────────────────────────────────

/// Read-only projection engine over the node's canonical + pending state.
#[derive(Debug, Clone)]
pub struct Passbook {
    /// Locking-condition scripts owned by this user.
    owned_locks: Vec<Vec<u8>>,
    /// Derived canonical Addresses.
    addresses: Vec<Address>,
}

impl Passbook {
    /// Creates a projection engine for the given user-owned locking conditions.
    pub fn new(owned_locks: Vec<Vec<u8>>) -> Self {
        let mut addresses = Vec::new();
        for lock in &owned_locks {
            if let Some(hash) = scytale_storage::extract_address_from_locking_condition(lock) {
                let addr = Address::new(hash);
                if !addresses.contains(&addr) {
                    addresses.push(addr);
                }
            }
        }
        Self {
            owned_locks,
            addresses,
        }
    }

    /// Creates a projection engine from a single Address.
    pub fn from_address(address: Address) -> Self {
        let lock = address.hash().to_vec();
        Self {
            owned_locks: vec![lock],
            addresses: vec![address],
        }
    }

    /// Adds an additional owner locking condition.
    pub fn add_owned_lock(&mut self, lock: Vec<u8>) {
        if !self.owned_locks.contains(&lock) {
            if let Some(hash) = scytale_storage::extract_address_from_locking_condition(&lock) {
                let addr = Address::new(hash);
                if !self.addresses.contains(&addr) {
                    self.addresses.push(addr);
                }
            }
            self.owned_locks.push(lock);
        }
    }

    /// Returns `true` if the given locking condition belongs to the user.
    pub fn owns(&self, locking_condition: &[u8]) -> bool {
        if self.owned_locks.iter().any(|l| l.as_slice() == locking_condition) {
            return true;
        }
        if let Some(h) = scytale_storage::extract_address_from_locking_condition(locking_condition) {
            return self.addresses.iter().any(|a| a.hash() == &h);
        }
        false
    }

    /// Asserts the node is in a query-ready state (`Ready` or `Running`).
    fn require_ready(&self, node: &Node) -> Result<(), PassbookError> {
        match node.state() {
            NodeState::Ready | NodeState::Running => Ok(()),
            other => Err(PassbookError::NodeNotReady(other)),
        }
    }

    /// Derives the confirmed native balance by summing all spendable canonical native UTXOs
    /// owned by the user. Zero synthetic balances: only real unspent outputs contribute.
    pub fn confirmed_balance_quanta(&self, node: &Node) -> Result<u64, PassbookError> {
        self.require_ready(node)?;
        let utxos = node.query_utxo_set();
        sum_native_owned(self, &utxos)
    }

    /// Returns the confirmed balances of all SCY-20 tokens owned by the user.
    pub fn token_balances(&self, node: &Node) -> Result<BTreeMap<Hash256, u64>, PassbookError> {
        self.require_ready(node)?;
        let utxos = node.query_utxo_set();
        sum_token_owned(self, &utxos)
    }

    /// Derives the net unconfirmed balance delta from the mempool:
    /// pending inflows (user-owned outputs) minus pending outflows (spending of
    /// confirmed user-owned inputs). Does not affect the confirmed balance.
    pub fn pending_balance_delta(&self, node: &Node) -> Result<i64, PassbookError> {
        self.require_ready(node)?;
        let confirmed_utxos = node.query_utxo_set();
        let pending = node.query_mempool();
        pending_delta(self, &confirmed_utxos, &pending)
    }

    /// Projects the full passbook view (confirmed + pending entries and balances)
    /// for the user using the optimized `ADDRESS_TX_INDEX` storage engine index.
    pub fn view(&self, node: &Node) -> Result<PassbookView, PassbookError> {
        self.require_ready(node)?;

        let confirmed_utxos = node.query_utxo_set();
        let confirmed_native = sum_native_owned(self, &confirmed_utxos)?;
        let token_balances = sum_token_owned(self, &confirmed_utxos)?;

        let tip_height = node.canonical_height();
        let pending = node.query_mempool();

        let mut entries = Vec::new();
        project_confirmed_history_via_index(self, node, tip_height, &mut entries)?;
        project_pending_entries(self, &confirmed_utxos, &pending, &mut entries);

        assign_entry_numbers(&mut entries);

        let pending_native = pending_delta(self, &confirmed_utxos, &pending)?;

        Ok(PassbookView {
            confirmed_native_balance_quanta: confirmed_native,
            token_balances,
            pending_native_balance_quanta: pending_native,
            entries,
        })
    }

    /// Generates a cryptographically verifiable PassbookStatement for the specified address.
    ///
    /// Identifies all active native UTXOs owned by `address`, generates balanced
    /// binary Merkle inclusion proofs for each UTXO against the canonical tip block's `utxo_root`,
    /// and bundles them with the complete financial journal entries.
    pub fn generate_statement(
        &self,
        node: &Node,
        address: &Address,
    ) -> Result<PassbookStatement, PassbookError> {
        self.require_ready(node)?;

        let height = node.canonical_height();
        let tip_hash = node.canonical_tip();
        let block = node
            .storage_handle()
            .get_block(&tip_hash)?
            .ok_or_else(|| PassbookError::StaleLedgerState)?;

        let block_hash = block.header.hash();
        let utxo_root = block.header.utxo_root;

        let all_utxos = node.query_utxo_set();
        let utxo_entries_with_outpoints = all_utxos.to_entries_with_outpoints();

        // Identify all active native UTXOs belonging to this address
        let mut user_outpoints = Vec::new();
        for (op, entry) in all_utxos.entries() {
            if let ParsedContractCondition::Scy20(_) =
                parse_locking_condition(&entry.output.locking_condition)
            {
                continue;
            }
            if let Some(h) = scytale_storage::extract_address_from_locking_condition(
                &entry.output.locking_condition,
            ) {
                if Address::new(h) == *address {
                    user_outpoints.push(*op);
                }
            } else if self.owns(&entry.output.locking_condition) && self.addresses.contains(address) {
                user_outpoints.push(*op);
            }
        }

        // Sort outpoints deterministically (txid ASC, index ASC)
        user_outpoints.sort_by(|a, b| a.txid.cmp(&b.txid).then_with(|| a.index.cmp(&b.index)));

        // Generate Merkle proofs for all active user native UTXOs
        let mut active_utxo_proofs = Vec::with_capacity(user_outpoints.len());
        for op in user_outpoints {
            let proof =
                scytale_core::generate_utxo_merkle_proof(&utxo_entries_with_outpoints, &op)
                    .map_err(|e| PassbookError::UtxoLookupFailed(e.to_string()))?;
            active_utxo_proofs.push(proof);
        }

        let view = self.view(node)?;

        Ok(PassbookStatement {
            account: address.clone(),
            generated_at_height: height,
            block_hash,
            utxo_root,
            confirmed_native_balance_quanta: view.confirmed_native_balance_quanta,
            token_balances: view.token_balances,
            entries: view.entries,
            active_utxo_proofs,
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
        let mut tx_height: HashMap<Hash256, u64> = HashMap::new();
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

        rev.reverse();
        Ok(rev)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Balance & Projection Calculations
// ─────────────────────────────────────────────────────────────────────────────

fn sum_native_owned(passbook: &Passbook, utxos: &UtxoSet) -> Result<u64, PassbookError> {
    let mut total: u64 = 0;
    for entry in utxos.entries().values() {
        if passbook.owns(&entry.output.locking_condition) {
            match parse_locking_condition(&entry.output.locking_condition) {
                ParsedContractCondition::Scy20(_) => {
                    // SCY-20 token balances tracked in token_balances
                }
                _ => {
                    total = total
                        .checked_add(entry.output.value)
                        .ok_or_else(|| PassbookError::UtxoLookupFailed("balance overflow".into()))?;
                }
            }
        }
    }
    Ok(total)
}

fn sum_token_owned(
    passbook: &Passbook,
    utxos: &UtxoSet,
) -> Result<BTreeMap<Hash256, u64>, PassbookError> {
    let mut balances = BTreeMap::new();
    for entry in utxos.entries().values() {
        if passbook.owns(&entry.output.locking_condition) {
            if let ParsedContractCondition::Scy20(scy20) =
                parse_locking_condition(&entry.output.locking_condition)
            {
                let tid = Hash256::new(scy20.token_id);
                let current = balances.entry(tid).or_insert(0u64);
                *current = current.checked_add(scy20.amount as u64).unwrap_or(u64::MAX);
            }
        }
    }
    Ok(balances)
}

fn pending_delta(
    passbook: &Passbook,
    confirmed_utxos: &UtxoSet,
    pending: &[MempoolEntry],
) -> Result<i64, PassbookError> {
    let mut inflow: u64 = 0;
    let mut outflow: u64 = 0;
    for entry in pending {
        for out in &entry.transaction.outputs {
            if passbook.owns(&out.locking_condition) {
                if let ParsedContractCondition::Scy20(_) = parse_locking_condition(&out.locking_condition) {
                    continue;
                }
                inflow = inflow
                    .checked_add(out.value)
                    .ok_or_else(|| PassbookError::UtxoLookupFailed("inflow overflow".into()))?;
            }
        }
        for input in &entry.transaction.inputs {
            if let Some(spent) = confirmed_utxos.get(&input.previous_output) {
                if passbook.owns(&spent.output.locking_condition) {
                    if let ParsedContractCondition::Scy20(_) =
                        parse_locking_condition(&spent.output.locking_condition)
                    {
                        continue;
                    }
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

/// Walks the address index in storage to project confirmed transaction history in O(log N + K).
fn project_confirmed_history_via_index(
    passbook: &Passbook,
    node: &Node,
    tip_height: u64,
    out: &mut Vec<PassbookEntry>,
) -> Result<(), PassbookError> {
    let storage = node.storage_handle();

    let chain = node.query_canonical_chain().unwrap_or_default();
    let block_timestamps: HashMap<u64, u64> = chain
        .iter()
        .map(|(b, h)| (*h, b.header.timestamp))
        .collect();

    // Query storage ADDRESS_TX_INDEX for each address owned by this passbook
    let mut height_records: Vec<(u64, AddressTxRecord)> = Vec::new();
    for addr in &passbook.addresses {
        let records = storage.get_address_transactions_with_height(addr, 0, tip_height, usize::MAX)?;
        height_records.extend(records);
    }

    // Sort chronologically ascending
    height_records.sort_by_key(|(h, _)| *h);

    let mut processed_txids: HashSet<Hash256> = HashSet::new();

    for (height, record) in height_records {
        let txid = record.txid;
        if !processed_txids.insert(txid) {
            continue;
        }

        let tx = match node.lookup_transaction(&txid)? {
            Some(t) => t,
            None => continue,
        };

        let timestamp = block_timestamps.get(&height).copied().unwrap_or(0);
        let confirmations = tip_height.saturating_sub(height).saturating_add(1);
        let status = EntryStatus::Confirmed { confirmations };

        // Collect outputs belonging to this user
        let mut user_outputs: Vec<(usize, &TxOut)> = Vec::new();
        for (idx, output) in tx.outputs.iter().enumerate() {
            if passbook.owns(&output.locking_condition) {
                user_outputs.push((idx, output));
            }
        }

        // Determine if user funds any inputs
        let owns_input = record.is_input || tx.inputs.iter().any(|input| {
            if let Ok(Some(prev_tx)) = node.lookup_transaction(&input.previous_output.txid) {
                if let Some(prev_out) = prev_tx.outputs.get(input.previous_output.index as usize) {
                    return passbook.owns(&prev_out.locking_condition);
                }
            }
            false
        });

        // Fee calculation
        let total_output = tx.total_output_quanta().unwrap_or(0);
        let user_input_val: u64 = if owns_input {
            tx.inputs
                .iter()
                .filter_map(|input| {
                    if let Ok(Some(prev_tx)) = node.lookup_transaction(&input.previous_output.txid) {
                        prev_tx
                            .outputs
                            .get(input.previous_output.index as usize)
                            .and_then(|prev_out| {
                                if passbook.owns(&prev_out.locking_condition) {
                                    Some(prev_out.value)
                                } else {
                                    None
                                }
                            })
                    } else {
                        None
                    }
                })
                .sum()
        } else {
            0
        };

        let fee = if owns_input {
            user_input_val.saturating_sub(total_output)
        } else {
            0
        };

        // 2. Outflow with no return output to user -> Sent / Burn / Withdrawal
        if user_outputs.is_empty() && owns_input {
            let (asset, action) = if let Some(tok) = record.token_id {
                (
                    PassbookAsset::Scy20 {
                        token_id: Hash256::new(tok),
                    },
                    PassbookAction::Scy20Transfer,
                )
            } else {
                (PassbookAsset::Native, PassbookAction::Sent)
            };

            out.push(PassbookEntry {
                entry_number: 0,
                timestamp,
                asset,
                action,
                amount_quanta: if record.value_quanta > 0 {
                    record.value_quanta
                } else {
                    user_input_val
                },
                fee_quanta: fee,
                status,
                txid,
                outpoint: None,
                block_height: Some(height),
                datum_hash: None,
            });
            continue;
        }

        // 3. User-owned outputs
        for (idx, output) in user_outputs {
            let op = OutPoint::new(txid, idx as u32);
            let parsed = parse_locking_condition(&output.locking_condition);

            let (asset, action, amount, datum_hash) = match parsed {
                ParsedContractCondition::Scy20(scy20) => {
                    let tid = Hash256::new(scy20.token_id);
                    let act = if tx.is_coinbase() {
                        PassbookAction::Scy20Mint
                    } else {
                        PassbookAction::Scy20Transfer
                    };
                    (
                        PassbookAsset::Scy20 { token_id: tid },
                        act,
                        scy20.amount as u64,
                        None,
                    )
                }
                ParsedContractCondition::Vault(vault) => (
                    PassbookAsset::Native,
                    PassbookAction::VaultDeposit {
                        timelock_until: vault.unlock_time,
                    },
                    output.value,
                    Some(Hash256::hash(
                        &bincode::serialize(&vault).unwrap_or_default(),
                    )),
                ),
                ParsedContractCondition::GenericScript { datum_hash } => (
                    PassbookAsset::Native,
                    PassbookAction::ContractInteraction { datum_hash },
                    output.value,
                    datum_hash,
                ),
                ParsedContractCondition::Standard => {
                    let act = if tx.is_coinbase() {
                        PassbookAction::MiningReward
                    } else if owns_input {
                        PassbookAction::Change
                    } else {
                        PassbookAction::Received
                    };
                    (PassbookAsset::Native, act, output.value, None)
                }
            };

            out.push(PassbookEntry {
                entry_number: 0,
                timestamp,
                asset,
                action,
                amount_quanta: amount,
                fee_quanta: fee,
                status,
                txid,
                outpoint: Some(op),
                block_height: Some(height),
                datum_hash,
            });
        }
    }

    Ok(())
}

fn project_pending_entries(
    passbook: &Passbook,
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
                .map(|spent| passbook.owns(&spent.output.locking_condition))
                .unwrap_or(false)
        });

        let mut has_user_output = false;
        for (idx, output) in tx.outputs.iter().enumerate() {
            if passbook.owns(&output.locking_condition) {
                has_user_output = true;
                let op = OutPoint::new(txid, idx as u32);
                let parsed = parse_locking_condition(&output.locking_condition);

                let (asset, action, amount, datum_hash) = match parsed {
                    ParsedContractCondition::Scy20(scy20) => {
                        let tid = Hash256::new(scy20.token_id);
                        let act = if owns_input {
                            PassbookAction::Scy20Transfer
                        } else {
                            PassbookAction::Scy20Mint
                        };
                        (
                            PassbookAsset::Scy20 { token_id: tid },
                            act,
                            scy20.amount as u64,
                            None,
                        )
                    }
                    ParsedContractCondition::Vault(vault) => (
                        PassbookAsset::Native,
                        PassbookAction::VaultDeposit {
                            timelock_until: vault.unlock_time,
                        },
                        output.value,
                        Some(Hash256::hash(
                            &bincode::serialize(&vault).unwrap_or_default(),
                        )),
                    ),
                    ParsedContractCondition::GenericScript { datum_hash } => (
                        PassbookAsset::Native,
                        PassbookAction::ContractInteraction { datum_hash },
                        output.value,
                        datum_hash,
                    ),
                    ParsedContractCondition::Standard => {
                        let act = if owns_input {
                            PassbookAction::Change
                        } else {
                            PassbookAction::Received
                        };
                        (PassbookAsset::Native, act, output.value, None)
                    }
                };

                out.push(PassbookEntry {
                    entry_number: 0,
                    timestamp,
                    asset,
                    action,
                    amount_quanta: amount,
                    fee_quanta: entry.fee,
                    status: EntryStatus::Pending,
                    txid,
                    outpoint: Some(op),
                    block_height: None,
                    datum_hash,
                });
            }
        }

        if !has_user_output && owns_input {
            let total_output = tx.total_output_quanta().unwrap_or(0);
            out.push(PassbookEntry {
                entry_number: 0,
                timestamp,
                asset: PassbookAsset::Native,
                action: PassbookAction::Sent,
                amount_quanta: entry.fee.saturating_add(total_output),
                fee_quanta: entry.fee,
                status: EntryStatus::Pending,
                txid,
                outpoint: None,
                block_height: None,
                datum_hash: None,
            });
        }
    }
}

fn assign_entry_numbers(entries: &mut [PassbookEntry]) {
    entries.sort_by_key(|e| (e.block_height, e.idx()));
    for (i, entry) in entries.iter_mut().enumerate() {
        entry.entry_number = (i + 1) as u64;
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
        assert_eq!(sum_native_owned(&p, &utxos).unwrap(), 0);
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
        assert_eq!(sum_native_owned(&p, &utxos).unwrap(), 600);
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
        let p = Passbook::new(vec![lock]);

        let delta = pending_delta(&p, &confirmed, &pending).unwrap();
        assert_eq!(delta, -400_000);
    }
}
