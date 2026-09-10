use crate::error::{ChainError, ConsensusError};
use crate::get_block_subsidy;
use crate::target::Target;
use crate::work::{block_work, CumulativeWork};
use crate::{difficulty, pow};
use scytale_core::{Block, Hash256, Transaction, UtxoSet};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_FUTURE_DRIFT: u64 = 7200;

/// Recomputes the canonical BLAKE3 commitment over the concatenated TxIDs.
pub fn transaction_commitment(block: &Block) -> Hash256 {
    let mut bytes = Vec::with_capacity(block.transactions.len() * 32);
    for tx in &block.transactions {
        bytes.extend_from_slice(tx.txid().as_bytes());
    }
    Hash256::hash(&bytes)
}

/// Calculates Median Time Past from the canonical headers preceding an active block.
pub fn calculate_median_time_past(
    headers: &[scytale_core::BlockHeader],
    current_index: usize,
) -> u64 {
    let end = current_index.min(headers.len());
    let start = end.saturating_sub(11);
    let mut timestamps: Vec<u64> = headers[start..end]
        .iter()
        .map(|header| header.timestamp)
        .collect();
    if timestamps.is_empty() {
        return 0;
    }
    timestamps.sort_unstable();
    timestamps[timestamps.len() / 2]
}

fn validate_lock_time(tx: &Transaction, block_height: u64, mtp: u64) -> Result<(), ConsensusError> {
    if tx.lock_time == 0 || tx.inputs.iter().all(|input| input.sequence == u32::MAX) {
        return Ok(());
    }
    if tx.lock_time < 500_000_000 {
        if tx.lock_time > block_height {
            return Err(ConsensusError::LockTimeNotMet);
        }
    } else if tx.lock_time > mtp {
        return Err(ConsensusError::LockTimeNotMet);
    }
    Ok(())
}

/// Performs consensus checks that must succeed before a block can mutate UTXO state.
pub fn validate_block_authoritative(
    block: &Block,
    height: u64,
    parent_utxo: &UtxoSet,
) -> Result<u64, ConsensusError> {
    validate_block_authoritative_with_headers(block, height, parent_utxo, &[])
}

pub fn validate_block_authoritative_with_headers(
    block: &Block,
    height: u64,
    parent_utxo: &UtxoSet,
    headers: &[scytale_core::BlockHeader],
) -> Result<u64, ConsensusError> {
    block
        .validate_structure()
        .map_err(|error| ConsensusError::TransactionVerification(error.to_string()))?;

    let expected_commitment = transaction_commitment(block);
    if block.header.transaction_commitment != expected_commitment {
        return Err(ConsensusError::InvalidTransactionCommitment {
            expected: expected_commitment,
            actual: block.header.transaction_commitment,
        });
    }

    let mut staged = parent_utxo.clone();
    let mtp = calculate_median_time_past(headers, headers.len());
    for tx in block.transactions.iter().skip(1) {
        validate_lock_time(tx, height, mtp)?;
    }
    let total_fees = staged
        .apply_block(&block.transactions, height)
        .map_err(|error| ConsensusError::TransactionVerification(error.to_string()))?;
    let coinbase_output = block.transactions[0]
        .total_output_quanta()
        .map_err(|error| ConsensusError::TransactionVerification(error.to_string()))?;
    let subsidy = get_block_subsidy(height);
    let maximum_reward = subsidy.checked_add(total_fees).ok_or_else(|| {
        ConsensusError::TransactionVerification("coinbase reward overflow".into())
    })?;
    if coinbase_output > maximum_reward {
        return Err(ConsensusError::InvalidCoinbaseReward {
            subsidy,
            fees: total_fees,
            output: coinbase_output,
        });
    }
    Ok(total_fees)
}

/// Contextual transaction verifier interface for validating block transactions against a staged UTXO set.
pub trait BlockTransactionVerifier {
    fn verify_block_transactions(
        &self,
        block: &Block,
        utxo_set: &UtxoSet,
    ) -> Result<(), ConsensusError>;
}

impl<F> BlockTransactionVerifier for F
where
    F: Fn(&Block, &UtxoSet) -> Result<(), ConsensusError>,
{
    fn verify_block_transactions(
        &self,
        block: &Block,
        utxo_set: &UtxoSet,
    ) -> Result<(), ConsensusError> {
        self(block, utxo_set)
    }
}

/// A default verifier that accepts all block transactions unconditionally (used when transaction scripts/ScyVM are validated externally).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpTransactionVerifier;

