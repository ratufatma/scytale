//! Node runtime orchestrator: coordinates Storage, Consensus, UTXO, Mempool, and Mining
//! subsystems through a deterministic lifecycle state machine.
//!
//! Architectural invariants:
//! - The node *coordinates* subsystems; it never invents consensus or monetary rules.
//! - Every block state transition is committed atomically to `redb` storage.
//! - Shutdown cancels background workers *before* the database handle is released.

use crate::config::NodeConfig;
use crate::error::{NodeError, NodeState};
use scytale_core::{
    AuthorizationError, AuthorizationVerifier, Block, BlockHeader, Hash256, OutPoint, Transaction,
    TxOut, UtxoSet, EutxoValidationError, verify_transaction_eutxo, MAX_TX_GAS, MAX_BLOCK_GAS,
};
use scytale_mempool::{Mempool, MempoolEntry};
use scytale_mining::{build_template, run_pow_search};
use scytale_storage::StorageEngine;
use std::path::Path;
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex, RwLock,
};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use scytale_bridge::{P2pBridgeEvent, UtxoWireEntryDto};
use scytale_core::codec::CanonicalSerialize;
use tokio::sync::broadcast;

/// Upper bound on the nonce space searched per template before refresh.
const MAX_NONCE_ITERATIONS: u64 = 1_000_000;

/// Permissionless verifier used during mempool reconciliation.
///
/// Scytale's mempool admission pipeline delegates cryptographic authorization to
/// pluggable verifiers. This default accepts any proof, enabling zero-balance
/// bootstrap and deterministic lifecycle tests without requiring real signatures.
#[derive(Clone, Copy, Debug)]
pub struct PermissiveVerifier;

impl AuthorizationVerifier for PermissiveVerifier {
    fn verify(
        &self,
        _preimage_digest: &Hash256,
        _locking_condition: &[u8],
        _authorization_proof: &[u8],
    ) -> Result<(), AuthorizationError> {
        Ok(())
    }
}

/// A running node's shared, contemporaneously-mutated subsystem state.
///
/// The mining worker and the main node thread access these locks concurrently;
/// each critical-section hold is kept short (never spanning a PoW nonce search).
struct Shared {
    chain_tree: Mutex<scytale_consensus::ChainTree>,
    utxo_set: Mutex<UtxoSet>,
    mempool: Mutex<Mempool>,
    p2p_event_tx: broadcast::Sender<P2pBridgeEvent>,
}

/// Runtime orchestrator coordinating all Scytale subsystems.
pub struct Node {
    config: NodeConfig,
    state: Arc<RwLock<NodeState>>,
    storage: Arc<StorageEngine>,
    shared: Arc<Shared>,
    mining_cancel: Arc<AtomicBool>,
    mining_handle: Mutex<Option<JoinHandle<()>>>,
    peer_count: Arc<AtomicUsize>,
}

