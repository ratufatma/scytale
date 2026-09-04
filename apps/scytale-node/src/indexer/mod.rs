//! In-process, non-blocking Indexer module for Scytale blockchain.
//!
//! Listens to newly committed blocks from the storage/consensus layer and
//! dispatches their metadata to an external web explorer via outbound HTTP POST.
//! The worker runs on its own dedicated OS thread using `crossbeam-channel` and `ureq`,
//! guaranteeing zero performance impact on mining and consensus.

use crossbeam_channel::{bounded, Receiver, Sender, TrySendError};
use scytale_core::{Address, Block};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Bounded channel capacity for the indexer queue.
pub const INDEXER_CHANNEL_CAPACITY: usize = 100;

/// Block metadata payload dispatched to the external explorer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockPayload {
    pub height: u64,
    pub hash: String,
    pub prev_hash: String,
    pub miner: String,
    pub timestamp: u64,
    pub tx_count: usize,
}

impl BlockPayload {
    /// Constructs a `BlockPayload` from a domain `Block` reference and height.
    pub fn from_block(block: &Block, height: u64) -> Self {
        let hash = block.header.hash().to_string();
        let prev_hash = block.header.previous_block_hash.to_string();
        let miner = extract_miner_address(block);
        let timestamp = block.header.timestamp;
        let tx_count = block.transactions.len();

        Self {
            height,
            hash,
            prev_hash,
            miner,
            timestamp,
            tx_count,
        }
    }
}

/// Extracts the miner address from a block's coinbase transaction output.
/// If P2PKH script is recognized, encodes it as a Bech32 address (`scy1...`).
/// Otherwise falls back to lowercase hexadecimal encoding of the locking script.
fn extract_miner_address(block: &Block) -> String {
    if let Some(coinbase) = block.transactions.first() {
        if let Some(out) = coinbase.outputs.first() {
            let script = &out.locking_condition;
            // Check standard P2PKH: OP_DUP(0x73) OP_BLAKE3(0xa0) OP_PUSHBYTES_32(0x20) [32B hash] OP_EQUALVERIFY(0x88) OP_CHECKSIG(0xac)
            if script.len() == 37
                && script[0] == 0x73
                && script[1] == 0xa0
                && script[2] == 0x20
                && script[35] == 0x88
                && script[36] == 0xac
            {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&script[3..35]);
                let addr = Address::new(hash);
                if let Ok(bech32_str) = addr.to_bech32() {
                    return bech32_str;
                }
            }
            if !script.is_empty() {
                return scytale_primitives::to_hex(script);
            }
        }
    }
    "coinbase".to_string()
}

/// Handle holding the sending half of the indexer bounded channel.
#[derive(Debug, Clone)]
pub struct IndexerHandle {
    pub sender: Sender<BlockPayload>,
}

impl IndexerHandle {
    /// Attempts to enqueue a block payload without blocking.
    ///
    /// If the channel buffer is full or the receiver has disconnected,
    /// returns an error immediately without stalling the caller.
    pub fn try_send(&self, payload: BlockPayload) -> Result<(), TrySendError<BlockPayload>> {
        self.sender.try_send(payload)
    }
}

/// Spawns a dedicated OS thread named `scytale-indexer` to dispatch committed
/// blocks to `target_url` via HTTP POST.
pub fn start_indexer(target_url: String, api_key: Option<String>) -> IndexerHandle {
    let (sender, receiver) = bounded::<BlockPayload>(INDEXER_CHANNEL_CAPACITY);

    let thread_name = "scytale-indexer".to_string();
    let spawn_res = std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            worker_loop(receiver, target_url, api_key);
        });

    if let Err(e) = spawn_res {
        tracing::error!("failed to spawn scytale-indexer OS thread: {e}");
    }

    IndexerHandle { sender }
}

/// Worker loop running on the dedicated OS thread.
///
/// Dispatches serialized JSON payloads to `target_url` via `ureq::post`.
/// Retries on network errors up to 3 times with 2-second sleep before discarding.
fn worker_loop(
    receiver: Receiver<BlockPayload>,
    target_url: String,
    api_key: Option<String>,
) {
    tracing::info!(target_url = %target_url, "scytale indexer worker started");

    let agent = ureq::builder()
        .timeout(Duration::from_secs(10))
        .build();

    while let Ok(payload) = receiver.recv() {
        let mut retries = 0;
        const MAX_RETRIES: usize = 3;

        loop {
            let mut req = agent.post(&target_url);
            if let Some(ref key) = api_key {
                req = req.set("Authorization", &format!("Bearer {key}"));
            }

            match req.send_json(&payload) {
                Ok(response) => {
                    tracing::info!(
                        height = payload.height,
                        status = response.status(),
                        "indexer dispatched block metadata to explorer successfully"
                    );
                    break;
                }
                Err(e) => {
                    retries += 1;
                    tracing::warn!(
                        attempt = retries,
                        max = MAX_RETRIES,
                        height = payload.height,
                        error = %e,
                        "indexer dispatch attempt failed"
                    );
                    if retries >= MAX_RETRIES {
                        tracing::error!(
                            height = payload.height,
                            "indexer exhausted maximum retries for block; discarding payload"
                        );
                        break;
                    }
                    std::thread::sleep(Duration::from_secs(2));
                }
            }
        }
    }

    tracing::info!("indexer channel disconnected; scytale indexer worker terminating cleanly");
}

