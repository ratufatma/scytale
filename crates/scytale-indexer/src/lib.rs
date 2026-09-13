#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
pub mod schema;
pub mod store;

pub use store::{AddressUtxoEntry, IndexerInput, IndexerOutput, IndexerStore, IndexerTxPayload};

use scytale_core::Block;

/// Helper function to extract relational indexer payloads from a canonical domain Block.
pub fn extract_block_indexer_payload(block: &Block) -> Vec<IndexerTxPayload> {
    block
        .transactions
        .iter()
        .map(|tx| {
            let txid = tx.txid().to_string();
            let inputs = if tx.is_coinbase() {
                Vec::new()
            } else {
                tx.inputs
                    .iter()
                    .map(|inp| IndexerInput {
                        prev_txid: inp.previous_output.txid.to_string(),
                        prev_vout: inp.previous_output.index,
                    })
                    .collect()
            };

            let outputs = tx
                .outputs
                .iter()
                .enumerate()
                .map(|(vout, out)| {
                    let address = scytale_storage::extract_address_from_locking_condition(
                        &out.locking_condition,
                    )
                    .map(|h| hex::encode(h))
                    .unwrap_or_else(|| hex::encode(&out.locking_condition));

                    IndexerOutput {
                        address,
                        vout: vout as u32,
                        value: out.value,
                        token_id: None,
                    }
                })
                .collect();

            IndexerTxPayload {
                txid,
                fee: 0,
                inputs,
                outputs,
            }
        })
        .collect()
}
