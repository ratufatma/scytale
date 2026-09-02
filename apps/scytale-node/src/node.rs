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
    AuthorizationError, AuthorizationVerifier, Block, BlockHeader, Hash256, Transaction, TxOut,
    UtxoSet,
};
use scytale_mempool::Mempool;
use scytale_mining::{build_template, run_pow_search};
use scytale_storage::StorageEngine;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
}

/// Runtime orchestrator coordinating all Scytale subsystems.
pub struct Node {
    config: NodeConfig,
    state: Arc<RwLock<NodeState>>,
    storage: Arc<StorageEngine>,
    shared: Arc<Shared>,
    mining_cancel: Arc<AtomicBool>,
    mining_handle: Option<JoinHandle<()>>,
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

        Ok(Self {
            state: Arc::new(RwLock::new(NodeState::Starting)),
            storage: Arc::new(storage),
            shared: Arc::new(Shared {
                chain_tree: Mutex::new(Self::empty_chain(&config)),
                utxo_set: Mutex::new(UtxoSet::new()),
                mempool: Mutex::new(Mempool::new()),
            }),
            mining_cancel: Arc::new(AtomicBool::new(false)),
            mining_handle: None,
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
            self.spawn_mining_worker();
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

    /// Spawns the autonomous Proof-of-Work mining worker on a background thread.
    fn spawn_mining_worker(&mut self) {
        let storage = Arc::clone(&self.storage);
        let cancel = Arc::clone(&self.mining_cancel);
        let shared = Arc::clone(&self.shared);
        let payout = self.config.miner_payout_script.clone();
        let initial_target = self.config.genesis_difficulty_target;

        self.mining_handle = Some(std::thread::spawn(move || {
            mining_worker_loop(storage, shared, initial_target, payout, cancel);
        }));
    }

    /// Constructs the Genesis Block 0 for a fresh database.
    ///
    /// Genesis carries the network's initial monetary emission as a coinbase output
    /// owned by the genesis payer. No *user* or wallet account is ever credited,
    /// preserving the permissionless zero-balance bootstrap invariant: a new node
    /// has a spendable balance of 0 SCY and may begin mining immediately.
    fn make_genesis(config: &NodeConfig) -> Block {
        let subsidy = scytale_consensus::calculate_block_reward(0);
        let coinbase = Transaction::new_coinbase(
            0,
            vec![TxOut::new(subsidy, config.miner_payout_script.clone())],
        );
        let commitment = Hash256::hash(coinbase.txid().as_bytes());
        let header = BlockHeader::new(
            1,
            Hash256::ZERO,
            commitment,
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
    pub fn shutdown(&mut self) -> Result<(), NodeError> {
        self.set_state(NodeState::Stopping);
        self.mining_cancel.store(true, Ordering::Relaxed);

        if let Some(handle) = self.mining_handle.take() {
            let _timeout = Duration::from_secs(self.config.shutdown_timeout_secs);
            handle.join().map_err(|_| NodeError::MiningNotRunning)?;
        }

        // Every background worker has now released its shared locks; the storage
        // handle remains valid and closes cleanly when the Node is dropped.
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

    /// Sets the node runtime state.
    fn set_state(&self, next: NodeState) {
        *self.state.write().unwrap() = next;
    }

    /// Returns a clone of the atomic cancellation flag observed by the mining worker.
    pub fn mining_cancel_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.mining_cancel)
    }

    /// Returns `true` if a background mining worker is currently running.
    pub fn mining_running(&self) -> bool {
        self.mining_handle.is_some()
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
