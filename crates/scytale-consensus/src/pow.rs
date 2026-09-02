use crate::error::PowError;
use crate::target::Target;
use scytale_core::BlockHeader;
use scytale_primitives::Hash256;

/// Computes the 32-byte BLAKE3 Proof-of-Work hash of a block header.
pub fn compute_pow_hash(header: &BlockHeader) -> Hash256 {
    header.hash()
}

/// Performs stateless Proof-of-Work verification of a block header against the expected target.
pub fn verify_pow(header: &BlockHeader, expected_target: &Target) -> Result<(), PowError> {
    let hash = compute_pow_hash(header);
    if !expected_target.is_met_by(&hash) {
        return Err(PowError::InsufficientWork {
            hash,
            target: *expected_target,
        });
    }
    Ok(())
}

/// Test helper: single-threaded search helper for finding a valid nonce.
pub fn mine_test_header(header: &mut BlockHeader, target: &Target, max_iterations: u64) -> bool {
    for nonce in 0..max_iterations {
        header.nonce = nonce;
        let hash = header.hash();
        if target.is_met_by(&hash) {
            return true;
        }
    }
    false
}
