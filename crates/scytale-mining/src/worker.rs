use crate::error::MiningError;
use scytale_consensus::{calculate_block_reward, verify_pow, ChainTree, Target};
use scytale_core::{Block, BlockHeader, Hash256, OutPoint, Transaction, TxOut, UtxoSet};
use scytale_mempool::Mempool;
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Maximum canonical payload byte size for transactions in a block (excluding header).
pub const MAX_BLOCK_PAYLOAD_SIZE: usize = 2_000_000;

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
    /// Prospective active UTXO Merkle root after block transactions
    pub utxo_root: Hash256,
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
            self.utxo_root,
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

    // Select mempool transactions sorted by fee-rate (highest first) up to block payload capacity
    let (candidate_txs, _) = mempool.select_transactions_for_block(MAX_BLOCK_PAYLOAD_SIZE);
    let mut selected_txs: Vec<Transaction> = Vec::new();
    let mut total_fees: u64 = 0;
    let mut spent_outputs: HashSet<OutPoint> = HashSet::new();

    for tx in candidate_txs {
        let all_inputs_valid = tx.inputs.iter().all(|input| {
            !spent_outputs.contains(&input.previous_output)
                && (utxos.contains(&input.previous_output)
                    || selected_txs.iter().any(|stx| {
                        stx.txid() == input.previous_output.txid
                            && (input.previous_output.index as usize) < stx.outputs.len()
                    }))
        });
        if all_inputs_valid {
            for input in &tx.inputs {
                spent_outputs.insert(input.previous_output);
            }
            if let Some(entry) = mempool.get(&tx.txid()) {
                total_fees = total_fees.saturating_add(entry.fee);
            }
            selected_txs.push(tx);
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

    // Simulate applying transactions and coinbase to calculate prospective utxo_root
    let mut prospective_utxos = utxos.clone();
    for tx in &transactions {
        if !tx.is_coinbase() {
            for input in &tx.inputs {
                prospective_utxos.remove(&input.previous_output);
            }
        }
        let txid = tx.txid();
        for (idx, output) in tx.outputs.iter().enumerate() {
            if output.locking_condition.first() != Some(&0x6a) {
                let op = OutPoint::new(txid, idx as u32);
                prospective_utxos.insert(
                    op,
                    scytale_core::UtxoEntry::new(output.clone(), height, tx.is_coinbase()),
                );
            }
        }
    }
    let utxo_root = prospective_utxos.compute_utxo_root();

    Ok(BlockTemplate {
        previous_block_hash: tip_hash,
        height,
        compact_target,
        transactions,
        utxo_root,
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