#[cfg(test)]
mod tests {
    use super::*;
    use scytale_core::{Block, BlockHeader, Transaction, TxOut};
    use scytale_primitives::Hash256;

    fn sample_payload(height: u64) -> BlockPayload {
        BlockPayload {
            height,
            hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            prev_hash: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            miner: "scy1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqp8f7c9".into(),
            timestamp: 1700000000,
            tx_count: 1,
        }
    }

    #[test]
    fn test_indexer_try_send_success() {
        let (sender, receiver) = bounded::<BlockPayload>(100);
        let handle = IndexerHandle { sender };
        let payload = sample_payload(1);

        let res = handle.try_send(payload.clone());
        assert!(res.is_ok());

        let received = receiver.recv().unwrap();
        assert_eq!(received, payload);
    }

    #[test]
    fn test_indexer_receiver_dropped_no_panic() {
        let (sender, receiver) = bounded::<BlockPayload>(100);
        let handle = IndexerHandle { sender };
        drop(receiver);

        let payload = sample_payload(2);
        let res = handle.try_send(payload);
        assert!(res.is_err());
        match res.unwrap_err() {
            TrySendError::Disconnected(dropped) => {
                assert_eq!(dropped.height, 2);
            }
            TrySendError::Full(_) => panic!("expected Disconnected error, got Full"),
        }
    }

    #[test]
    fn test_indexer_bounded_channel_overflow() {
        let (sender, _receiver) = bounded::<BlockPayload>(INDEXER_CHANNEL_CAPACITY);
        let handle = IndexerHandle { sender };

        for i in 0..INDEXER_CHANNEL_CAPACITY {
            let res = handle.try_send(sample_payload(i as u64));
            assert!(res.is_ok(), "failed to push item {i}");
        }

        // 101st try_send must immediately fail with Full without blocking
        let overflow_payload = sample_payload(999);
        let res = handle.try_send(overflow_payload);
        assert!(res.is_err());
        match res.unwrap_err() {
            TrySendError::Full(dropped) => {
                assert_eq!(dropped.height, 999);
            }
            TrySendError::Disconnected(_) => panic!("expected Full error, got Disconnected"),
        }
    }

    #[test]
    fn test_block_payload_serialization() {
        let payload = sample_payload(42);
        let json_val = serde_json::to_value(&payload).expect("serialization failed");

        assert_eq!(json_val["height"], 42);
        assert_eq!(
            json_val["hash"],
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            json_val["prev_hash"],
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        assert_eq!(
            json_val["miner"],
            "scy1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqp8f7c9"
        );
        assert_eq!(json_val["timestamp"], 1700000000);
        assert_eq!(json_val["tx_count"], 1);
    }

    #[test]
    fn test_block_payload_from_block() {
        let header = BlockHeader {
            version: 1,
            previous_block_hash: Hash256::ZERO,
            transaction_commitment: Hash256::ZERO,
            utxo_root: Hash256::ZERO,
            timestamp: 1680000000,
            difficulty_target: 0x1d00ffff,
            nonce: 12345,
        };
        let coinbase = Transaction::new_coinbase(
            0,
            vec![TxOut::new(50_0000_0000, vec![0x01, 0x02, 0x03])],
        );
        let block = Block::new(header, vec![coinbase]);

        let payload = BlockPayload::from_block(&block, 5);
        assert_eq!(payload.height, 5);
        assert_eq!(payload.hash, block.header.hash().to_string());
        assert_eq!(payload.prev_hash, Hash256::ZERO.to_string());
        assert_eq!(payload.miner, "010203");
        assert_eq!(payload.timestamp, 1680000000);
        assert_eq!(payload.tx_count, 1);
    }

    #[test]
    fn test_start_indexer_spawn() {
        let handle = start_indexer(
            "http://127.0.0.1:54321/mock-explorer".into(),
            Some("secret-key".into()),
        );
        let payload = sample_payload(10);
        let res = handle.try_send(payload);
        assert!(res.is_ok());
    }
}
