use scytale_consensus::{compute_pow_hash, mine_test_header, verify_pow, PowError, Target};
use scytale_core::{BlockHeader, Hash256};

#[test]
fn test_pow_hash_reproducibility() {
    let header = BlockHeader::new(
        1,
        Hash256::hash(b"prev_block"),
        Hash256::hash(b"tx_commitment"),
        1700000000,
        0x1d00ffff,
        42,
    );

    let h1 = compute_pow_hash(&header);
    let h2 = compute_pow_hash(&header);
    assert_eq!(h1, h2);
    assert_eq!(h1, header.hash());
}

#[test]
fn test_nonce_mutation_changes_hash() {
    let mut header1 = BlockHeader::new(
        1,
        Hash256::hash(b"prev"),
        Hash256::hash(b"txs"),
        1700000000,
        0x1d00ffff,
        0,
    );
    let h1 = compute_pow_hash(&header1);

    header1.nonce = 1;
    let h2 = compute_pow_hash(&header1);

    assert_ne!(h1, h2);
}

#[test]
fn test_target_boundary_exact_match() {
    let raw_target = [
        0x00, 0x00, 0x7F, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        0x07, 0x08,
    ];
    let target = Target::from_be_bytes(raw_target);

    // Exact match: hash == target
    let hash_exact = Hash256::new(raw_target);
    assert!(target.is_met_by(&hash_exact));
}

#[test]
fn test_target_boundary_off_by_one_below() {
    let mut raw_target = [
        0x00, 0x00, 0x7F, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        0x07, 0x08,
    ];
    let target = Target::from_be_bytes(raw_target);

    // Target - 1 (decrement last byte)
    raw_target[31] -= 1;
    let hash_below = Hash256::new(raw_target);
    assert!(target.is_met_by(&hash_below));
}

#[test]
fn test_target_boundary_off_by_one_above() {
    let mut raw_target = [
        0x00, 0x00, 0x7F, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
        0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
        0x07, 0x08,
    ];
    let target = Target::from_be_bytes(raw_target);

    // Target + 1 (increment last byte)
    raw_target[31] += 1;
    let hash_above = Hash256::new(raw_target);
    assert!(!target.is_met_by(&hash_above));
}

#[test]
fn test_reject_insufficient_pow() {
    // Extremely difficult target (only hash starting with 0x00000000... can pass)
    let strict_target = Target::from_be_bytes([
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00,
    ]);

    let header = BlockHeader::new(
        1,
        Hash256::hash(b"arbitrary_prev"),
        Hash256::hash(b"arbitrary_tx"),
        1700000000,
        0x1d00ffff,
        9999,
    );

    let result = verify_pow(&header, &strict_target);
    match result {
        Err(PowError::InsufficientWork { hash, target }) => {
            assert_eq!(hash, header.hash());
            assert_eq!(target, strict_target);
        }
        _ => panic!("expected InsufficientWork error, got {:?}", result),
    }
}

#[test]
fn test_mine_and_verify_with_easy_target() {
    let easy_target = Target::max();
    let mut header = BlockHeader::new(
        1,
        Hash256::ZERO,
        Hash256::hash(b"commitment"),
        1700000000,
        0x1d00ffff,
        0,
    );

    let solved = mine_test_header(&mut header, &easy_target, 100);
    assert!(solved);
    assert!(verify_pow(&header, &easy_target).is_ok());
}
