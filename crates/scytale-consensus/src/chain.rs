use crate::error::ChainError;
use crate::target::Target;
use crate::work::{block_work, CumulativeWork};
use scytale_core::{Block, Hash256, Transaction, UtxoSet};
use std::collections::{HashMap, HashSet};

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

/// In-memory tree tracking all validated blocks, competing forks, and the active canonical tip.
pub struct ChainTree {
    nodes: HashMap<Hash256, BlockNode>,
    canonical_tip: Hash256,
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
        }
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
        // 1. Stateless structural validation
        block.validate_structure().map_err(ChainError::BlockError)?;

        let block_hash = block.header.hash();
        if self.nodes.contains_key(&block_hash) {
            return Ok(None);
        }

        // 2. Parent linkage
        let parent_hash = block.header.previous_block_hash;
        let parent_node = self
            .nodes
            .get(&parent_hash)
            .ok_or(ChainError::CorruptedLinkage {
                parent: parent_hash,
            })?
            .clone();

        let height = parent_node.height + 1;

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
        // Path from Genesis to common_ancestor:
        let genesis_to_ancestor = self.get_path_from_genesis(&common_ancestor)?;

        let mut staged_utxo = UtxoSet::new();
        for node in &genesis_to_ancestor {
            staged_utxo
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

        // Apply all blocks in connected_nodes
        let mut connected_blocks = Vec::new();
        for node in &connected_nodes {
            staged_utxo
                .apply_block_transactions(
                    &node.block.transactions[0],
                    &node.block.transactions[1..],
                    node.height,
                )
                .map_err(|e| ChainError::ReorgFailed {
                    hash: node.hash,
                    error: e.to_string(),
                })?;
            let calculated_root = staged_utxo.compute_utxo_root();
            if node.block.header.utxo_root != calculated_root {
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