impl BlockTransactionVerifier for NoOpTransactionVerifier {
    fn verify_block_transactions(
        &self,
        _block: &Block,
        _utxo_set: &UtxoSet,
    ) -> Result<(), ConsensusError> {
        Ok(())
    }
}

/// Metadata representation of a block node in the chain tree.
#[derive(Debug, Clone)]
pub struct BlockNode {
    pub hash: Hash256,
    pub parent_hash: Hash256,
    pub height: u64,
    pub block_work: CumulativeWork,
    pub cumulative_work: CumulativeWork,
    pub block: Block,
}

/// Result of a successful reorganization or linear tip progression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReorgResult {
    pub old_tip: Hash256,
    pub new_tip: Hash256,
    pub disconnected_blocks: Vec<Block>,
    pub connected_blocks: Vec<Block>,
    pub transactions_for_mempool: Vec<Transaction>,
}

/// Default maximum allowed reorganization depth (100 blocks).
pub const DEFAULT_MAX_REORG_DEPTH: u64 = 100;

/// In-memory tree tracking all validated blocks, competing forks, and the active canonical tip.
#[derive(Clone)]
pub struct ChainTree {
    nodes: HashMap<Hash256, BlockNode>,
    canonical_tip: Hash256,
    max_reorg_depth: u64,
    invalid_blocks: HashSet<Hash256>,
}

impl ChainTree {
    /// Initializes the chain tree with the genesis block.
    pub fn new(genesis_block: Block) -> Self {
        let genesis_hash = genesis_block.header.hash();
        let target = Target::from_compact(genesis_block.header.difficulty_target);
        let b_work = block_work(&target);

        let genesis_node = BlockNode {
            hash: genesis_hash,
            parent_hash: genesis_block.header.previous_block_hash,
            height: 0,
            block_work: b_work,
            cumulative_work: b_work,
            block: genesis_block,
        };

        let mut nodes = HashMap::new();
        nodes.insert(genesis_hash, genesis_node);

        Self {
            nodes,
            canonical_tip: genesis_hash,
            max_reorg_depth: DEFAULT_MAX_REORG_DEPTH,
            invalid_blocks: HashSet::new(),
        }
    }

    /// Sets the maximum reorganization depth allowed during fork resolution.
    pub fn with_max_reorg_depth(mut self, max_reorg_depth: u64) -> Self {
        self.max_reorg_depth = max_reorg_depth;
        self
    }

    /// Returns the maximum allowed reorganization depth.
    pub fn max_reorg_depth(&self) -> u64 {
        self.max_reorg_depth
    }

    /// Updates the maximum allowed reorganization depth.
    pub fn set_max_reorg_depth(&mut self, max_reorg_depth: u64) {
        self.max_reorg_depth = max_reorg_depth;
    }

    /// Returns the active canonical tip hash.
    pub fn canonical_tip(&self) -> Hash256 {
        self.canonical_tip
    }

    /// Returns the height of the active canonical tip.
    pub fn canonical_height(&self) -> u64 {
        self.nodes
            .get(&self.canonical_tip)
            .map(|n| n.height)
            .unwrap_or(0)
    }

    /// Returns the cumulative Proof-of-Work of the active canonical tip.
    pub fn canonical_work(&self) -> CumulativeWork {
        self.nodes
            .get(&self.canonical_tip)
            .map(|n| n.cumulative_work)
            .unwrap_or(CumulativeWork::zero())
    }

    /// Retrieves a block node by its hash.
    pub fn get_node(&self, hash: &Hash256) -> Option<&BlockNode> {
        self.nodes.get(hash)
    }

    /// Returns the total number of blocks stored in the tree (canonical + side-branches).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns true if the chain tree is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns the set of block hashes marked as invalid by consensus verifiers.
    pub fn invalid_blocks(&self) -> &HashSet<Hash256> {
        &self.invalid_blocks
    }

    /// Returns true if a given block hash has been flagged as invalid.
    pub fn is_block_invalid(&self, hash: &Hash256) -> bool {
        self.invalid_blocks.contains(hash)
    }

    /// Marks a block hash as invalid to prevent future connection or reorganization onto it.
    pub fn mark_block_invalid(&mut self, hash: Hash256) {
        self.invalid_blocks.insert(hash);
    }

