use crate::error::MiningError;
use scytale_consensus::{calculate_block_reward, verify_pow, ChainTree, Target};
use scytale_core::{Block, BlockHeader, Hash256, Transaction, TxOut, UtxoSet};
use scytale_mempool::Mempool;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Candidate block template holding all data needed for a PoW nonce search.
#[derive(Debug, Clone)]
pub struct BlockTemplate {
    /// Target parent hash (current canonical tip)
    pub previous_block_hash: Hash256,
    /// Block height this candidate targets
    pub height: u64,
    /// Compact difficulty target for this block
    pub compact_target: u32,
    /// Transactions to include (coinbase at index 0)
    pub transactions: Vec<Transaction>,
    /// Timestamp to embed in the header (Unix seconds)
    pub timestamp: u64,
}

impl BlockTemplate {
    /// Returns the full 256-bit Target from the compact target.
    pub fn target(&self) -> Target {
        Target::from_compact(self.compact_target)
    }

    /// Builds a BlockHeader with a given nonce from this template.
    pub fn build_header(&self, nonce: u64) -> BlockHeader {
        let tx_commitment = self.compute_transaction_commitment();
        BlockHeader::new(
            1,
            self.previous_block_hash,
            tx_commitment,
            self.timestamp,
            self.compact_target,
            nonce,
        )
    }

    /// Computes the transaction commitment (BLAKE3 of all TxIDs concatenated).
    fn compute_transaction_commitment(&self) -> Hash256 {
        let mut combined = Vec::with_capacity(self.transactions.len() * 32);
        for tx in &self.transactions {
            combined.extend_from_slice(tx.txid().as_bytes());
        }
        if combined.is_empty() {
            Hash256::ZERO
        } else {
            Hash256::hash(&combined)
        }
    }

    /// Assembles a complete `Block` from this template and a solved header.
    pub fn assemble_block(&self, solved_header: BlockHeader) -> Block {
        Block::new(solved_header, self.transactions.clone())
    }
}

/// Constructs a candidate `BlockTemplate` from the current canonical tip and mempool.
pub fn build_template(
    chain: &ChainTree,
    utxos: &UtxoSet,
    mempool: &Mempool,
    compact_target: u32,
    miner_locking_condition: Vec<u8>,
    timestamp: u64,
) -> Result<BlockTemplate, MiningError> {
    let tip_hash = chain.canonical_tip();
    let tip_node = chain
        .get_node(&tip_hash)
        .ok_or(MiningError::CanonicalTipMissing)?;

    let height = tip_node.height + 1;
    let subsidy = calculate_block_reward(height);

    // Select mempool transactions sorted by fee-rate (highest first)
    let mempool_entries = mempool.get_entries_sorted_by_fee_rate();
    let mut selected_txs: Vec<Transaction> = Vec::new();
    let mut total_fees: u64 = 0;

    for entry in &mempool_entries {
        // Validate input resolution against current canonical UTXO set
        let all_inputs_valid = entry
            .transaction
            .inputs
            .iter()
            .all(|input| utxos.contains(&input.previous_output));
        if all_inputs_valid {
            total_fees = total_fees.saturating_add(entry.fee);
            selected_txs.push(entry.transaction.clone());
        }
    }

    // Coinbase value = subsidy + aggregate fees (strictly <= subsidy + fees, no inflation)
    let coinbase_value = subsidy
        .checked_add(total_fees)
        .ok_or(MiningError::ArithmeticOverflow)?;

    let coinbase = Transaction::new_coinbase(
        height,
        vec![TxOut::new(coinbase_value, miner_locking_condition)],
    );

    let mut transactions = vec![coinbase];
    transactions.extend(selected_txs);

    Ok(BlockTemplate {
        previous_block_hash: tip_hash,
        height,
        compact_target,
        transactions,
        timestamp,
    })
}

/// Executes the PoW nonce search against a `BlockTemplate`.
///
/// Iterates nonces in `[start_nonce, start_nonce + max_iterations)` and checks
/// each BLAKE3 header hash against the difficulty target.
///
/// Returns `Ok(solved_header)` on success, `Err(MiningError::Cancelled)` if the
/// cancellation token fires, and `Err(MiningError::ExhaustedNonce)` if the
/// entire nonce range is exhausted without a solution.
pub fn run_pow_search(
    template: &BlockTemplate,
    start_nonce: u64,
    max_iterations: u64,
    cancel: &Arc<AtomicBool>,
) -> Result<BlockHeader, MiningError> {
    let target = template.target();

    for i in 0..max_iterations {
        // Poll cancellation token on every iteration
        if cancel.load(Ordering::Relaxed) {
            return Err(MiningError::Cancelled {
                height: template.height,
            });
        }

        let nonce = start_nonce.wrapping_add(i);
        let header = template.build_header(nonce);

        if verify_pow(&header, &target).is_ok() {
            return Ok(header);
        }
    }

    Err(MiningError::ExhaustedNonce {
        height: template.height,
        searched: max_iterations,
    })
}
