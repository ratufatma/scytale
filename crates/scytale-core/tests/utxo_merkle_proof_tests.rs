use scytale_core::{
    compute_utxo_leaf, generate_utxo_merkle_proof, Hash256, OutPoint, TxOut, UtxoEntry,
    UtxoEntryWithOutpoint, UtxoSet,
};

fn create_mock_utxo(txid_byte: u8, index: u32, value: u64, lock_byte: u8) -> (OutPoint, UtxoEntry) {
    let txid = Hash256::new([txid_byte; 32]);
    let op = OutPoint::new(txid, index);
    let out = TxOut::new(value, vec![lock_byte; 32]);
    let entry = UtxoEntry::new(out, 1, false);
    (op, entry)
}

#[test]
fn test_single_leaf_merkle_proof() {
    let (op, entry) = create_mock_utxo(1, 0, 100_000_000, 0xaa);
    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(op, entry.clone());

    let expected_root = utxo_set.compute_utxo_root();
    assert_eq!(
        expected_root,
        compute_utxo_leaf(&op, &entry.output),
        "single-leaf tree root equals the leaf hash"
    );

    let entries_vec = utxo_set.to_entries_with_outpoints();
    let proof = generate_utxo_merkle_proof(&entries_vec, &op).unwrap();

    assert_eq!(proof.outpoint, op);
    assert_eq!(proof.value_quanta, 100_000_000);
    assert_eq!(proof.leaf_hash, compute_utxo_leaf(&op, &entry.output));
    assert!(proof.audit_path.is_empty(), "single leaf has no siblings");
    assert_eq!(proof.leaf_index, 0);

    // Verify against correct root
    assert!(proof.verify(&expected_root));

    // Rejection on invalid root
    let bogus_root = Hash256::new([0x99; 32]);
    assert!(!proof.verify(&bogus_root));
}

#[test]
fn test_even_and_odd_multi_leaf_merkle_proof() {
    // Test with 2 leaves (even)
    let (op1, e1) = create_mock_utxo(0x10, 0, 500, 0x11);
    let (op2, e2) = create_mock_utxo(0x20, 0, 1000, 0x22);

    let mut set2 = UtxoSet::new();
    set2.insert(op1, e1.clone());
    set2.insert(op2, e2.clone());
    let root2 = set2.compute_utxo_root();

    let entries2 = set2.to_entries_with_outpoints();
    let p1 = generate_utxo_merkle_proof(&entries2, &op1).unwrap();
    let p2 = generate_utxo_merkle_proof(&entries2, &op2).unwrap();

    assert!(p1.verify(&root2));
    assert!(p2.verify(&root2));
    assert_eq!(p1.audit_path.len(), 1);
    assert_eq!(p2.audit_path.len(), 1);

    // Test with 3 leaves (odd - exercises duplicate-last leaf logic)
    let (op3, e3) = create_mock_utxo(0x30, 0, 1500, 0x33);
    let mut set3 = set2.clone();
    set3.insert(op3, e3.clone());
    let root3 = set3.compute_utxo_root();

    let entries3 = set3.to_entries_with_outpoints();
    for op in &[op1, op2, op3] {
        let proof = generate_utxo_merkle_proof(&entries3, op).unwrap();
        assert!(
            proof.verify(&root3),
            "proof for outpoint {:?} must verify against 3-leaf root",
            op
        );
        assert_eq!(proof.audit_path.len(), 2, "3-leaf tree has depth 2");
    }

    // Test with 7 leaves (odd multi-depth)
    let mut set7 = UtxoSet::new();
    for i in 1..=7 {
        let (op, e) = create_mock_utxo(i as u8, 0, i * 100, i as u8);
        set7.insert(op, e);
    }
    let root7 = set7.compute_utxo_root();
    let entries7 = set7.to_entries_with_outpoints();

    for entry in &entries7 {
        let proof = generate_utxo_merkle_proof(&entries7, &entry.outpoint).unwrap();
        assert!(proof.verify(&root7));
    }
}

#[test]
fn test_merkle_proof_tamper_resistance() {
    let mut set = UtxoSet::new();
    let mut outpoints = Vec::new();
    for i in 1..=5 {
        let (op, e) = create_mock_utxo(i as u8 * 0x11, 0, i * 10_000, i as u8);
        set.insert(op, e);
        outpoints.push(op);
    }
    let expected_root = set.compute_utxo_root();
    let entries = set.to_entries_with_outpoints();

    let valid_proof = generate_utxo_merkle_proof(&entries, &outpoints[2]).unwrap();
    assert!(valid_proof.verify(&expected_root));

    // 1. Tamper value_quanta
    let mut tampered_value = valid_proof.clone();
    tampered_value.value_quanta += 1;
    assert!(
        !tampered_value.verify(&expected_root),
        "tampered value_quanta must be rejected"
    );

    // 2. Tamper leaf_hash
    let mut tampered_leaf = valid_proof.clone();
    tampered_leaf.leaf_hash = Hash256::new([0xfe; 32]);
    assert!(
        !tampered_leaf.verify(&expected_root),
        "tampered leaf_hash must be rejected"
    );

    // 3. Tamper audit path sibling
    let mut tampered_sibling = valid_proof.clone();
    if let Some((sibling, _)) = tampered_sibling.audit_path.first_mut() {
        *sibling = Hash256::new([0xbb; 32]);
    }
    assert!(
        !tampered_sibling.verify(&expected_root),
        "tampered sibling hash must be rejected"
    );

    // 4. Tamper audit path orientation (is_right_sibling flip)
    let mut tampered_orientation = valid_proof.clone();
    if let Some((_, is_right)) = tampered_orientation.audit_path.first_mut() {
        *is_right = !*is_right;
    }
    assert!(
        !tampered_orientation.verify(&expected_root),
        "tampered orientation must be rejected"
    );

    // 5. Tamper expected root
    let false_root = Hash256::new([0x12; 32]);
    assert!(
        !valid_proof.verify(&false_root),
        "valid proof with wrong expected_root must be rejected"
    );
}

#[test]
fn test_proof_generation_missing_outpoint_fails() {
    let (op1, e1) = create_mock_utxo(1, 0, 100, 1);
    let (missing_op, _) = create_mock_utxo(2, 0, 200, 2);

    let entries = vec![UtxoEntryWithOutpoint::new(op1, e1)];
    let res = generate_utxo_merkle_proof(&entries, &missing_op);
    assert!(res.is_err(), "missing outpoint must yield error");
}
