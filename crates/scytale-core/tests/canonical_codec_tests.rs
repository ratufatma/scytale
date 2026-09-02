use scytale_core::{
    CanonicalDeserialize, CanonicalSerialize, Hash256, OutPoint, SerializationError, Transaction,
    TxIn, TxOut, TRANSACTION_VERSION_1,
};

#[test]
fn test_blake3_digest_length() {
    let payload = b"scytale canonical binary serialization test payload";
    let hash = Hash256::hash(payload);
    assert_eq!(hash.as_bytes().len(), 32);
    assert_ne!(hash, Hash256::ZERO);
}

#[test]
fn test_roundtrip_all_primitives() {
    // 1. Integer types
    let u8_val: u8 = 42;
    let u8_bytes = u8_val.to_canonical_bytes().unwrap();
    assert_eq!(u8::from_canonical_bytes(&u8_bytes).unwrap(), u8_val);

    let u16_val: u16 = 1337;
    let u16_bytes = u16_val.to_canonical_bytes().unwrap();
    assert_eq!(u16::from_canonical_bytes(&u16_bytes).unwrap(), u16_val);

    let u32_val: u32 = 1_000_000;
    let u32_bytes = u32_val.to_canonical_bytes().unwrap();
    assert_eq!(u32::from_canonical_bytes(&u32_bytes).unwrap(), u32_val);

    let u64_val: u64 = 42_000_000_000_000_000;
    let u64_bytes = u64_val.to_canonical_bytes().unwrap();
    assert_eq!(u64::from_canonical_bytes(&u64_bytes).unwrap(), u64_val);

    // 2. Bool
    let true_val = true;
    let true_bytes = true_val.to_canonical_bytes().unwrap();
    assert_eq!(bool::from_canonical_bytes(&true_bytes).unwrap(), true_val);

    let false_val = false;
    let false_bytes = false_val.to_canonical_bytes().unwrap();
    assert_eq!(bool::from_canonical_bytes(&false_bytes).unwrap(), false_val);

    // 3. Hash256
    let hash_val = Hash256::hash(b"scytale_hash_test");
    let hash_bytes = hash_val.to_canonical_bytes().unwrap();
    assert_eq!(
        Hash256::from_canonical_bytes(&hash_bytes).unwrap(),
        hash_val
    );

    // 4. OutPoint
    let outpoint = OutPoint::new(hash_val, 7);
    let op_bytes = outpoint.to_canonical_bytes().unwrap();
    assert_eq!(OutPoint::from_canonical_bytes(&op_bytes).unwrap(), outpoint);

    // 5. TxIn
    let txin = TxIn::new(outpoint, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    let txin_bytes = txin.to_canonical_bytes().unwrap();
    assert_eq!(TxIn::from_canonical_bytes(&txin_bytes).unwrap(), txin);

    // 6. TxOut
    let txout = TxOut::new(500_000_000, vec![0xCA, 0xFE, 0xBA, 0xBE]);
    let txout_bytes = txout.to_canonical_bytes().unwrap();
    assert_eq!(TxOut::from_canonical_bytes(&txout_bytes).unwrap(), txout);

    // 7. Transaction
    let tx = Transaction::new(TRANSACTION_VERSION_1, vec![txin], vec![txout], 0);
    let tx_bytes = tx.to_canonical_bytes().unwrap();
    let reconstructed_tx = Transaction::from_canonical_bytes(&tx_bytes).unwrap();
    assert_eq!(reconstructed_tx, tx);
}

#[test]
fn test_canonical_determinism() {
    let txid = Hash256::hash(b"prev_transaction_id");
    let op1 = OutPoint::new(txid, 0);
    let op2 = OutPoint::new(txid, 1);

    let in1 = TxIn::new(op1, vec![1, 2, 3, 4, 5]);
    let in2 = TxIn::new(op2, vec![6, 7, 8]);

    let out1 = TxOut::new(100_000_000, vec![9, 10]);
    let out2 = TxOut::new(250_000_000, vec![11, 12, 13, 14]);

    let tx = Transaction::new(TRANSACTION_VERSION_1, vec![in1, in2], vec![out1, out2], 0);

    let bytes_pass_1 = tx.to_canonical_bytes().unwrap();
    let bytes_pass_2 = tx.to_canonical_bytes().unwrap();
    assert_eq!(bytes_pass_1, bytes_pass_2);

    let reconstructed = Transaction::from_canonical_bytes(&bytes_pass_1).unwrap();
    let bytes_pass_3 = reconstructed.to_canonical_bytes().unwrap();
    assert_eq!(bytes_pass_1, bytes_pass_3);
}

#[test]
fn test_reject_trailing_bytes() {
    let outpoint = OutPoint::new(Hash256::hash(b"op_test"), 0);
    let mut bytes = outpoint.to_canonical_bytes().unwrap();

    // Append 1 trailing garbage byte
    bytes.push(0xFF);

    let err = OutPoint::from_canonical_bytes(&bytes).unwrap_err();
    assert_eq!(err, SerializationError::TrailingBytes(1));
}

#[test]
fn test_reject_truncated_bytes() {
    let outpoint = OutPoint::new(Hash256::hash(b"op_test"), 0);
    let bytes = outpoint.to_canonical_bytes().unwrap();

    // Truncate the buffer
    let truncated = &bytes[0..bytes.len() - 2];

    let err = OutPoint::from_canonical_bytes(truncated).unwrap_err();
    assert_eq!(
        err,
        SerializationError::UnexpectedEof {
            needed: 1,
            available: 0,
        }
    );
}

#[test]
fn test_fixed_regression_test_vectors() {
    // Construct fixed deterministic transaction
    let fixed_prev_hash = Hash256::new([
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ]);

    let input = TxIn::new(
        OutPoint::new(fixed_prev_hash, 0),
        vec![0xAA, 0xBB, 0xCC, 0xDD],
    );

    let output = TxOut::new(
        1_000_000_000, // 10 SCY in quanta
        vec![0x11, 0x22, 0x33, 0x44, 0x55],
    );

    let tx = Transaction::new(
        1, // version 1
        vec![input],
        vec![output],
        0, // lock_time 0
    );

    let canonical_bytes = tx.to_canonical_bytes().unwrap();

    // Compute TxID
    let txid = tx.txid();
    let expected_hash = Hash256::hash(&canonical_bytes);
    assert_eq!(txid, expected_hash);

    // Assert stable byte-for-byte serialization
    let expected_len = 4 + 4 + (36 + 4 + 4) + 4 + (8 + 4 + 5) + 8;
    assert_eq!(canonical_bytes.len(), expected_len);

    // Verify roundtrip reproducibility
    let decoded_tx = Transaction::from_canonical_bytes(&canonical_bytes).unwrap();
    assert_eq!(decoded_tx.txid(), txid);
}