#[allow(clippy::result_large_err)]
impl Node {
    /// Opens the embedded storage and constructs an orchestrator in `Starting` state.
    pub fn open(config: NodeConfig) -> Result<Self, NodeError> {
        let storage: StorageEngine = if config.data_dir.as_os_str() == ":memory:" {
            StorageEngine::in_memory()?
        } else {
            if let Some(parent) = Path::new(&config.data_dir).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        NodeError::InconsistentState(format!(
                            "failed to create data_dir parent: {e}"
                        ))
                    })?;
                }
            }
            std::fs::create_dir_all(&config.data_dir).map_err(|e| {
                NodeError::InconsistentState(format!("failed to create data_dir: {e}"))
            })?;
            // The embedded `redb` database lives at a file inside the data directory.
            StorageEngine::open(config.data_dir.join("scytale.db"))?
        };

        let (p2p_event_tx, _rx) = broadcast::channel(128);

        Ok(Self {
            state: Arc::new(RwLock::new(NodeState::Starting)),
            storage: Arc::new(storage),
            shared: Arc::new(Shared {
                chain_tree: Mutex::new(Self::empty_chain(&config)),
                utxo_set: Mutex::new(UtxoSet::new()),
                mempool: Mutex::new(Mempool::new()),
                p2p_event_tx,
            }),
            mining_cancel: Arc::new(AtomicBool::new(false)),
            mining_handle: Mutex::new(None),
            peer_count: Arc::new(AtomicUsize::new(0)),
            config,
        })
    }

    /// Runs the full deterministic startup sequence and returns in `Ready`/`Running` state.
    pub fn start(&mut self) -> Result<(), NodeError> {
        self.set_state(NodeState::Initializing);
        self.recover()?;

        self.set_state(NodeState::Syncing);
        // P2P bridge/IBD is a transport-level concern; a standalone node with no peers
        // is already fully synchronized with its local canonical chain.
        self.set_state(NodeState::Ready);

        if self.config.mining_enabled {
            self.start_mining()?;
        }
        self.set_state(NodeState::Running);
        Ok(())
    }

    /// Restores persisted canonical state (blocks, tip, height, UTXO set) from disk.
    fn recover(&self) -> Result<(), NodeError> {
        let tip = self.storage.get_canonical_tip()?;

        // Fresh database: initialize Genesis Block 0 and apply its emission to the
        // in-memory UTXO set, guaranteeing the permissionless zero-balance bootstrap
        // invariant (the genesis value is owned by the genesis payer, never a user).
        if tip.is_none() {
            let genesis = Self::make_genesis(&self.config);
            let mut chain_tree = self.shared.chain_tree.lock().unwrap();
            *chain_tree = scytale_consensus::ChainTree::new(genesis.clone());
            let tip_height = chain_tree.canonical_height();
            let work = chain_tree.canonical_work().0;
            self.storage.commit_block(&genesis, tip_height, work)?;

            let mut utxo_set = UtxoSet::new();
            utxo_set
                .apply_coinbase(&genesis.transactions[0], 0)
                .map_err(|e| NodeError::InconsistentState(format!("genesis utxo: {e}")))?;
            let mut utxo_guard = self.shared.utxo_set.lock().unwrap();
            *utxo_guard = utxo_set;
            return Ok(());
        }

        let (tip_hash, tip_height) = tip.unwrap();

        // Rebuild the canonical path from genesis to tip by walking parent links.
        let mut path_rev: Vec<Block> = Vec::new();
        let mut cur = tip_hash;
        loop {
            let block = self.storage.get_block(&cur)?.ok_or_else(|| {
                NodeError::InconsistentState("missing block on canonical path".into())
            })?;
            path_rev.push(block.clone());
            if block.header.previous_block_hash == Hash256::ZERO
                || block.header.previous_block_hash == cur
            {
                break;
            }
            cur = block.header.previous_block_hash;
        }
        path_rev.reverse();

        if path_rev.len() as u64 != tip_height + 1 {
            return Err(NodeError::InconsistentState(
                "canonical path length does not match persisted tip height".into(),
            ));
        }

        // Replay blocks into a fresh chain tree, rebuilding the canonical UTXO set.
        let mut tree = scytale_consensus::ChainTree::new(path_rev[0].clone());
        let mut utxo_set = UtxoSet::new();
        utxo_set
            .apply_coinbase(&path_rev[0].transactions[0], 0)
            .map_err(|e| NodeError::InconsistentState(format!("genesis utxo: {e}")))?;
        for block in &path_rev[1..] {
            tree.process_block(block.clone(), &mut utxo_set)?;
        }

        {
            let mut chain_tree = self.shared.chain_tree.lock().unwrap();
            *chain_tree = tree;
        }
        {
            let mut utxo_guard = self.shared.utxo_set.lock().unwrap();
            *utxo_guard = utxo_set;
        }
        {
            let mut mempool = self.shared.mempool.lock().unwrap();
            *mempool = Mempool::new();
        }

        Ok(())
    }

    /// Starts the autonomous Proof-of-Work mining worker on a background thread if not already running.
    pub fn start_mining(&self) -> Result<bool, NodeError> {
        let mut handle_guard = self.mining_handle.lock().unwrap();
        if handle_guard.is_some() {
            return Ok(false);
        }
        self.mining_cancel.store(false, Ordering::Relaxed);
        let storage = Arc::clone(&self.storage);
        let cancel = Arc::clone(&self.mining_cancel);
        let shared = Arc::clone(&self.shared);
        let payout = self.config.miner_payout_script.clone();
        let initial_target = self.config.genesis_difficulty_target;

        *handle_guard = Some(std::thread::spawn(move || {
            mining_worker_loop(storage, shared, initial_target, payout, cancel);
        }));
        Ok(true)
    }

    /// Stops the background Proof-of-Work mining worker if running.
    pub fn stop_mining(&self) -> Result<bool, NodeError> {
        self.mining_cancel.store(true, Ordering::Relaxed);
        let handle = self.mining_handle.lock().unwrap().take();
        if let Some(h) = handle {
            h.join().map_err(|_| NodeError::MiningNotRunning)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Constructs the Genesis Block 0 for a fresh database.
    ///
    /// Genesis carries the network's initial monetary emission as a coinbase output
    /// owned by the genesis payer. No *user* or wallet account is ever credited,
    /// preserving the permissionless zero-balance bootstrap invariant: a new node
    /// has a spendable balance of 0 SCY and may begin mining immediately.
    fn make_genesis(config: &NodeConfig) -> Block {
        let subsidy = scytale_consensus::calculate_block_reward(0);
        let coinbase =
            Transaction::new_coinbase(0, vec![TxOut::new(subsidy, vec![0x01, 0x02, 0x03])]);
        let commitment = Hash256::hash(coinbase.txid().as_bytes());
        let genesis_outpoint = OutPoint::new(coinbase.txid(), 0);
        let genesis_utxo_root =
            scytale_core::compute_utxo_leaf(&genesis_outpoint, &coinbase.outputs[0]);
        let header = BlockHeader::new(
            1,
            Hash256::ZERO,
            commitment,
            genesis_utxo_root,
            0,
            config.genesis_difficulty_target,
            0,
        );
        Block::new(header, vec![coinbase])
    }

    /// Returns a fresh chain tree seeded with a placeholder genesis (used before recovery).
    fn empty_chain(config: &NodeConfig) -> scytale_consensus::ChainTree {
        scytale_consensus::ChainTree::new(Self::make_genesis(config))
    }

    /// Gracefully shuts the node down, cancelling workers before releasing storage.
    pub fn shutdown(&self) -> Result<(), NodeError> {
        self.set_state(NodeState::Stopping);
        self.stop_mining()?;
        self.set_state(NodeState::Stopped);
        Ok(())
    }

    /// Submits an externally-originated block into the consensus pipeline.
    ///
    /// Returns `Ok(true)` if the block became the canonical tip (and any stale mining
    /// template is therefore invalidated), `Ok(false)` if it was a side branch or
    /// duplicate, and propagates consensus errors for invalid blocks.
    pub fn submit_external_block(&self, block: Block) -> Result<bool, NodeError> {
        let canonical_after = {
            let mut chain = self.shared.chain_tree.lock().unwrap();
            let mut utxos = self.shared.utxo_set.lock().unwrap();

            // Verify non-coinbase transaction scripts against current UTXO set if extending canonical tip
            if block.header.previous_block_hash == chain.canonical_tip() {
                let height = chain.canonical_height() + 1;
                let block_time = block.header.timestamp;
                let mut staging_utxos = utxos.clone();
                let mut block_gas_consumed: u64 = 0;
                for tx in &block.transactions {
                    if !tx.is_coinbase() {
                        Self::verify_transaction_scripts(tx, height, &staging_utxos)?;
                        // eUTXO ScyVM validation
                        let tx_gas = verify_transaction_eutxo(
                            tx,
                            block_time,
                            &staging_utxos,
                            MAX_TX_GAS,
                        ).map_err(NodeError::EutxoValidation)?;
                        block_gas_consumed = block_gas_consumed.saturating_add(tx_gas);
                        if block_gas_consumed > MAX_BLOCK_GAS {
                            return Err(NodeError::EutxoValidation(
                                EutxoValidationError::BlockGasLimitExceeded {
                                    consumed: block_gas_consumed,
                                    limit: MAX_BLOCK_GAS,
                                },
                            ));
                        }
                        for input in &tx.inputs {
                            staging_utxos.remove(&input.previous_output);
                        }
                    }
                    let txid = tx.txid();
                    for (idx, output) in tx.outputs.iter().enumerate() {
                        if output.locking_condition.first() != Some(&0x6a) {
                            let op = OutPoint::new(txid, idx as u32);
                            staging_utxos.insert(
                                op,
                                scytale_core::UtxoEntry::new(
                                    output.clone(),
                                    height,
                                    tx.is_coinbase(),
                                ),
                            );
                        }
                    }
                }
                let calculated_root = staging_utxos.compute_utxo_root();
                if block.header.utxo_root != calculated_root {
                    return Err(NodeError::Consensus(
                        scytale_consensus::ChainError::BlockError(
                            scytale_core::BlockError::InvalidUtxoRoot {
                                expected: block.header.utxo_root,
                                actual: calculated_root,
                            },
                        ),
                    ));
                }
            }

            match chain.process_block(block.clone(), &mut utxos) {
                Ok(Some(reorg)) => {
                    let height = chain.canonical_height();
                    let work = chain.canonical_work().0;
                    if reorg.disconnected_blocks.is_empty() {
                        self.storage.commit_block(&block, height, work)?;
                    } else {
                        let connected_meta = reorg
                            .connected_blocks
                            .iter()
                            .map(|b| {
                                let n = chain.get_node(&b.header.hash());
                                (
                                    b.clone(),
                                    n.map(|x| x.height).unwrap_or(height),
                                    n.map(|x| x.cumulative_work.0).unwrap_or(work),
                                )
                            })
                            .collect::<Vec<_>>();
                        self.storage
                            .apply_reorganization(&reorg.disconnected_blocks, &connected_meta)?;
                        let mut mempool = self.shared.mempool.lock().unwrap();
                        let verifier = PermissiveVerifier;
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        mempool.on_reorg(reorg.transactions_for_mempool, &utxos, &verifier, now);
                        drop(mempool);
                    }
                    let mut mempool = self.shared.mempool.lock().unwrap();
                    mempool.on_block_connected(&block, &utxos);
                    drop(mempool);
                    true
                }
                Ok(None) => false,
                Err(e) => return Err(NodeError::Consensus(e)),
            }
        };
        Ok(canonical_after)
    }

    /// Returns a shared handle to the embedded storage for downstream inspection.
    pub fn storage_handle(&self) -> Arc<StorageEngine> {
        Arc::clone(&self.storage)
    }

    /// The active canonical tip hash.
    pub fn canonical_tip(&self) -> Hash256 {
        self.shared.chain_tree.lock().unwrap().canonical_tip()
    }

    /// The active canonical chain height.
    pub fn canonical_height(&self) -> u64 {
        self.shared.chain_tree.lock().unwrap().canonical_height()
    }

    /// The current node runtime state.
    pub fn state(&self) -> NodeState {
        self.state.read().unwrap().clone()
    }

    /// The read-only sum of all unspent output values (integer quanta).
    pub fn total_utxo_quanta(&self) -> u64 {
        self.shared
            .utxo_set
            .lock()
            .unwrap()
            .total_quanta()
            .unwrap_or(0)
    }

    /// Returns the number of pending mempool transactions.
    pub fn mempool_len(&self) -> usize {
        self.shared.mempool.lock().unwrap().len()
    }

    /// Returns the total canonical serialized byte size of all mempool transactions.
    pub fn mempool_total_bytes(&self) -> usize {
        self.shared.mempool.lock().unwrap().total_bytes()
    }

    /// Returns the aggregate fees of all mempool transactions in quanta.
    pub fn mempool_total_fees(&self) -> u64 {
        self.shared.mempool.lock().unwrap().total_fees()
    }

    /// Sets the node runtime state.
    fn set_state(&self, next: NodeState) {
        *self.state.write().unwrap() = next;
    }

    /// Returns a clone of the atomic cancellation flag observed by the mining worker.
    pub fn mining_cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.mining_cancel)
    }

    /// Returns a reference to the active node configuration.
    pub fn config(&self) -> &NodeConfig {
        &self.config
    }

    /// Returns `true` if a background mining worker is currently running.
    pub fn mining_running(&self) -> bool {
        self.mining_handle.lock().unwrap().is_some()
    }

    // ─────────────────────────────────────────────────────────────────────
    // Read-only query interface (consumed by the Passbook presentation layer
    // and other downstream read clients). None of these mutate ledger state.
    // ─────────────────────────────────────────────────────────────────────

    /// Returns a snapshot copy of the active canonical UTXO set.
    pub fn query_utxo_set(&self) -> UtxoSet {
        self.shared.utxo_set.lock().unwrap().clone()
    }

    /// Returns a snapshot of all unconfirmed mempool entries.
    pub fn query_mempool(&self) -> Vec<MempoolEntry> {
        self.shared
            .mempool
            .lock()
            .unwrap()
            .get_entries_sorted_by_fee_rate()
    }

    /// Walks the persisted canonical chain from genesis to tip, returning every
    /// confirmed block plus its canonical height. Pure read; never mutates state.
    pub fn query_canonical_chain(&self) -> Result<Vec<(Block, u64)>, NodeError> {
        let (tip_hash, tip_height) = self
            .storage
            .get_canonical_tip()?
            .ok_or_else(|| NodeError::InconsistentState("no canonical tip".into()))?;

        let mut path_rev: Vec<(Block, u64)> = Vec::new();
        let mut cur = tip_hash;
        let mut height = tip_height;
        loop {
            let block = self.storage.get_block(&cur)?.ok_or_else(|| {
                NodeError::InconsistentState("missing block on canonical path".into())
            })?;
            path_rev.push((block.clone(), height));
            if height == 0
                || block.header.previous_block_hash == Hash256::ZERO
                || block.header.previous_block_hash == cur
            {
                break;
            }
            cur = block.header.previous_block_hash;
            height -= 1;
        }
        path_rev.reverse();
        Ok(path_rev)
    }

    /// Looks up a confirmed transaction by TxID through the embedded storage.
    pub fn lookup_transaction(&self, txid: &Hash256) -> Result<Option<Transaction>, NodeError> {
        Ok(self.storage.get_transaction(txid)?)
    }

    /// Subscribes to asynchronous P2P network broadcast events (mined blocks, admitted transactions).
    pub fn subscribe_p2p_events(&self) -> broadcast::Receiver<P2pBridgeEvent> {
        self.shared.p2p_event_tx.subscribe()
    }

    /// Computes the block locator hashes (exponential spacing from canonical tip to genesis)
    /// used to negotiate Initial Block Download (IBD) synchronization with peers.
    pub fn get_block_locator(&self) -> Result<Vec<Hash256>, NodeError> {
        let chain = self.query_canonical_chain()?;
        if chain.is_empty() {
            return Ok(vec![Hash256::ZERO]);
        }
        let mut locator = Vec::new();
        let tip_idx = chain.len() - 1;
        let mut step = 1;
        let mut cur = tip_idx;
        loop {
            let hash = chain[cur].0.header.hash();
            if !locator.contains(&hash) {
                locator.push(hash);
            }
            if cur == 0 {
                break;
            }
            if cur >= step {
                cur -= step;
            } else {
                cur = 0;
            }
            if locator.len() > 10 {
                step *= 2;
            }
        }
        let genesis_hash = chain[0].0.header.hash();
        if !locator.contains(&genesis_hash) {
            locator.push(genesis_hash);
        }
        Ok(locator)
    }

    /// Returns all canonical block hashes in ascending order from genesis to tip.
    pub fn get_canonical_hashes(&self) -> Result<Vec<Hash256>, NodeError> {
        let chain = self.query_canonical_chain()?;
        Ok(chain.into_iter().map(|(b, _)| b.header.hash()).collect())
    }

    /// Emits a dynamic peer connection command to the supervised P2P daemon.
    pub fn connect_peer(&self, addr: String) {
        let _ = self
            .shared
            .p2p_event_tx
            .send(P2pBridgeEvent::ConnectPeer { addr });
    }

    /// Returns the number of currently connected network peers.
    pub fn peer_count(&self) -> usize {
        self.peer_count.load(Ordering::Relaxed)
    }

    /// Sets the number of currently connected network peers.
    pub fn set_peer_count(&self, count: usize) {
        self.peer_count.store(count, Ordering::Relaxed);
    }

    /// Exports a paginated chunk of the authenticated UTXO snapshot.
    pub fn export_snapshot_chunk(
        &self,
        block_hash_hex: &str,
        chunk_index: u32,
        chunk_size: u32,
    ) -> Result<(String, u32, u32, Vec<UtxoWireEntryDto>), NodeError> {
        let snapshot = self
            .storage
            .export_utxo_snapshot()
            .map_err(NodeError::Storage)?;
        let chunk_size = if chunk_size == 0 { 2000 } else { chunk_size };
        let total_entries = snapshot.entries.len();
        let total_chunks = if total_entries == 0 {
            1
        } else {
            (total_entries as u32).div_ceil(chunk_size)
        };

        let start = (chunk_index as usize).saturating_mul(chunk_size as usize);
        let entries = if start >= total_entries {
            Vec::new()
        } else {
            let end = (start + chunk_size as usize).min(total_entries);
            snapshot.entries[start..end]
                .iter()
                .map(|(outpoint, entry)| UtxoWireEntryDto {
                    txid_hex: outpoint.txid.to_string(),
                    index: outpoint.index,
                    value_quanta: entry.output.value,
                    locking_script_hex: scytale_primitives::to_hex(&entry.output.locking_condition),
                })
                .collect()
        };

        let target_hash_hex =
            if block_hash_hex.is_empty() || block_hash_hex == Hash256::ZERO.to_string() {
                snapshot.block_hash.to_string()
            } else {
                block_hash_hex.to_string()
            };

        Ok((target_hash_hex, chunk_index, total_chunks, entries))
    }

    /// Verifies and applies an authenticated UTXO snapshot to the node's storage and active state.
    pub fn apply_snapshot(
        &self,
        block_hash_hex: &str,
        entries: &[UtxoWireEntryDto],
    ) -> Result<usize, NodeError> {
        let block_hash = Hash256::from_str(block_hash_hex)
            .map_err(|e| NodeError::InconsistentState(format!("invalid block hash: {e}")))?;

        let mut snapshot_entries = Vec::with_capacity(entries.len());
        for e in entries {
            let txid = Hash256::from_str(&e.txid_hex)
                .map_err(|err| NodeError::InconsistentState(format!("invalid txid hex: {err}")))?;
            let locking_condition =
                scytale_primitives::from_hex(&e.locking_script_hex).map_err(|err| {
                    NodeError::InconsistentState(format!("invalid locking script hex: {err}"))
                })?;
            let outpoint = OutPoint::new(txid, e.index);
            let output = scytale_core::TxOut::new(e.value_quanta, locking_condition);
            let entry = scytale_core::UtxoEntry::new(output, 0, false);
            snapshot_entries.push((outpoint, entry));
        }

        snapshot_entries.sort_by(|(a_op, _), (b_op, _)| {
            a_op.txid
                .cmp(&b_op.txid)
                .then_with(|| a_op.index.cmp(&b_op.index))
        });

        let leaves: Vec<Hash256> = snapshot_entries
            .iter()
            .map(|(op, entry)| scytale_core::compute_utxo_leaf(op, &entry.output))
            .collect();
        let calculated_root = scytale_core::compute_utxo_merkle_root(leaves);

        if let Ok(Some(block)) = self.storage.get_block(&block_hash) {
            if block.header.utxo_root != calculated_root {
                return Err(NodeError::InconsistentState(format!(
                    "Snapshot utxo_root mismatch with block {}: expected {}, calculated {}",
                    block_hash, block.header.utxo_root, calculated_root
                )));
            }
        }

        let snapshot = scytale_storage::UtxoSnapshotDto {
            height: 0,
            block_hash,
            utxo_root: calculated_root,
            entries: snapshot_entries.clone(),
        };

        self.storage
            .apply_utxo_snapshot(&snapshot)
            .map_err(NodeError::Storage)?;

        {
            let mut utxo_set = self.shared.utxo_set.lock().unwrap();
            *utxo_set = scytale_core::UtxoSet::new();
            for (outpoint, entry) in snapshot_entries {
                utxo_set.insert(outpoint, entry);
            }
        }

        {
            let mut mempool = self.shared.mempool.lock().unwrap();
            *mempool = Mempool::new();
        }

        Ok(entries.len())
    }

    /// Verifies that all inputs of a transaction satisfy the locking conditions of their
    /// referenced UTXOs using `ScriptEngine` and `compute_sighash`.
    pub fn verify_transaction_scripts(
        tx: &Transaction,
        block_height: u64,
        utxos: &UtxoSet,
    ) -> Result<(), NodeError> {
        if tx.is_coinbase() {
            return Ok(());
        }

        let engine = scytale_script::ScriptEngine::default();
        for (input_idx, input) in tx.inputs.iter().enumerate() {
            let utxo = utxos.get(&input.previous_output).ok_or_else(|| {
                NodeError::InconsistentState(format!(
                    "Missing UTXO for input {:?}:{:?}",
                    input.previous_output.txid, input.previous_output.index
                ))
            })?;

            let sighash = tx.compute_sighash(input_idx, &utxo.output.locking_condition);
            let ctx = scytale_script::ScriptContext::new(&sighash, block_height);

            let valid = engine
                .execute(&input.authorization, &utxo.output.locking_condition, &ctx)
                .map_err(|e| NodeError::InvalidScript(e.to_string()))?;

            if !valid {
                return Err(NodeError::ScriptEvaluationFailed);
            }
        }

        Ok(())
    }

    /// Admits a new transaction into the local mempool through the admission
    /// pipeline (stateless validation, double-spend and in-flight checks, UTXO
    /// resolution, authorization, and value conservation). Read-only with respect
    /// to the canonical ledger; the transaction remains unconfirmed until mined.
    pub fn submit_transaction(&self, tx: Transaction) -> Result<Hash256, NodeError> {
        let height = self.canonical_height();
        let utxos = self.shared.utxo_set.lock().unwrap();
        Self::verify_transaction_scripts(&tx, height, &utxos)?;
        // eUTXO ScyVM validation gate
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        verify_transaction_eutxo(&tx, now, &utxos, MAX_TX_GAS)
            .map_err(NodeError::EutxoValidation)?;
        let mut mempool = self.shared.mempool.lock().unwrap();
        let verifier = PermissiveVerifier;
        let txid = mempool.admit_transaction(tx.clone(), &utxos, &verifier, now)?;

        if let Ok(bytes) = tx.to_canonical_bytes() {
            let _ = self
                .shared
                .p2p_event_tx
                .send(P2pBridgeEvent::BroadcastTransaction {
                    tx_hex: scytale_primitives::to_hex(&bytes),
                    txid_hex: txid.to_string(),
                });
        }

        Ok(txid)
    }

    /// Constructs a `BlockTemplate` using the current canonical chain, UTXO set, and mempool.
    pub fn build_mining_template(
        &self,
        miner_payout_script: Vec<u8>,
    ) -> Result<scytale_mining::BlockTemplate, NodeError> {
        let chain = self.shared.chain_tree.lock().unwrap();
        if chain.is_empty() {
            return Err(NodeError::InconsistentState("chain tree is empty".into()));
        }
        let tip = chain.canonical_tip();
        let compact_target = chain
            .get_node(&tip)
            .map(|n| n.block.header.difficulty_target)
            .unwrap_or(0x1d00ffff);
        let utxos = self.shared.utxo_set.lock().unwrap();
        let mempool = self.shared.mempool.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        build_template(
            &chain,
            &utxos,
            &mempool,
            compact_target,
            miner_payout_script,
            now,
        )
        .map_err(NodeError::Mining)
    }

    /// Constructs a standard transfer transaction by selecting UTXOs matching
    /// `sender_script` (or default miner payout script), generating a recipient output
    /// and a change output, then submitting it into the node mempool.
    pub fn create_and_submit_transaction(
        &self,
        recipient_script: Vec<u8>,
        amount_quanta: u64,
        fee_quanta: u64,
        sender_script: Option<Vec<u8>>,
    ) -> Result<Hash256, NodeError> {
        if amount_quanta == 0 {
            return Err(NodeError::InconsistentState(
                "Transaction amount must be greater than zero".into(),
            ));
        }
        let total_needed = amount_quanta
            .checked_add(fee_quanta)
            .ok_or_else(|| NodeError::InconsistentState("Amount plus fee overflow".into()))?;

        let sender = sender_script.unwrap_or_else(|| self.config.miner_payout_script.clone());

        let utxos_snapshot = self.query_utxo_set();
        let mut selected_inputs = Vec::new();
        let mut accumulated_value: u64 = 0;

        for (outpoint, entry) in utxos_snapshot.entries() {
            if entry.output.locking_condition == sender {
                selected_inputs.push(*outpoint);
                accumulated_value = accumulated_value
                    .checked_add(entry.output.value)
                    .ok_or_else(|| {
                        NodeError::InconsistentState("Value accumulation overflow".into())
                    })?;
                if accumulated_value >= total_needed {
                    break;
                }
            }
        }

        if accumulated_value < total_needed {
            return Err(NodeError::InconsistentState(format!(
                "Insufficient funds: required {} quanta ({} SCY), available {} quanta ({} SCY)",
                total_needed,
                total_needed / scytale_core::QUANTA_PER_SCY,
                accumulated_value,
                accumulated_value / scytale_core::QUANTA_PER_SCY,
            )));
        }

        let inputs = selected_inputs
            .into_iter()
            .map(|op| scytale_core::TxIn::new(op, sender.clone()))
            .collect();

        let mut outputs = vec![TxOut::new(amount_quanta, recipient_script)];
        if accumulated_value > total_needed {
            let change_amount = accumulated_value - total_needed;
            outputs.push(TxOut::new(change_amount, sender));
        }

        let tx = Transaction::new(1, inputs, outputs, 0);
        self.submit_transaction(tx)
    }
}