    /// Traverses backwards from both tips to find their latest common ancestor.
    pub fn find_common_ancestor(
        &self,
        tip_a: &Hash256,
        tip_b: &Hash256,
    ) -> Result<Hash256, ChainError> {
        if tip_a == tip_b {
            return Ok(*tip_a);
        }

        // Trace all ancestors of tip_a
        let mut ancestors_a = HashSet::new();
        let mut curr_a = *tip_a;
        while let Some(node) = self.nodes.get(&curr_a) {
            ancestors_a.insert(curr_a);
            if node.height == 0 {
                break;
            }
            curr_a = node.parent_hash;
        }

        // Trace tip_b until we find an ancestor in ancestors_a
        let mut curr_b = *tip_b;
        while let Some(node) = self.nodes.get(&curr_b) {
            if ancestors_a.contains(&curr_b) {
                return Ok(curr_b);
            }
            if node.height == 0 {
                break;
            }
            curr_b = node.parent_hash;
        }

        Err(ChainError::CommonAncestorNotFound {
            tip_a: *tip_a,
            tip_b: *tip_b,
        })
    }

    /// Collects the list of blocks from Genesis up to `tip` (inclusive).
    pub fn get_path_from_genesis(&self, tip: &Hash256) -> Result<Vec<BlockNode>, ChainError> {
        let mut path = Vec::new();
        let mut curr = *tip;
        while let Some(node) = self.nodes.get(&curr) {
            path.push(node.clone());
            if node.height == 0 {
                break;
            }
            curr = node.parent_hash;
        }
        path.reverse();
        Ok(path)
    }

    /// Evaluates a new candidate block, updates the block tree, compares cumulative work,
    /// and performs atomic state transitions on the UTXO set if the new block/branch becomes canonical.
    pub fn process_block(
        &mut self,
        block: Block,
        utxo_set: &mut UtxoSet,
    ) -> Result<Option<ReorgResult>, ChainError> {
        self.process_block_with_verifier(block, utxo_set, &NoOpTransactionVerifier)
    }

