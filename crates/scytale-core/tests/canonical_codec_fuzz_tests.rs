//! Adversarial and Property-Based Fuzz Testing Suite for Canonical Binary Codec.
//!
//! Verifies that the canonical deserializer is fail-closed, memory-safe, and never panics
//! when presented with corrupt, truncated, randomized, or maliciously formatted byte streams.

use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use scytale_core::{
    Block, BlockHeader, CanonicalDeserialize, CanonicalSerialize, Hash256, OutPoint,
    SerializationError, Transaction, TxIn, TxOut, TRANSACTION_VERSION_1,
};

/// Constructs a valid sample transaction for differential testing.
fn sample_valid_tx() -> Transaction {
    let prev_op = OutPoint::new(Hash256::hash(b"fuzz_funding_tx"), 0);
    let input = TxIn::new(prev_op, vec![0x01, 0x02, 0x03, 0x04]);
    let output1 = TxOut::new(500_000_000, vec![0x73, 0xa0, 0x88, 0xac]);
    let output2 = TxOut::new(490_000_000, vec![0x6a, 0x04, 0xde, 0xad]);
    Transaction::new(TRANSACTION_VERSION_1, vec![input], vec![output1, output2], 0)
}

/// Constructs a valid sample block for differential testing.
fn sample_valid_block() -> Block {
    let tx = sample_valid_tx();
    let coinbase = Transaction::new_coinbase(
        1,
        vec![TxOut::new(1_000_000_000, vec![0x73, 0xa0, 0x88, 0xac])],
    );
    let header = BlockHeader::new(
        1,
        Hash256::hash(b"genesis_prev"),
        Hash256::hash(b"tx_commitment"),
        Hash256::hash(b"utxo_root"),
        1700000000,
        0x207fffff,
        42,
    );
    Block::new(header, vec![coinbase, tx])
}

#[test]
fn test_fuzz_random_noise_inputs() {
    let mut rng = StdRng::seed_from_u64(0xCAFE_BABE_DEAD_BEEF);

    // Test 1,000 pseudorandom arbitrary byte buffers across varying sizes
    for _ in 0..1000 {
        let len = rng.gen_range(0..2048);
        let mut noise = vec![0u8; len];
        rng.fill_bytes(&mut noise);

        // Deserialization must never panic
        let _ = Transaction::from_canonical_bytes(&noise);
        let _ = BlockHeader::from_canonical_bytes(&noise);
        let _ = Block::from_canonical_bytes(&noise);
        let _ = OutPoint::from_canonical_bytes(&noise);
        let _ = TxIn::from_canonical_bytes(&noise);
        let _ = TxOut::from_canonical_bytes(&noise);
    }
}

#[test]
fn test_fuzz_bit_flips_on_valid_payloads() {
    let mut rng = StdRng::seed_from_u64(0x1337_C0DE_5EED_0001);

    let sample_tx = sample_valid_tx();
    let original_bytes = sample_tx.to_canonical_bytes().unwrap();

    // 500 bit-flip mutations
    for _ in 0..500 {
        let mut mutated = original_bytes.clone();
        let flip_count = rng.gen_range(1..=5);
        for _ in 0..flip_count {
            let byte_idx = rng.gen_range(0..mutated.len());
            let bit_idx = rng.gen_range(0..8);
            mutated[byte_idx] ^= 1 << bit_idx;
        }

        match Transaction::from_canonical_bytes(&mutated) {
            Ok(parsed) => {
                // If it parsed successfully, it must be valid and re-serialize deterministically
                let reserialized = parsed.to_canonical_bytes().unwrap();
                assert_eq!(reserialized, mutated);
            }
            Err(e) => {
                // Must be a recognized SerializationError
                assert!(matches!(
                    e,
                    SerializationError::Io(_)
                        | SerializationError::UnexpectedEof { .. }
                        | SerializationError::TrailingBytes(_)
                        | SerializationError::InvalidEncoding
                        | SerializationError::UnsupportedVersion(_)
                        | SerializationError::LengthExceedsLimit { .. }
                ));
            }
        }
    }
}

#[test]
fn test_fuzz_truncation_resilience() {
    let block = sample_valid_block();
    let bytes = block.to_canonical_bytes().unwrap();

    // Test every single prefix truncation from 0 up to len - 1
    for prefix_len in 0..bytes.len() {
        let truncated = &bytes[0..prefix_len];
        let res = Block::from_canonical_bytes(truncated);
        assert!(
            res.is_err(),
            "Truncated block at length {prefix_len} / {} must fail closed",
            bytes.len()
        );
    }
}

#[test]
fn test_fuzz_trailing_bytes_detection() {
    let tx = sample_valid_tx();
    let valid_bytes = tx.to_canonical_bytes().unwrap();

    let mut rng = StdRng::seed_from_u64(0x9999_8888_7777_6666);

    for trailing_len in 1..=50 {
        let mut corrupted = valid_bytes.clone();
        let mut trailing = vec![0u8; trailing_len];
        rng.fill_bytes(&mut trailing);
        corrupted.extend_from_slice(&trailing);

        let res = Transaction::from_canonical_bytes(&corrupted);
        match res {
            Err(SerializationError::TrailingBytes(n)) => {
                assert_eq!(n, trailing_len);
            }
            other => panic!("Expected TrailingBytes({trailing_len}), got {other:?}"),
        }
    }
}

#[test]
fn test_fuzz_malicious_length_headers() {
    // Maliciously forged vector lengths to attempt heap allocation bombs
    let malicious_lengths: Vec<u32> = vec![
        16 * 1024 * 1024 + 1, // 1 byte over MAX_VECTOR_LENGTH
        32 * 1024 * 1024,
        100 * 1024 * 1024,
        1024 * 1024 * 1024,
        u32::MAX - 1,
        u32::MAX,
    ];

    for len in malicious_lengths {
        // Construct transaction buffer with malicious input_count
        let mut buf = Vec::new();
        buf.extend_from_slice(&TRANSACTION_VERSION_1.to_le_bytes()); // version
        buf.extend_from_slice(&len.to_le_bytes()); // input_count

        let res = Transaction::from_canonical_bytes(&buf);
        match res {
            Err(SerializationError::LengthExceedsLimit { length, max }) => {
                assert_eq!(length, len as usize);
                assert_eq!(max, 16 * 1024 * 1024);
            }
            other => panic!("Expected LengthExceedsLimit for {len}, got {other:?}"),
        }
    }
}