/// Background mining loop: builds a template, searches for PoW, verifies tip
/// clarity, then atomically commits the solved block to storage.
fn mining_worker_loop(
    storage: Arc<StorageEngine>,
    shared: Arc<Shared>,
    initial_target: u32,
    payout: Vec<u8>,
    cancel: Arc<AtomicBool>,
) {
    let mut compact_target = initial_target;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return;
        }

        // Build a fresh template without holding locks during the PoW search.
        let template = {
            let chain = shared.chain_tree.lock().unwrap();
            if chain.is_empty() {
                return;
            }
            let tip = chain.canonical_tip();
            if let Some(node) = chain.get_node(&tip) {
                compact_target = node.block.header.difficulty_target;
            }

            let utxos = shared.utxo_set.lock().unwrap();
            let mempool = shared.mempool.lock().unwrap();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            match build_template(
                &chain,
                &utxos,
                &mempool,
                compact_target,
                payout.clone(),
                now,
            ) {
                Ok(t) => t,
                Err(_) => {
                    std::thread::sleep(Duration::from_millis(10));
                    continue;
                }
            }
        };

        // Search for a solution without holding any subsystem lock (must not block
        // block-arrival or shutdown while hashing).
        let solved = match run_pow_search(&template, 0, MAX_NONCE_ITERATIONS, &cancel) {
            Ok(h) => h,
            Err(_) => continue, // cancelled or exhausted -> rebuild a fresh template
        };
        if cancel.load(Ordering::Relaxed) {
            return;
        }

        let block = template.assemble_block(solved);

        // Re-acquire locks and confirm the tip is still the template's parent before
        // committing; otherwise the candidate is stale and superseded by an arriving block.
        {
            let mut chain = shared.chain_tree.lock().unwrap();
            let mut utxos = shared.utxo_set.lock().unwrap();
            if chain.canonical_tip() != template.previous_block_hash {
                continue;
            }

            match chain.process_block(block.clone(), &mut utxos) {
                Ok(Some(reorg)) => {
                    let height = template.height;
                    let work = chain.canonical_work().0;
                    if reorg.disconnected_blocks.is_empty() {
                        let _ = storage.commit_block(&block, height, work);
                    } else {
                        let connected_meta = reorg
                            .connected_blocks
                            .iter()
                            .map(|b| {
                                let n = chain.get_node(&b.header.hash());
                                (
                                    b.clone(),
                                    n.map(|x| x.height).unwrap_or(height),
                                    n.map(|x| x.cumulative_work.0).unwrap_or(work),
                                )
                            })
                            .collect::<Vec<_>>();
                        let _ = storage
                            .apply_reorganization(&reorg.disconnected_blocks, &connected_meta);

                        let verifier = PermissiveVerifier;
                        let mut mempool = shared.mempool.lock().unwrap();
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        mempool.on_reorg(reorg.transactions_for_mempool, &utxos, &verifier, now);
                        drop(mempool);
                    }
                    let mut mempool = shared.mempool.lock().unwrap();
                    mempool.on_block_connected(&block, &utxos);
                    drop(mempool);

                    if let Ok(bytes) = block.to_canonical_bytes() {
                        let _ = shared.p2p_event_tx.send(P2pBridgeEvent::BroadcastBlock {
                            block_hex: scytale_primitives::to_hex(&bytes),
                            hash_hex: block.header.hash().to_string(),
                        });
                    }
                }
                Ok(None) => {
                    // Side branch or duplicate: retained in the in-memory tree only.
                }
                Err(_e) => {
                    // Consensus rejected the locally mined block; keep mining.
                }
            }
        }

        if cancel.load(Ordering::Relaxed) {
            return;
        }

        // Yield to the scheduler between blocks so an arriving block can supersede us.
        std::thread::sleep(Duration::from_millis(1));
    }
}
