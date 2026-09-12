use dashmap::DashSet;
use std::sync::atomic::{AtomicU64, Ordering};

/// State representation for an active TCP worker session.
pub struct WorkerSession {
    pub session_id: u64,
    pub extranonce1: [u8; 4],
    pub authorized_worker: parking_lot::RwLock<Option<String>>,
    pub authorized_address: parking_lot::RwLock<Option<String>>,
    pub difficulty: parking_lot::RwLock<f64>,
    pub valid_shares: AtomicU64,
    pub invalid_shares: AtomicU64,
    /// Set of submitted shares `(job_id, nonce, extranonce2_u64)` to detect replays.
    submitted_shares: DashSet<(String, u64, u64)>,
}

impl WorkerSession {
    pub fn new(session_id: u64, extranonce1: [u8; 4], initial_diff: f64) -> Self {
        Self {
            session_id,
            extranonce1,
            authorized_worker: parking_lot::RwLock::new(None),
            authorized_address: parking_lot::RwLock::new(None),
            difficulty: parking_lot::RwLock::new(initial_diff),
            valid_shares: AtomicU64::new(0),
            invalid_shares: AtomicU64::new(0),
            submitted_shares: DashSet::new(),
        }
    }

    /// Checks whether the worker has authorized with a username/wallet.
    pub fn is_authorized(&self) -> bool {
        self.authorized_worker.read().is_some()
    }

    /// Gets the current difficulty for this worker session.
    pub fn get_difficulty(&self) -> f64 {
        *self.difficulty.read()
    }

    /// Sets the worker difficulty.
    pub fn set_difficulty(&self, diff: f64) {
        *self.difficulty.write() = diff;
    }

    /// Records an accepted valid share.
    pub fn record_valid_share(&self) -> u64 {
        self.valid_shares.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Records a rejected invalid share.
    pub fn record_invalid_share(&self) -> u64 {
        self.invalid_shares.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Checks if a share has already been submitted, and if not, records it.
    /// Returns `true` if the share is unique, or `false` if it is a duplicate.
    pub fn check_and_record_share(&self, job_id: &str, nonce: u64, extranonce2_num: u64) -> bool {
        self.submitted_shares
            .insert((job_id.to_string(), nonce, extranonce2_num))
    }

    /// Clears expired shares to maintain bounded memory across long sessions.
    pub fn prune_old_jobs(&self, active_job_id: &str) {
        self.submitted_shares.retain(|(jid, _, _)| jid == active_job_id);
    }
}
