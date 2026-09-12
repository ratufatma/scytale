//! Embedded Stratum pool server service bridge for Scytale Node.
//!
//! Orchestrates the local `StratumServer` TCP listener, block candidate ingestion,
//! and real-time block template/job broadcasting to connected mining workers.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use scytale_core::{
    Block, BlockHeader, CanonicalDeserialize, CanonicalSerialize, Hash256, OutPoint, Transaction,
    TxIn, TRANSACTION_VERSION_1,
};
use scytale_mining::BlockTemplate;
use scytale_stratum::{BlockFoundEvent, StratumJob, StratumServer};

use crate::node::Node;

/// Cached active Stratum job for assembling full blocks upon worker share submission.
#[derive(Clone)]
struct CachedJob {
    #[allow(dead_code)]
    pub job_id: String,
    pub template: BlockTemplate,
    pub other_txs: Vec<Transaction>,
}

/// Handle to the spawned Stratum background tasks.
pub struct StratumServiceHandle {
    pub server_task: tokio::task::JoinHandle<()>,
    pub ingestion_task: tokio::task::JoinHandle<()>,
    pub broadcaster_task: tokio::task::JoinHandle<()>,
}

impl StratumServiceHandle {
    /// Aborts all running background tasks for this Stratum service.
    pub fn abort(&self) {
        self.server_task.abort();
        self.ingestion_task.abort();
        self.broadcaster_task.abort();
    }
}

