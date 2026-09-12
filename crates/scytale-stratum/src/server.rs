use crate::codec::LinesCodec;
use crate::error::{StratumError, StratumRpcError};
use crate::header::RawBlockHeader120;
use crate::job::StratumJob;
use crate::protocol::*;
use crate::session::WorkerSession;
use crate::shares::{verify_worker_share, ShareVerificationResult};
use byteorder::{BigEndian, ByteOrder};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_util::codec::Framed;
use tracing::{debug, info, warn};

/// Callback packet delivered when a worker finds a valid block candidate.
#[derive(Debug, Clone)]
pub struct BlockFoundEvent {
    pub header: RawBlockHeader120,
    pub pow_hash: scytale_core::Hash256,
    pub job: Arc<StratumJob>,
    pub worker: String,
    pub extranonce1: Vec<u8>,
    pub extranonce2: Vec<u8>,
}

/// Supervisor and multi-threaded TCP server for Stratum mining pool workers.
pub struct StratumServer {
    bind_addr: SocketAddr,
    sessions: Arc<DashMap<u64, (Arc<WorkerSession>, mpsc::UnboundedSender<String>)>>,
    current_job: Arc<parking_lot::RwLock<Option<Arc<StratumJob>>>>,
    session_counter: AtomicU64,
    default_difficulty: f64,
    block_candidate_tx: mpsc::UnboundedSender<BlockFoundEvent>,
    block_candidate_rx: parking_lot::Mutex<Option<mpsc::UnboundedReceiver<BlockFoundEvent>>>,
}

