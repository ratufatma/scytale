use ed25519_dalek::{Signer, SigningKey};
use scytale_core::{
    verify_transaction_eutxo, EutxoValidationError, Hash256, OutputLock, OutPoint, Transaction,
    TxIn, TxInput, TxOut, TxOutput, UtxoEntry, UtxoSet, MAX_TX_GAS, TRANSACTION_VERSION_1,
};
use serde::{Deserialize, Serialize};

// ── Vault contract structures (mirrored from contracts/vault/src/lib.rs) ─────

#[derive(Serialize, Deserialize)]
struct VaultDatum {
    owner_pubkey: [u8; 32],
    unlock_time: u64,
    emergency_key: [u8; 32],
    penalty_fee: u64,
}

mod serde_signature {
    use core::fmt;
    use serde::{de, Deserializer, Serializer};

    pub fn serialize<S>(sig: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(sig)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SigVisitor;
        impl<'de> de::Visitor<'de> for SigVisitor {
            type Value = [u8; 64];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a 64-byte signature array")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v.len() == 64 {
                    let mut arr = [0u8; 64];
                    arr.copy_from_slice(v);
                    Ok(arr)
                } else {
                    Err(de::Error::invalid_length(v.len(), &self))
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut arr = [0u8; 64];
                for (i, byte) in arr.iter_mut().enumerate() {
                    *byte = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_bytes(SigVisitor)
    }
}

#[derive(Serialize, Deserialize)]
enum VaultRedeemer {
    NormalWithdraw {
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
    },
    EmergencyRescue {
        penalty_accepted: bool,
    },
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_vault_wasm() -> Vec<u8> {
    let wasm_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm");
    std::fs::read(&wasm_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read vault wasm at {:?}: {}\n\
             Run `cargo build --target wasm32-unknown-unknown --release -p scytale-contract-vault` first.",
            wasm_path, e
        )
    })
}

fn make_script_utxo(
    wasm: &[u8],
    datum_bytes: Vec<u8>,
    value: u64,
) -> (OutPoint, UtxoEntry) {
    let script_hash = *blake3::hash(wasm).as_bytes();
    let lock = OutputLock::Script {
        script_hash,
        datum: datum_bytes,
    };
    let tx_out = TxOutput::new(value, lock).to_tx_out();
    let outpoint = OutPoint::new(Hash256::hash(b"dummy_prev_tx"), 0);
    let entry = UtxoEntry::new(tx_out, 1, false);
    (outpoint, entry)
}

fn make_spending_tx(
    outpoint: OutPoint,
    wasm: Vec<u8>,
    redeemer_bytes: Vec<u8>,
    output_value: u64,
) -> Transaction {
    let eutxo_input = TxInput::new(
        *outpoint.txid.as_bytes(),
        outpoint.index,
        None,
        Some(redeemer_bytes),
        Some(wasm),
    );
    let tx_in = eutxo_input.to_tx_in();
    let tx_out = TxOut::new(output_value, vec![0x51]); // OP_TRUE output
    Transaction::new(TRANSACTION_VERSION_1, vec![tx_in], vec![tx_out], 0)
}

fn make_signed_vault_tx(
    signing_key: &SigningKey,
    outpoint: OutPoint,
    wasm: &[u8],
    output_value: u64,
) -> (Transaction, [u8; 64]) {
    // 1. Build draft tx with placeholder signature to get deterministic tx_hash
    let placeholder_redeemer = VaultRedeemer::NormalWithdraw {
        signature: [0u8; 64],
    };
    let placeholder_bytes = bincode::serialize(&placeholder_redeemer).unwrap();
    let draft_tx = make_spending_tx(outpoint, wasm.to_vec(), placeholder_bytes, output_value);

    // 2. Sign deterministic transaction body hash
    let tx_hash = draft_tx.compute_hash();
    let signature = signing_key.sign(&tx_hash).to_bytes();

    // 3. Build final transaction with real signature
    let real_redeemer = VaultRedeemer::NormalWithdraw { signature };
    let real_bytes = bincode::serialize(&real_redeemer).unwrap();
    let final_tx = make_spending_tx(outpoint, wasm.to_vec(), real_bytes, output_value);

    (final_tx, signature)
}

fn make_tampered_signed_vault_tx(
    signing_key: &SigningKey,
    outpoint: OutPoint,
    wasm: &[u8],
    output_value: u64,
) -> Transaction {
    let (_, mut sig) = make_signed_vault_tx(signing_key, outpoint, wasm, output_value);
    sig[0] ^= 0xff; // Invalidate signature
    let tampered_redeemer = VaultRedeemer::NormalWithdraw { signature: sig };
    let tampered_bytes = bincode::serialize(&tampered_redeemer).unwrap();
    make_spending_tx(outpoint, wasm.to_vec(), tampered_bytes, output_value)
}

// ── Test 1: Reject before unlock_time ─────────────────────────────────────────

#[test]
fn test_vault_rejected_before_unlock_time() {
    let wasm = load_vault_wasm();
    let unlock_time: u64 = 1_800_000_000;

    let signing_key = SigningKey::from_bytes(&[0x11u8; 32]);
    let owner_pubkey = signing_key.verifying_key().to_bytes();

    let datum = VaultDatum {
        owner_pubkey,
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 1_000,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");

    let (outpoint, utxo_entry) = make_script_utxo(&wasm, datum_bytes, 5_000_000);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, utxo_entry);

    let (tx, _) = make_signed_vault_tx(&signing_key, outpoint, &wasm, 4_999_000);

    // block_time = 1_700_000_000 < unlock_time => should be REJECTED despite valid signature
    let block_time = 1_700_000_000u64;
    let result = verify_transaction_eutxo(&tx, block_time, &utxo_set, MAX_TX_GAS);

    assert!(
        result.is_err(),
        "Expected rejection before unlock_time, but got: {:?}",
        result
    );
    match result.unwrap_err() {
        EutxoValidationError::ValidationRejected => {} // Expected
        other => panic!("Expected ValidationRejected, got: {:?}", other),
    }
}

// ── Test 2: Accept after unlock_time ──────────────────────────────────────────

#[test]
fn test_vault_accepted_after_unlock_time() {
    let wasm = load_vault_wasm();
    let unlock_time: u64 = 1_800_000_000;

    let signing_key = SigningKey::from_bytes(&[0x11u8; 32]);
    let owner_pubkey = signing_key.verifying_key().to_bytes();

    let datum = VaultDatum {
        owner_pubkey,
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 1_000,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");

    let (outpoint, utxo_entry) = make_script_utxo(&wasm, datum_bytes, 5_000_000);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, utxo_entry);

    let (tx, _) = make_signed_vault_tx(&signing_key, outpoint, &wasm, 4_999_000);

    // block_time = 1_850_000_000 > unlock_time => should PASS with valid signature
    let block_time = 1_850_000_000u64;
    let result = verify_transaction_eutxo(&tx, block_time, &utxo_set, MAX_TX_GAS);

    assert!(
        result.is_ok(),
        "Expected acceptance after unlock_time, but got: {:?}",
        result
    );
    let gas_consumed = result.unwrap();
    assert!(gas_consumed > 0, "Gas consumed should be > 0 for script input");
    println!("[OK] Vault accepted. Gas consumed: {} fuel", gas_consumed);

    // Tampered signature must be REJECTED even after unlock_time
    let tampered_tx = make_tampered_signed_vault_tx(&signing_key, outpoint, &wasm, 4_999_000);
    let result_tampered = verify_transaction_eutxo(&tampered_tx, block_time, &utxo_set, MAX_TX_GAS);
    assert!(
        result_tampered.is_err(),
        "Tampered signature must be rejected by ScyVM"
    );
    match result_tampered.unwrap_err() {
        EutxoValidationError::ValidationRejected => {
            println!("[OK] Tampered signature correctly rejected by ScyVM.");
        }
        other => panic!("Expected ValidationRejected, got: {:?}", other),
    }
}

// ── Test 3: Gas limit enforcement ─────────────────────────────────────────────

#[test]
fn test_vault_gas_limit_exceeded() {
    let wasm = load_vault_wasm();
    let unlock_time: u64 = 1_000;

    let signing_key = SigningKey::from_bytes(&[0x11u8; 32]);
    let owner_pubkey = signing_key.verifying_key().to_bytes();

    let datum = VaultDatum {
        owner_pubkey,
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 0,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");

    let (outpoint, utxo_entry) = make_script_utxo(&wasm, datum_bytes, 1_000_000);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, utxo_entry);

    let (tx, _) = make_signed_vault_tx(&signing_key, outpoint, &wasm, 999_000);

    // Set gas_limit to 1 — far too low; any VM execution should fail with OutOfGas
    let result = verify_transaction_eutxo(&tx, 9_999_999_999, &utxo_set, 1);

    assert!(
        result.is_err(),
        "Expected GasLimitExceeded or VmExecutionFailed, got: {:?}",
        result
    );
    println!("[OK] Gas limit enforcement passed: {:?}", result.unwrap_err());
}

// ── Test 4: Script hash mismatch ─────────────────────────────────────────────

#[test]
fn test_vault_script_hash_mismatch() {
    let wasm = load_vault_wasm();
    let signing_key = SigningKey::from_bytes(&[0x11u8; 32]);
    let owner_pubkey = signing_key.verifying_key().to_bytes();

    let datum = VaultDatum {
        owner_pubkey,
        unlock_time: 1_000,
        emergency_key: [0u8; 32],
        penalty_fee: 0,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");

    // Intentionally use wrong hash
    let wrong_hash = [0xABu8; 32];
    let lock = OutputLock::Script {
        script_hash: wrong_hash,
        datum: datum_bytes,
    };
    let tx_out = TxOutput::new(1_000_000, lock).to_tx_out();
    let outpoint = OutPoint::new(Hash256::hash(b"dummy_prev_tx_hashfail"), 0);
    let entry = UtxoEntry::new(tx_out, 1, false);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, entry);

    let (tx, _) = make_signed_vault_tx(&signing_key, outpoint, &wasm, 999_000);

    let result = verify_transaction_eutxo(&tx, 9_999_999_999, &utxo_set, MAX_TX_GAS);
    assert!(result.is_err());
    match result.unwrap_err() {
        EutxoValidationError::ScriptHashMismatch { .. } => {}
        other => panic!("Expected ScriptHashMismatch, got: {:?}", other),
    }
    println!("[OK] Script hash mismatch correctly detected.");
}

// ── Test 5: Standard (non-script) transactions pass through unchanged ─────────

#[test]
fn test_standard_pkh_transaction_unaffected() {
    let prev_out = OutPoint::new(Hash256::hash(b"standard_tx"), 0);
    let tx_in = TxIn::new(prev_out, vec![0x01u8; 72]); // dummy signature
    let tx_out = TxOut::new(999_000, vec![0x51]); // OP_TRUE
    let tx = Transaction::new(TRANSACTION_VERSION_1, vec![tx_in], vec![tx_out], 0);

    // Standard P2PK UTXO (no Script lock)
    let utxo_tx_out = TxOut::new(1_000_000, vec![0x51]);
    let entry = UtxoEntry::new(utxo_tx_out, 1, false);
    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(prev_out, entry);

    // Should return Ok(0) gas — no script inputs
    let result = verify_transaction_eutxo(&tx, 1_700_000_000, &utxo_set, MAX_TX_GAS);
    assert_eq!(result.unwrap(), 0, "Non-script tx should consume 0 eUTXO gas");
    println!("[OK] Standard P2PK transaction passes eUTXO gate with 0 gas.");
}

// ── Test 6: Node submit_transaction eUTXO Wasm bypasses ScriptEngine blockade ──

#[test]
fn test_node_submit_transaction_eutxo_wasm_bypasses_script_engine_blockade() {
    use scytale_core::{Block, BlockHeader};
    use scytale_node::{error::NodeError, Node, NodeConfig};
    use tempfile::tempdir;

    let temp = tempdir().expect("Failed to create tempdir");
    let config = NodeConfig {
        data_dir: temp.path().to_path_buf(),
        mining_enabled: false,
        miner_payout_script: vec![0x01, 0x02, 0x03],
        shutdown_timeout_secs: 5,
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).expect("Failed to open node");
    node.start().expect("Failed to start node");

    let wasm = load_vault_wasm();
    let genesis_tip = node.canonical_tip();
    let subsidy = scytale_consensus::calculate_block_reward(1);

    // 1. Buat UTXO OutputLock::Script dengan unlock_time di masa lalu (100)
    let signing_key = SigningKey::from_bytes(&[0x42u8; 32]);
    let owner_pubkey = signing_key.verifying_key().to_bytes();

    let datum = VaultDatum {
        owner_pubkey,
        unlock_time: 100, // Waktu sudah terlampaui saat ini
        emergency_key: [0x99u8; 32],
        penalty_fee: 1_000,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");
    let script_hash = *blake3::hash(&wasm).as_bytes();
    let lock = OutputLock::Script {
        script_hash,
        datum: datum_bytes,
    };
    let script_tx_out = TxOutput::new(subsidy, lock).to_tx_out();

    // 2. Tambang Block 1 berisi output script eUTXO tersebut
    let cb1 = Transaction::new_coinbase(1, vec![script_tx_out.clone()]);
    let mut staging = node.query_utxo_set();
    staging.insert(
        OutPoint::new(cb1.txid(), 0),
        UtxoEntry::new(script_tx_out, 1, true),
    );
    let utxo_root = staging.compute_utxo_root();
    let header = BlockHeader::new(1, genesis_tip, Hash256::ZERO, utxo_root, 100, 0x207fffff, 0);
    let block1 = Block::new(header, vec![cb1.clone()]);
    assert!(
        node.submit_external_block(block1).unwrap(),
        "Block 1 with eUTXO script output must be accepted"
    );
    assert_eq!(node.canonical_height(), 1);

    let script_outpoint = OutPoint::new(cb1.txid(), 0);
    let current_utxos = node.query_utxo_set();
    assert!(
        current_utxos.get(&script_outpoint).is_some(),
        "Script UTXO must exist in node UTXO set"
    );

    // 3. Buat spending transaction eUTXO yang sah (NormalWithdraw dengan Ed25519 signature sah)
    let (valid_tx, _sig) = make_signed_vault_tx(&signing_key, script_outpoint, &wasm, subsidy - 1_000_000);

    // 3a. Verifikasi langsung bahwa verify_transaction_scripts tidak memicu error "Invalid opcode: 0x43"
    let script_check = Node::verify_transaction_scripts(&valid_tx, 1, &current_utxos);
    assert!(
        script_check.is_ok(),
        "verify_transaction_scripts must bypass ScriptEngine for OutputLock::Script without error: {:?}",
        script_check.err()
    );

    // 3b. Submit transaksi ke node mempool: harus lolos ScyVM dan berhasil admitted
    let submit_res = node.submit_transaction(valid_tx);
    assert!(
        submit_res.is_ok(),
        "submit_transaction for valid eUTXO contract must succeed without 'Invalid opcode: 0x43': {:?}",
        submit_res.err()
    );
    let txid = submit_res.unwrap();
    println!("[OK] eUTXO transaction admitted to mempool with TxID: {}", txid);

    // 4. Verifikasi transaksi invalid (NormalWithdraw dengan signature palsu) ditolak oleh ScyVM
    let invalid_tx = make_tampered_signed_vault_tx(&signing_key, script_outpoint, &wasm, subsidy - 1_000_000);

    let invalid_res = node.submit_transaction(invalid_tx);
    assert!(
        invalid_res.is_err(),
        "Invalid eUTXO redeemer must be rejected"
    );
    match invalid_res.unwrap_err() {
        NodeError::EutxoValidation(EutxoValidationError::ValidationRejected) => {
            println!("[OK] Correctly rejected by ScyVM with ValidationRejected (not InvalidOpCode).");
        }
        other => panic!("Expected EutxoValidation(ValidationRejected), got: {:?}", other),
    }

    node.shutdown().expect("Node shutdown failed");
}
