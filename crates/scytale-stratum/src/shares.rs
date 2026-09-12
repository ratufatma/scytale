use crate::error::StratumError;
use crate::header::RawBlockHeader120;
use crate::job::StratumJob;
use scytale_consensus::Target;
use scytale_core::Hash256;
use uint::construct_uint;

construct_uint! {
    /// 256-bit unsigned integer for difficulty and share calculations.
    pub struct U256(4);
}

/// Result of evaluating a worker's submitted share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShareVerificationResult {
    /// Share satisfies the pool difficulty and is accepted for worker accounting.
    ValidShare { hash: Hash256 },
    /// Share satisfies both pool difficulty and network consensus difficulty (block found!).
    BlockCandidate {
        header: RawBlockHeader120,
        hash: Hash256,
    },
}

/// Converts a 32-bit compact difficulty bits into a numerical 256-bit unsigned target.
pub fn compact_to_target(compact: u32) -> U256 {
    let exponent = (compact >> 24) as usize;
    let mantissa = (compact & 0x007F_FFFF) as u64;
    let is_negative = (compact & 0x0080_0000) != 0;

    if is_negative || mantissa == 0 || exponent == 0 {
        return U256::zero();
    }

    if exponent <= 3 {
        U256::from(mantissa >> (8 * (3 - exponent)))
    } else {
        U256::from(mantissa) << (8 * (exponent - 3))
    }
}

/// Evaluates a worker's share submission against the pool difficulty and network consensus target.
pub fn verify_worker_share(
    job: &StratumJob,
    extranonce1: &[u8],
    extranonce2: &[u8],
    timestamp: u64,
    nonce: u64,
    share_difficulty: f64,
) -> Result<ShareVerificationResult, StratumError> {
    // 1. Reconstruct Merkle root and 120-byte block header
    let merkle_root = job.build_merkle_root(extranonce1, extranonce2);
    let header = job.build_header(merkle_root, timestamp, nonce);

    // 2. Evaluate BLAKE3 PoW hash
    let hash = header.compute_pow_hash();
    let hash_be = U256::from_big_endian(hash.as_bytes());
    let hash_le = U256::from_little_endian(hash.as_bytes());

    // 3. Compute worker share difficulty target
    // Base difficulty 1.0 corresponds to standard floor 0x1d00ffff
    let base_target = compact_to_target(0x1d00ffff);
    let share_target = if share_difficulty <= 0.0 {
        U256::max_value()
    } else if share_difficulty < 1.0 {
        let mult = (1.0 / share_difficulty).min(u64::MAX as f64) as u64;
        let (res, overflow) = base_target.overflowing_mul(U256::from(mult));
        if overflow {
            U256::max_value()
        } else {
            res
        }
    } else {
        let diff_scaled = (share_difficulty * 1000.0).max(1.0) as u64;
        (base_target / U256::from(diff_scaled)) * U256::from(1000)
    };

    // Verify whether hash meets share target (supporting both big-endian and little-endian hash representations)
    let meets_share_target = hash_be <= share_target || hash_le <= share_target;
    if !meets_share_target {
        return Err(StratumError::HighHash);
    }

    // 4. Verify whether hash satisfies canonical network consensus target
    let network_target_num = compact_to_target(job.bits);
    let consensus_target = Target::from_compact(job.bits);

    let meets_network = consensus_target.is_met_by(&hash)
        || hash_be <= network_target_num
        || hash_le <= network_target_num;

    if meets_network {
        Ok(ShareVerificationResult::BlockCandidate { header, hash })
    } else {
        Ok(ShareVerificationResult::ValidShare { hash })
    }
}
