use scytale_core::{
    Block, BlockError, BlockHeader, CanonicalDeserialize, CanonicalSerialize, Hash256, OutPoint,
    Transaction, TxIn, TxOut, TRANSACTION_VERSION_1,
};

#[test]
fn test_block_header_serialization_roundtrip() {
    let prev_hash = Hash256::hash(b"previous_block");
    let tx_root = Hash256::hash(b"tx_commitment_root");

    let header = BlockHeader::new(1, prev_hash, tx_root, 1700000000, 0x1d00ffff, 42);

    let bytes = header.to_canonical_bytes().unwrap();
    assert_eq!(bytes.len(), 4 + 32 + 32 + 8 + 4 + 8); // Exactly 88 bytes

    let decoded = BlockHeader::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(decoded, header);
    assert_eq!(decoded.hash(), header.hash());
}

#[test]
fn test_block_serialization_roundtrip() {
    let header = BlockHeader::new(
        1,
        Hash256::ZERO,
        Hash256::hash(b"txs"),
        1700000000,
        0x1d00ffff,
        12345,
    );

    let coinbase = Transaction::new_coinbase(0, vec![TxOut::new(1_000_000_000, vec![1, 2, 3])]);
    let tx1 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(
            OutPoint::new(Hash256::hash(b"source"), 0),
            vec![4, 5],
        )],
        vec![TxOut::new(900_000_000, vec![6, 7])],
        0,
    );

    let block = Block::new(header, vec![coinbase, tx1]);

    let bytes = block.to_canonical_bytes().unwrap();
    let decoded_block = Block::from_canonical_bytes(&bytes).unwrap();
    assert_eq!(decoded_block, block);
    assert_eq!(decoded_block.header.hash(), block.header.hash());
}

#[test]
fn test_valid_minimal_block_coinbase_only() {
    let header = BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, 1700000000, 0x1d00ffff, 0);

    let coinbase = Transaction::new_coinbase(0, vec![TxOut::new(1_000_000_000, vec![1, 2, 3])]);
    let block = Block::new(header, vec![coinbase]);

    assert!(block.validate_structure().is_ok());
}

#[test]
fn test_valid_block_with_regular_transactions() {
    let header = BlockHeader::new(
        1,
        Hash256::hash(b"prev"),
        Hash256::hash(b"root"),
        1700000001,
        0x1d00ffff,
        999,
    );

    let coinbase = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![])]);

    let tx1 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(
            OutPoint::new(Hash256::hash(b"tx1_src"), 0),
            vec![1],
        )],
        vec![TxOut::new(100_000_000, vec![2])],
        0,
    );

    let tx2 = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(
            OutPoint::new(Hash256::hash(b"tx2_src"), 0),
            vec![3],
        )],
        vec![TxOut::new(200_000_000, vec![4])],
        0,
    );

    let block = Block::new(header, vec![coinbase, tx1, tx2]);
    assert!(block.validate_structure().is_ok());
}

#[test]
fn test_reject_empty_transactions() {
    let header = BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, 1700000000, 0x1d00ffff, 0);

    let block = Block::new(header, vec![]);
    assert_eq!(
        block.validate_structure(),
        Err(BlockError::EmptyTransactionVector)
    );
}

#[test]
fn test_reject_missing_coinbase_at_index_0() {
    let header = BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, 1700000000, 0x1d00ffff, 0);

    // Regular transaction at index 0
    let regular_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(OutPoint::new(Hash256::hash(b"src"), 0), vec![])],
        vec![TxOut::new(100_000_000, vec![])],
        0,
    );

    let block = Block::new(header, vec![regular_tx]);
    assert_eq!(block.validate_structure(), Err(BlockError::MissingCoinbase));
}

#[test]
fn test_reject_duplicate_coinbase() {
    let header = BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, 1700000000, 0x1d00ffff, 0);

    let coinbase_1 = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![])]);
    let coinbase_2 = Transaction::new_coinbase(1, vec![TxOut::new(500_000_000, vec![])]);

    let block = Block::new(header, vec![coinbase_1, coinbase_2]);
    assert_eq!(
        block.validate_structure(),
        Err(BlockError::DuplicateCoinbase(1))
    );
}

#[test]
fn test_reject_nested_invalid_transaction() {
    let header = BlockHeader::new(1, Hash256::ZERO, Hash256::ZERO, 1700000000, 0x1d00ffff, 0);

    let coinbase = Transaction::new_coinbase(1, vec![TxOut::new(1_000_000_000, vec![])]);

    // Transaction with duplicate input
    let op = OutPoint::new(Hash256::hash(b"same_input"), 0);
    let invalid_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![TxIn::new(op, vec![1]), TxIn::new(op, vec![2])],
        vec![TxOut::new(100_000_000, vec![])],
        0,
    );

    let block = Block::new(header, vec![coinbase, invalid_tx]);
    assert!(block.validate_structure().is_err());
}
