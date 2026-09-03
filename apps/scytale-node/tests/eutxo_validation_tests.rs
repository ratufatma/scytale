//! Integration tests: ScyVM eUTXO smart contract validation in mempool and block pipeline.
//!
//! These tests exercise the full validation path:
//!   OutputLock::Script -> ScyVM::execute_validator -> accept/reject decision
//! using the compiled `scytale_contract_vault.wasm` Autonomous Timelock Vault contract.

use scytale_core::{
    verify_transaction_eutxo, EutxoValidationError, OutputLock, OutPoint, Transaction,
    TxIn, TxInput, TxOut, TxOutput, UtxoEntry, UtxoSet, Hash256, TRANSACTION_VERSION_1,
    MAX_TX_GAS,
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

#[derive(Serialize, Deserialize)]
enum VaultRedeemer {
    NormalWithdraw { sig_valid: bool },
    EmergencyRescue { penalty_accepted: bool },
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn load_vault_wasm() -> Vec<u8> {
    // Resolve the wasm artifact from the build output directory.
    // Cargo sets CARGO_MANIFEST_DIR to the test crate's manifest dir.
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
    let lock = OutputLock::Script { script_hash, datum: datum_bytes };
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

// ── Test 1: Reject before unlock_time ─────────────────────────────────────────

#[test]
fn test_vault_rejected_before_unlock_time() {
    let wasm = load_vault_wasm();
    let unlock_time: u64 = 1_800_000_000;

    let datum = VaultDatum {
        owner_pubkey: [0u8; 32],
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 1_000,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");

    let redeemer = VaultRedeemer::NormalWithdraw { sig_valid: true };
    let redeemer_bytes = bincode::serialize(&redeemer).expect("redeemer serialization failed");

    let (outpoint, utxo_entry) = make_script_utxo(&wasm, datum_bytes, 5_000_000);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, utxo_entry);

    let tx = make_spending_tx(outpoint, wasm, redeemer_bytes, 4_999_000);

    // block_time = 1_700_000_000 < unlock_time => should be REJECTED
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

    let datum = VaultDatum {
        owner_pubkey: [0u8; 32],
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 1_000,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");

    let redeemer = VaultRedeemer::NormalWithdraw { sig_valid: true };
    let redeemer_bytes = bincode::serialize(&redeemer).expect("redeemer serialization failed");

    let (outpoint, utxo_entry) = make_script_utxo(&wasm, datum_bytes, 5_000_000);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, utxo_entry);

    let tx = make_spending_tx(outpoint, wasm, redeemer_bytes, 4_999_000);

    // block_time = 1_850_000_000 > unlock_time => should PASS
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
}

// ── Test 3: Gas limit enforcement ─────────────────────────────────────────────

#[test]
fn test_vault_gas_limit_exceeded() {
    let wasm = load_vault_wasm();
    let unlock_time: u64 = 1_000;

    let datum = VaultDatum {
        owner_pubkey: [0u8; 32],
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 0,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");

    let redeemer = VaultRedeemer::NormalWithdraw { sig_valid: true };
    let redeemer_bytes = bincode::serialize(&redeemer).expect("redeemer serialization failed");

    let (outpoint, utxo_entry) = make_script_utxo(&wasm, datum_bytes, 1_000_000);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, utxo_entry);

    let tx = make_spending_tx(outpoint, wasm, redeemer_bytes, 999_000);

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
    let datum = VaultDatum {
        owner_pubkey: [0u8; 32],
        unlock_time: 1_000,
        emergency_key: [0u8; 32],
        penalty_fee: 0,
    };
    let datum_bytes = bincode::serialize(&datum).expect("datum serialization failed");
    let redeemer = VaultRedeemer::NormalWithdraw { sig_valid: true };
    let redeemer_bytes = bincode::serialize(&redeemer).expect("redeemer serialization failed");

    // Intentionally use wrong hash
    let wrong_hash = [0xABu8; 32];
    let lock = OutputLock::Script { script_hash: wrong_hash, datum: datum_bytes };
    let tx_out = TxOutput::new(1_000_000, lock).to_tx_out();
    let outpoint = OutPoint::new(Hash256::hash(b"dummy_prev_tx_hashfail"), 0);
    let entry = UtxoEntry::new(tx_out, 1, false);

    let mut utxo_set = UtxoSet::new();
    utxo_set.insert(outpoint, entry);

    let tx = make_spending_tx(outpoint, wasm, redeemer_bytes, 999_000);

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