    /// Evaluates a candidate block using a contextual transaction verifier.
    ///
    /// When a candidate block or fork branch has greater cumulative work than the active tip,
    /// every block in the candidate branch is validated via `verifier.verify_block_transactions`
    /// against the staged UTXO state before the reorganization is committed. If verification
    /// fails, the failing block is recorded into `invalid_blocks` and the reorg is atomically
    /// rejected without altering the canonical tip or UTXO set (fail-closed).
    pub fn process_block_with_verifier<V: BlockTransactionVerifier>(
        &mut self,
        block: Block,
        utxo_set: &mut UtxoSet,
        verifier: &V,
    ) -> Result<Option<ReorgResult>, ChainError> {
        let block_hash = block.header.hash();
        if self.invalid_blocks.contains(&block_hash) {
            return Err(ChainError::InvalidBranchBlock {
                hash: block_hash,
                reason: "Block is marked as invalid".to_string(),
            });
        }

        let parent_hash = block.header.previous_block_hash;
        if self.invalid_blocks.contains(&parent_hash) {
            self.invalid_blocks.insert(block_hash);
            return Err(ChainError::InvalidBranchBlock {
                hash: block_hash,
                reason: "Parent block is marked as invalid".to_string(),
            });
        }

        // 1. Stateless structural validation
        block.validate_structure().map_err(ChainError::BlockError)?;

        if self.nodes.contains_key(&block_hash) {
            return Ok(None);
        }

        // 2. Parent linkage
        let parent_node = self
            .nodes
            .get(&parent_hash)
            .ok_or(ChainError::CorruptedLinkage {
                parent: parent_hash,
            })?
            .clone();

        let height = parent_node.height + 1;

        let mut expected_target = Target::from_compact(parent_node.block.header.difficulty_target);
        if height >= difficulty::DEFAULT_DIFFICULTY_EPOCH_BLOCKS
            && height % difficulty::DEFAULT_DIFFICULTY_EPOCH_BLOCKS == 0
        {
            let epoch_headers = self.get_path_from_genesis(&parent_hash)?;
            let start_index = epoch_headers
                .len()
                .saturating_sub(difficulty::DEFAULT_DIFFICULTY_EPOCH_BLOCKS as usize + 1);
            let start_time = epoch_headers
                .get(start_index)
                .map(|node| node.block.header.timestamp)
                .ok_or_else(|| ChainError::InvalidTimestamp("missing DAA epoch history".into()))?;
            expected_target = difficulty::calculate_next_target(
                &expected_target,
                start_time,
                parent_node.block.header.timestamp,
                &difficulty::DifficultyConfig::default(),
            )?;
            difficulty::validate_block_target(&block.header, &expected_target)?;
        }
        pow::verify_pow(&block.header, &expected_target).map_err(ChainError::InvalidPoW)?;
        if block.header.timestamp <= parent_node.block.header.timestamp {
            return Err(ChainError::NonMonotonicTimestamp);
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ChainError::InvalidTimestamp("system clock before epoch".into()))?
            .as_secs();
        if block.header.timestamp > now.saturating_add(MAX_FUTURE_DRIFT) {
            return Err(ChainError::BlockTooFarInFuture);
        }

        // Build the parent state before inserting the candidate into the DAG.
        // Fork candidates cannot use the active UTXO tip, so replay their parent
        // path from genesis just as the reorg path does below.
        let parent_utxo = if parent_hash == self.canonical_tip {
            utxo_set.clone()
        } else {
            let mut staged = UtxoSet::new();
            for node in self.get_path_from_genesis(&parent_hash)? {
                staged
                    .apply_block_transactions(
                        &node.block.transactions[0],
                        &node.block.transactions[1..],
                        node.height,
                    )
                    .map_err(|error| ChainError::ReorgFailed {
                        hash: node.hash,
                        error: error.to_string(),
                    })?;
            }
            staged
        };

        // Authoritative validation must happen before any chain-tree or UTXO
        // mutation, including storing a lower-work fork candidate.
        let parent_headers: Vec<_> = self
            .get_path_from_genesis(&parent_hash)?
            .into_iter()
            .map(|node| node.block.header)
            .collect();
        let mtp = calculate_median_time_past(&parent_headers, parent_headers.len());
        if !parent_headers.is_empty() && block.header.timestamp < mtp {
            return Err(ChainError::TimestampBeforeMedianTimePast);
        }
        if let Err(error) =
            validate_block_authoritative_with_headers(&block, height, &parent_utxo, &parent_headers)
        {
            self.invalid_blocks.insert(block_hash);
            return Err(match error {
                ConsensusError::InvalidCoinbaseReward {
                    subsidy,
                    fees,
                    output,
                } => ChainError::InvalidCoinbaseReward {
                    subsidy,
                    fees,
                    output,
                },
                ConsensusError::InvalidTransactionCommitment { expected, actual } => {
                    ChainError::InvalidTransactionCommitment { expected, actual }
                }
                other => ChainError::ReorgFailed {
                    hash: block_hash,
                    error: other.to_string(),
                },
            });
        }

        // 3. Work computation
        let target = Target::from_compact(block.header.difficulty_target);
        let b_work = block_work(&target);
        let cum_work = parent_node
            .cumulative_work
            .checked_add(&b_work)
            .ok_or(ChainError::WorkOverflow)?;

        let new_node = BlockNode {
            hash: block_hash,
            parent_hash,
            height,
            block_work: b_work,
            cumulative_work: cum_work,
            block: block.clone(),
        };

        // 4. Check active canonical tip work
        let active_tip_node = self
            .nodes
            .get(&self.canonical_tip)
            .expect("canonical tip must exist");

        // Equal work or less work -> keep existing canonical tip (first-seen rule)
        if cum_work <= active_tip_node.cumulative_work {
            self.nodes.insert(block_hash, new_node);
            return Ok(None);
        }

        // 5. Candidate branch has greater cumulative work -> Attempt reorg / connection
        let old_tip = self.canonical_tip;
        let common_ancestor = self.find_common_ancestor(&old_tip, &parent_hash)?;

        // Build disconnected path: from old_tip down to common_ancestor (excluding common_ancestor)
        let mut disconnected_blocks = Vec::new();
        let mut curr_disc = old_tip;
        while curr_disc != common_ancestor {
            if let Some(node) = self.nodes.get(&curr_disc) {
                disconnected_blocks.push(node.block.clone());
                curr_disc = node.parent_hash;
            } else {
                break;
            }
        }

        let reorg_depth = disconnected_blocks.len() as u64;
        if reorg_depth > self.max_reorg_depth {
            // Retain block in the DAG for archival/audit without switching canonical tip
            self.nodes.insert(block_hash, new_node);
            return Err(ChainError::ReorgDepthExceeded {
                depth: reorg_depth,
                max: self.max_reorg_depth,
            });
        }

        // Build connected path: from common_ancestor up to candidate block (excluding common_ancestor)
        let mut connected_nodes = Vec::new();
        let mut curr_conn = parent_hash;
        while curr_conn != common_ancestor {
            if let Some(node) = self.nodes.get(&curr_conn) {
                connected_nodes.push(node.clone());
                curr_conn = node.parent_hash;
            } else {
                break;
            }
        }
        connected_nodes.reverse();
        // Add the new block itself
        connected_nodes.push(new_node.clone());

        // 6. Atomic State Transition Simulation
        let mut staged_utxo = if common_ancestor == old_tip {
            utxo_set.clone()
        } else {
            // Replay from Genesis to common_ancestor on actual deep reorg
            let genesis_to_ancestor = self.get_path_from_genesis(&common_ancestor)?;
            let mut staged = UtxoSet::new();
            for node in &genesis_to_ancestor {
                staged
                    .apply_block_transactions(
                        &node.block.transactions[0],
                        &node.block.transactions[1..],
                        node.height,
                    )
                    .map_err(|e| ChainError::ReorgFailed {
                        hash: node.hash,
                        error: e.to_string(),
                    })?;
            }
            staged
        };

        // Apply all blocks in connected_nodes
        let mut connected_blocks = Vec::new();
        for node in &connected_nodes {
            let headers: Vec<_> = self
                .get_path_from_genesis(&node.parent_hash)?
                .into_iter()
                .map(|parent| parent.block.header)
                .collect();
            if let Err(error) = validate_block_authoritative_with_headers(
                &node.block,
                node.height,
                &staged_utxo,
                &headers,
            ) {
                self.invalid_blocks.insert(node.hash);
                self.invalid_blocks.insert(block_hash);
                return Err(match error {
                    ConsensusError::InvalidCoinbaseReward {
                        subsidy,
                        fees,
                        output,
                    } => ChainError::InvalidCoinbaseReward {
                        subsidy,
                        fees,
                        output,
                    },
                    ConsensusError::InvalidTransactionCommitment { expected, actual } => {
                        ChainError::InvalidTransactionCommitment { expected, actual }
                    }
                    other => ChainError::ReorgFailed {
                        hash: node.hash,
                        error: other.to_string(),
                    },
                });
            }
            if let Err(err) = verifier.verify_block_transactions(&node.block, &staged_utxo) {
                self.invalid_blocks.insert(node.hash);
                self.invalid_blocks.insert(block_hash);
                return Err(ChainError::ReorgFailed {
                    hash: node.hash,
                    error: format!("Block transaction verification failed: {err}"),
                });
            }
            staged_utxo
                .apply_block_transactions(
                    &node.block.transactions[0],
                    &node.block.transactions[1..],
                    node.height,
                )
                .map_err(|e| {
                    self.invalid_blocks.insert(node.hash);
                    self.invalid_blocks.insert(block_hash);
                    ChainError::ReorgFailed {
                        hash: node.hash,
                        error: e.to_string(),
                    }
                })?;
            let calculated_root = staged_utxo.compute_utxo_root();
            if node.block.header.utxo_root != calculated_root {
                self.invalid_blocks.insert(node.hash);
                self.invalid_blocks.insert(block_hash);
                return Err(ChainError::ReorgFailed {
                    hash: node.hash,
                    error: format!(
                        "UTXO root commitment mismatch: expected {}, got {}",
                        node.block.header.utxo_root, calculated_root
                    ),
                });
            }
            connected_blocks.push(node.block.clone());
        }

        // 7. Success! Insert node, update canonical tip, commit utxo set
        self.nodes.insert(block_hash, new_node);
        self.canonical_tip = block_hash;
        *utxo_set = staged_utxo;

        // Collect mempool transactions from disconnected blocks (excluding coinbase)
        // and remove any that are included in the new connected blocks
        let connected_txids: HashSet<_> = connected_blocks
            .iter()
            .flat_map(|b| b.transactions.iter().map(|t| t.txid()))
            .collect();

        let mut transactions_for_mempool = Vec::new();
        for d_block in &disconnected_blocks {
            for tx in d_block.transactions.iter().skip(1) {
                if !connected_txids.contains(&tx.txid()) {
                    transactions_for_mempool.push(tx.clone());
                }
            }
        }

        Ok(Some(ReorgResult {
            old_tip,
            new_tip: block_hash,
            disconnected_blocks,
            connected_blocks,
            transactions_for_mempool,
        }))
    }
}