impl StratumServer {
    /// Creates a new Stratum pool server instance.
    pub fn new(bind_addr: SocketAddr, default_difficulty: f64) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            bind_addr,
            sessions: Arc::new(DashMap::new()),
            current_job: Arc::new(parking_lot::RwLock::new(None)),
            session_counter: AtomicU64::new(1),
            default_difficulty,
            block_candidate_tx: tx,
            block_candidate_rx: parking_lot::Mutex::new(Some(rx)),
        }
    }

    /// Takes the receiver for block candidate events found by pool workers.
    pub fn take_block_receiver(&self) -> Option<mpsc::UnboundedReceiver<BlockFoundEvent>> {
        self.block_candidate_rx.lock().take()
    }

    /// Sets the current active mining job template and broadcasts to all active workers.
    pub async fn broadcast_job(&self, job: Arc<StratumJob>) {
        *self.current_job.write() = Some(Arc::clone(&job));

        let branches_hex: Vec<String> = job
            .merkle_branches
            .iter()
            .map(|b| hex::encode(b.as_bytes()))
            .collect();

        let notify = StratumNotification::notify(
            &job.job_id,
            &hex::encode(job.prev_hash.as_bytes()),
            &hex::encode(&job.coinbase1),
            &hex::encode(&job.coinbase2),
            &branches_hex,
            job.version,
            job.bits,
            job.curtime,
            job.clean_jobs,
        );

        if let Ok(msg_str) = serde_json::to_string(&notify) {
            let line = msg_str + "\n";
            for entry in self.sessions.iter() {
                let (_, sender) = entry.value();
                let _ = sender.send(line.clone());
            }
        }
    }

    /// Returns the number of active worker connections.
    pub fn active_connections(&self) -> usize {
        self.sessions.len()
    }

    /// Runs the Stratum TCP pool listener.
    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let listener = TcpListener::bind(self.bind_addr).await?;
        info!("Stratum TCP pool server running on {}", self.bind_addr);

        loop {
            let (stream, peer_addr) = listener.accept().await?;
            let server = Arc::clone(&self);

            tokio::spawn(async move {
                if let Err(e) = server.handle_connection(stream, peer_addr).await {
                    warn!("Worker connection {} ended: {:?}", peer_addr, e);
                }
            });
        }
    }

    pub async fn handle_connection(
        self: &Arc<Self>,
        stream: TcpStream,
        peer_addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session_id = self.session_counter.fetch_add(1, Ordering::Relaxed);
        let extranonce1 = [0u8; 4];

        let session = Arc::new(WorkerSession::new(
            session_id,
            extranonce1,
            self.default_difficulty,
        ));
        let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<String>();

        self.sessions.insert(session_id, (Arc::clone(&session), outbound_tx));

        let (mut writer, mut reader) = Framed::new(stream, LinesCodec::new()).split();

        // Writer pump task
        let writer_task = tokio::spawn(async move {
            while let Some(msg) = outbound_rx.recv().await {
                if let Err(err) = writer.send(msg).await {
                    debug!("Error sending to worker: {:?}", err);
                    break;
                }
            }
        });

        // Reader pump
        while let Some(line_result) = reader.next().await {
            let line = line_result?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let req: StratumRequest = match serde_json::from_str(trimmed) {
                Ok(r) => r,
                Err(err) => {
                    let err_resp = StratumResponse::error(
                        None,
                        StratumRpcError::new(StratumRpcError::PARSE_ERROR, err.to_string()),
                    );
                    if let Some(entry) = self.sessions.get(&session_id) {
                        let _ = entry.1.send(serde_json::to_string(&err_resp)?);
                    }
                    continue;
                }
            };

            let response = self.dispatch_rpc(&session, req).await;
            if let Some(entry) = self.sessions.get(&session_id) {
                let _ = entry.1.send(serde_json::to_string(&response)?);
            }
        }

        self.sessions.remove(&session_id);
        writer_task.abort();
        info!("Worker disconnected from {}", peer_addr);
        Ok(())
    }

    async fn dispatch_rpc(&self, session: &Arc<WorkerSession>, req: StratumRequest) -> StratumResponse {
        match req.method.as_str() {
            "mining.subscribe" => {
                let en1_hex = hex::encode(session.extranonce1);
                let res = serde_json::json!([
                    [
                        ["mining.set_difficulty", "sub_diff"],
                        ["mining.notify", "sub_notify"]
                    ],
                    en1_hex,
                    4 // extranonce2 length in bytes
                ]);

                // Immediately schedule difficulty and job notification if available
                if let Some(entry) = self.sessions.get(&session.session_id) {
                    let diff_notify = StratumNotification::set_difficulty(session.get_difficulty());
                    if let Ok(line) = serde_json::to_string(&diff_notify) {
                        let _ = entry.1.send(line);
                    }

                    if let Some(ref job) = *self.current_job.read() {
                        let branches_hex: Vec<String> = job
                            .merkle_branches
                            .iter()
                            .map(|b| hex::encode(b.as_bytes()))
                            .collect();

                        let notify = StratumNotification::notify(
                            &job.job_id,
                            &hex::encode(job.prev_hash.as_bytes()),
                            &hex::encode(&job.coinbase1),
                            &hex::encode(&job.coinbase2),
                            &branches_hex,
                            job.version,
                            job.bits,
                            job.curtime,
                            job.clean_jobs,
                        );
                        if let Ok(notify_line) = serde_json::to_string(&notify) {
                            let _ = entry.1.send(notify_line);
                        }
                    }
                }

                StratumResponse::success(req.id, res)
            }

            "mining.authorize" => {
                if let Some(params) = req.params.as_array() {
                    let worker_name = params
                        .first()
                        .and_then(|v| v.as_str())
                        .unwrap_or("anonymous");
                    *session.authorized_worker.write() = Some(worker_name.to_string());

                    // If worker format is <address>.<worker_id>, parse address
                    if let Some((addr, _)) = worker_name.split_once('.') {
                        *session.authorized_address.write() = Some(addr.to_string());
                    } else {
                        *session.authorized_address.write() = Some(worker_name.to_string());
                    }
                }
                StratumResponse::success(req.id, serde_json::json!(true))
            }

            "mining.submit" => {
                // Parameter layout: [worker_name, job_id, extranonce2, curtime, nonce]
                let params = match req.params.as_array() {
                    Some(p) if p.len() >= 5 => p,
                    _ => {
                        return StratumResponse::error(
                            req.id,
                            StratumRpcError::invalid_params("expected 5 parameters: [worker, job_id, extranonce2, curtime, nonce]"),
                        );
                    }
                };

                let worker_name = params[0].as_str().unwrap_or("");
                let job_id = params[1].as_str().unwrap_or("");
                let en2_str = params[2].as_str().unwrap_or("");
                let time_val = &params[3];
                let nonce_val = &params[4];

                // 1. Verify job exists
                let active_job = {
                    let guard = self.current_job.read();
                    match &*guard {
                        Some(job) if job.job_id == job_id => Arc::clone(job),
                        _ => {
                            session.record_invalid_share();
                            return StratumResponse::error(
                                req.id,
                                StratumRpcError::job_not_found(job_id),
                            );
                        }
                    }
                };

                // 2. Parse extranonce2 hex
                let en2_bytes = match hex::decode(en2_str) {
                    Ok(b) if b.len() == 4 => b,
                    Ok(b) => {
                        session.record_invalid_share();
                        return StratumResponse::error(
                            req.id,
                            StratumRpcError::invalid_params(&format!(
                                "extranonce2 must be 4 bytes (8 hex chars), got {} bytes",
                                b.len()
                            )),
                        );
                    }
                    Err(err) => {
                        session.record_invalid_share();
                        return StratumResponse::error(
                            req.id,
                            StratumRpcError::invalid_params(&format!("invalid extranonce2 hex: {}", err)),
                        );
                    }
                };

                // 3. Parse timestamp
                let timestamp = if let Some(s) = time_val.as_str() {
                    u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or(active_job.curtime)
                } else if let Some(n) = time_val.as_u64() {
                    n
                } else {
                    active_job.curtime
                };

                // 4. Parse nonce
                let nonce = if let Some(s) = nonce_val.as_str() {
                    match u64::from_str_radix(s.trim_start_matches("0x"), 16) {
                        Ok(n) => n,
                        Err(err) => {
                            session.record_invalid_share();
                            return StratumResponse::error(
                                req.id,
                                StratumRpcError::invalid_params(&format!("invalid nonce hex: {}", err)),
                            );
                        }
                    }
                } else if let Some(n) = nonce_val.as_u64() {
                    n
                } else {
                    session.record_invalid_share();
                    return StratumResponse::error(
                        req.id,
                        StratumRpcError::invalid_params("nonce must be an integer or hex string"),
                    );
                };

                // 5. Check duplicate share
                let en2_num = BigEndian::read_u32(&en2_bytes) as u64;
                if !session.check_and_record_share(job_id, nonce, en2_num) {
                    session.record_invalid_share();
                    return StratumResponse::error(req.id, StratumRpcError::duplicate_share());
                }

                // 6. Verify share cryptographic validity against targets
                let diff = session.get_difficulty();
                match verify_worker_share(&active_job, &session.extranonce1, &en2_bytes, timestamp, nonce, diff) {
                    Ok(ShareVerificationResult::ValidShare { hash }) => {
                        session.record_valid_share();
                        debug!("Valid share accepted from {} (hash: {})", worker_name, hash);
                        StratumResponse::success(req.id, serde_json::json!(true))
                    }
                    Ok(ShareVerificationResult::BlockCandidate { header, hash }) => {
                        session.record_valid_share();
                        info!(
                            "Block candidate found by worker {}! Nonce: {}, Hash: {}",
                            worker_name, nonce, hash
                        );
                        let _ = self.block_candidate_tx.send(BlockFoundEvent {
                            header,
                            pow_hash: hash,
                            job: Arc::clone(&active_job),
                            worker: worker_name.to_string(),
                            extranonce1: session.extranonce1.to_vec(),
                            extranonce2: en2_bytes.clone(),
                        });
                        StratumResponse::success(req.id, serde_json::json!(true))
                    }
                    Err(StratumError::HighHash) => {
                        session.record_invalid_share();
                        StratumResponse::error(req.id, StratumRpcError::low_difficulty_share())
                    }
                    Err(err) => {
                        session.record_invalid_share();
                        StratumResponse::error(req.id, err.into())
                    }
                }
            }

            "mining.extranonce.subscribe" => {
                StratumResponse::success(req.id, serde_json::json!(true))
            }

            "mining.get_transactions" => {
                StratumResponse::success(req.id, serde_json::json!([]))
            }

            unknown => StratumResponse::error(req.id, StratumRpcError::method_not_found(unknown)),
        }
    }
}