/// Starts the embedded Stratum mining server and connects it to the node daemon.
pub fn start_stratum_service(
    node: Arc<Node>,
    bind_addr: SocketAddr,
    stratum_diff: f64,
    miner_payout_script: Vec<u8>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> StratumServiceHandle {
    let server = Arc::new(StratumServer::new(bind_addr, stratum_diff));
    let mut block_rx = server
        .take_block_receiver()
        .expect("Stratum block candidate receiver must be available");

    let jobs_cache = Arc::new(RwLock::new(HashMap::<String, CachedJob>::new()));
    let job_counter = Arc::new(AtomicU64::new(1));

    // ── Task 1: Stratum TCP Pool Server ──────────────────────────────────────
    let server_for_run = Arc::clone(&server);
    let server_task = tokio::spawn(async move {
        info!("Starting embedded Stratum TCP pool server on {}", bind_addr);
        if let Err(err) = server_for_run.run().await {
            error!("Stratum server terminated with error: {}", err);
        }
    });

    // ── Task 2: Block Candidate Ingestion ────────────────────────────────────
    let node_for_ingest = Arc::clone(&node);
    let cache_for_ingest = Arc::clone(&jobs_cache);
    let ingestion_task = tokio::spawn(async move {
        while let Some(event) = block_rx.recv().await {
            let BlockFoundEvent {
                header,
                pow_hash,
                job,
                worker,
                extranonce1,
                extranonce2,
            } = event;

            info!(
                worker = %worker,
                job_id = %job.job_id,
                pow_hash = %pow_hash,
                "Received valid Proof-of-Work block candidate from Stratum worker"
            );

            let cached = {
                let cache = cache_for_ingest.read().unwrap();
                cache.get(&job.job_id).cloned()
            };

            let (template, other_txs) = match cached {
                Some(c) => (c.template, c.other_txs),
                None => {
                    warn!(
                        job_id = %job.job_id,
                        worker = %worker,
                        "Received share for unknown or expired Stratum job ID"
                    );
                    continue;
                }
            };

            // Reconstruct canonical coinbase payload:
            // raw_coinbase = coinbase1 + extranonce1 + extranonce2 + coinbase2
            let mut raw_cb = Vec::with_capacity(
                job.coinbase1.len() + extranonce1.len() + extranonce2.len() + job.coinbase2.len(),
            );
            raw_cb.extend_from_slice(&job.coinbase1);
            raw_cb.extend_from_slice(&extranonce1);
            raw_cb.extend_from_slice(&extranonce2);
            raw_cb.extend_from_slice(&job.coinbase2);

            let coinbase_tx = match Transaction::from_canonical_bytes(&raw_cb) {
                Ok(tx) => tx,
                Err(err) => {
                    warn!(
                        worker = %worker,
                        error = %err,
                        "Failed to decode canonical coinbase from extranonces, falling back to template coinbase"
                    );
                    template.transactions[0].clone()
                }
            };

            let mut block_txs = Vec::with_capacity(1 + other_txs.len());
            block_txs.push(coinbase_tx);
            block_txs.extend(other_txs);

            let block_header: BlockHeader = header.into();
            let block = Block::new(block_header, block_txs);

            match node_for_ingest.submit_external_block(block) {
                Ok(true) => {
                    info!(
                        worker = %worker,
                        pow_hash = %pow_hash,
                        height = node_for_ingest.canonical_height(),
                        "Stratum block candidate successfully accepted and connected to canonical chain!"
                    );
                }
                Ok(false) => {
                    info!(
                        worker = %worker,
                        pow_hash = %pow_hash,
                        "Stratum block candidate accepted as side branch or duplicate"
                    );
                }
                Err(err) => {
                    error!(
                        worker = %worker,
                        pow_hash = %pow_hash,
                        error = %err,
                        "Stratum block candidate rejected by consensus rules"
                    );
                }
            }
        }
    });

    // ── Task 3: Real-Time Block Template & Job Broadcaster ────────────────────
    let node_for_broadcast = Arc::clone(&node);
    let server_for_broadcast = Arc::clone(&server);
    let cache_for_broadcast = Arc::clone(&jobs_cache);
    let broadcaster_task = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_millis(1000));
        let mut last_tip = Hash256::ZERO;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    info!("Stratum job broadcaster received shutdown signal");
                    break;
                }
                _ = ticker.tick() => {
                    let current_tip = node_for_broadcast.canonical_tip();
                    let tip_changed = current_tip != last_tip;

                    if tip_changed || last_tip == Hash256::ZERO {
                        match node_for_broadcast.build_mining_template(miner_payout_script.clone()) {
                            Ok(mut template) => {
                                let height = template.height;

                                // Format coinbase input with 16-byte authorization:
                                // [8-byte height LE] + [4-byte extranonce1 placeholder] + [4-byte extranonce2 placeholder]
                                let mut auth = Vec::with_capacity(16);
                                auth.extend_from_slice(&height.to_le_bytes());
                                auth.extend_from_slice(&[0u8; 8]);

                                let cb_input = TxIn::new(OutPoint::null(), auth);
                                let cb_tx = Transaction::new(
                                    TRANSACTION_VERSION_1,
                                    vec![cb_input],
                                    template.transactions[0].outputs.clone(),
                                    0,
                                );
                                template.transactions[0] = cb_tx;

                                // Recompute prospective utxo_root with the template coinbase
                                let mut utxos = node_for_broadcast.query_utxo_set();
                                if utxos.apply_block(&template.transactions, height).is_ok() {
                                    template.utxo_root = utxos.compute_utxo_root();
                                }

                                if let Ok(cb_bytes) = template.transactions[0].to_canonical_bytes() {
                                    if cb_bytes.len() >= 64 {
                                        let coinbase1 = cb_bytes[0..56].to_vec();
                                        let coinbase2 = cb_bytes[64..].to_vec();

                                        let other_txs = template.transactions[1..].to_vec();
                                        let other_hashes: Vec<Hash256> =
                                            other_txs.iter().map(|t| t.txid()).collect();

                                        let merkle_branches = if other_hashes.is_empty() {
                                            vec![Hash256::ZERO]
                                        } else if other_hashes.len() == 1 {
                                            vec![other_hashes[0]]
                                        } else {
                                            StratumJob::calculate_merkle_branches(&other_hashes)
                                        };

                                        let jid = format!("{:x}", job_counter.fetch_add(1, Ordering::Relaxed));
                                        let job = Arc::new(StratumJob::new(
                                            jid.clone(),
                                            template.previous_block_hash,
                                            coinbase1,
                                            coinbase2,
                                            merkle_branches,
                                            1,
                                            template.compact_target,
                                            template.utxo_root,
                                            template.timestamp,
                                            tip_changed,
                                        ));

                                        // Store in active jobs cache (prune if exceeds 64 jobs)
                                        {
                                            let mut cache = cache_for_broadcast.write().unwrap();
                                            if cache.len() > 64 {
                                                cache.clear();
                                            }
                                            cache.insert(
                                                jid.clone(),
                                                CachedJob {
                                                    job_id: jid.clone(),
                                                    template: template.clone(),
                                                    other_txs,
                                                },
                                            );
                                        }

                                        server_for_broadcast.broadcast_job(job).await;
                                        last_tip = current_tip;
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Failed to construct mining template for Stratum: {}", err);
                            }
                        }
                    }
                }
            }
        }
    });

    StratumServiceHandle {
        server_task,
        ingestion_task,
        broadcaster_task,
    }
}
