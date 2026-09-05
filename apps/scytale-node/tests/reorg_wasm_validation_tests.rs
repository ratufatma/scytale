//! Integration tests: Chain Reorganization with ScyVM Wasm Contract Validation
//!
//! Verifies the **fail-closed** reorg security guarantee introduced in Tahap 4:
//! a competing fork that spends a Wasm-locked eUTXO with an invalid redeemer
//! (e.g. forged signature) must be rejected, and the canonical tip must never shift.
//!
//! Test matrix:
//!   A) reorg_branch_invalid_wasm_spend_is_rejected   – forge sig in competing block → tip stays
//!   B) reorg_branch_valid_wasm_spend_triggers_reorg  – real sig in competing block → tip shifts

use ed25519_dalek::{Signer, SigningKey};
use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, OutputLock, Transaction, TxInput, TxOut,
    TxOutput, UtxoEntry, UtxoSet, TRANSACTION_VERSION_1,
};
use scytale_node::{Node, NodeConfig};
use serde::{Deserialize, Serialize};
use tempfile::tempdir;

// ── Mirror vault types ────────────────────────────────────────────────────────

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
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a 64-byte signature array")
            }
            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                if v.len() == 64 {
                    let mut arr = [0u8; 64];
                    arr.copy_from_slice(v);
                    Ok(arr)
                } else {
                    Err(de::Error::invalid_length(v.len(), &self))
                }
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
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

/// Build and return a fresh node backed by a temporary directory.
fn make_test_node() -> Node {
    let temp = tempdir().expect("tempdir");
    let config = NodeConfig {
        data_dir: temp.path().to_path_buf(),
        mining_enabled: false,
        miner_payout_script: vec![0x51],
        shutdown_timeout_secs: 5,
        ..NodeConfig::default()
    };
    // Keep temp alive by leaking it (test process is short-lived).
    std::mem::forget(temp);
    let mut node = Node::open(config).expect("Node::open");
    node.start().expect("node.start");
    node
}

fn make_spending_tx(outpoint: OutPoint, wasm: Vec<u8>, redeemer: Vec<u8>, out_val: u64) -> Transaction {
    let input = TxInput::new(
        *outpoint.txid.as_bytes(),
        outpoint.index,
        None,
        Some(redeemer),
        Some(wasm),
    );
    let tx_in = input.to_tx_in();
    let tx_out = TxOut::new(out_val, vec![0x51]);
    Transaction::new(TRANSACTION_VERSION_1, vec![tx_in], vec![tx_out], 0)
}

/// Build a vault-spending transaction signed by `signing_key`.
fn make_valid_vault_spend(
    signing_key: &SigningKey,
    outpoint: OutPoint,
    wasm: &[u8],
    out_val: u64,
) -> Transaction {
    let placeholder = VaultRedeemer::NormalWithdraw { signature: [0u8; 64] };
    let placeholder_bytes = bincode::serialize(&placeholder).unwrap();
    let draft = make_spending_tx(outpoint, wasm.to_vec(), placeholder_bytes, out_val);
    let tx_hash = draft.compute_hash();
    let sig = signing_key.sign(&tx_hash).to_bytes();
    let real = VaultRedeemer::NormalWithdraw { signature: sig };
    make_spending_tx(outpoint, wasm.to_vec(), bincode::serialize(&real).unwrap(), out_val)
}

/// Build a vault-spending transaction with a deliberately forged (invalid) signature.
fn make_invalid_vault_spend(
    signing_key: &SigningKey,
    outpoint: OutPoint,
    wasm: &[u8],
    out_val: u64,
) -> Transaction {
    // Build a real signature first, then corrupt it before constructing the tx.
    let placeholder = VaultRedeemer::NormalWithdraw { signature: [0u8; 64] };
    let placeholder_bytes = bincode::serialize(&placeholder).unwrap();
    let draft = make_spending_tx(outpoint, wasm.to_vec(), placeholder_bytes, out_val);
    let tx_hash = draft.compute_hash();
    let mut sig = signing_key.sign(&tx_hash).to_bytes();
    sig[0] ^= 0xff; // Corrupt the signature
    let bad_redeemer = VaultRedeemer::NormalWithdraw { signature: sig };
    let bad_bytes = bincode::serialize(&bad_redeemer).unwrap();
    make_spending_tx(outpoint, wasm.to_vec(), bad_bytes, out_val)
}

/// Minimal helper: mine a block that extends `parent_hash` using the given
/// transactions.  Computes the correct UTXO root from `staged_utxos`.
fn mine_block(
    height: u64,
    parent_hash: Hash256,
    coinbase: Transaction,
    non_coinbase: Vec<Transaction>,
    staged_utxos: &mut UtxoSet,
    timestamp: u64,
) -> Block {
    // Apply coinbase outputs to staged UTXO
    let cb_txid = coinbase.txid();
    for (idx, output) in coinbase.outputs.iter().enumerate() {
        if output.locking_condition.first() != Some(&0x6a) {
            staged_utxos.insert(
                OutPoint::new(cb_txid, idx as u32),
                UtxoEntry::new(output.clone(), height, true),
            );
        }
    }
    // Apply non-coinbase txs
    for tx in &non_coinbase {
        for input in &tx.inputs {
            staged_utxos.remove(&input.previous_output);
        }
        let txid = tx.txid();
        for (idx, output) in tx.outputs.iter().enumerate() {
            if output.locking_condition.first() != Some(&0x6a) {
                staged_utxos.insert(
                    OutPoint::new(txid, idx as u32),
                    UtxoEntry::new(output.clone(), height, false),
                );
            }
        }
    }
    let utxo_root = staged_utxos.compute_utxo_root();
    let mut txs = vec![coinbase];
    txs.extend(non_coinbase);
    // Use minimal difficulty target (regtest-style); version=1, height is tracked by ChainTree
    let header = BlockHeader::new(1u32, parent_hash, Hash256::ZERO, utxo_root, timestamp, 0x207fffff, 0);
    Block::new(header, txs)
}

// ─────────────────────────────────────────────────────────────────────────────
// Test A: Reorg with INVALID Wasm spend must be rejected (fail-closed)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_reorg_with_invalid_wasm_contract_spend_is_rejected() {
    let wasm = load_vault_wasm();
    let signing_key = SigningKey::from_bytes(&[0x77u8; 32]);
    let owner_pubkey = signing_key.verifying_key().to_bytes();
    let unlock_time: u64 = 50; // already unlocked

    let datum = VaultDatum {
        owner_pubkey,
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 1_000,
    };
    let datum_bytes = bincode::serialize(&datum).unwrap();

    let node = make_test_node();
    let genesis_tip = node.canonical_tip();
    let subsidy = scytale_consensus::calculate_block_reward(1);

    // ── Block A1: canonical block 1 with a Vault eUTXO output ────────────────
    let script_hash = *blake3::hash(&wasm).as_bytes();
    let lock = OutputLock::Script { script_hash, datum: datum_bytes.clone() };
    let vault_txout = TxOutput::new(subsidy, lock).to_tx_out();

    let cb_a1 = Transaction::new_coinbase(1, vec![vault_txout.clone()]);
    let vault_outpoint = OutPoint::new(cb_a1.txid(), 0);

    let mut staged_a = node.query_utxo_set();
    let block_a1 = mine_block(1, genesis_tip, cb_a1.clone(), vec![], &mut staged_a, 100);
    assert!(node.submit_external_block(block_a1.clone()).unwrap(), "Block A1 accepted");
    assert_eq!(node.canonical_height(), 1);
    let a1_hash = block_a1.header.hash();
    let staged_after_a1 = staged_a.clone();

    // ── Block A2: extends A1 (plain coinbase) ────────────────────────────────
    let cb_a2 = Transaction::new_coinbase(2, vec![TxOut::new(subsidy, vec![0x51])]);
    let block_a2 = mine_block(2, a1_hash, cb_a2, vec![], &mut staged_a, 200);
    assert!(node.submit_external_block(block_a2.clone()).unwrap(), "Block A2 accepted");
    assert_eq!(node.canonical_height(), 2);
    let tip_a2 = node.canonical_tip();

    // ── Challenger Branch B: forks from A1 with FORGED signature spend of Vault ─────────
    // Common ancestor between A2 and B is A1, where the Vault eUTXO is unspent.
    let mut staged_b = staged_after_a1.clone();
    let invalid_spend_tx = make_invalid_vault_spend(
        &signing_key,
        vault_outpoint,
        &wasm,
        subsidy - 1_000,
    );
    let cb_b2 = Transaction::new_coinbase(2, vec![TxOut::new(subsidy, vec![0x51])]);
    let block_b2 = mine_block(
        2,
        a1_hash,
        cb_b2,
        vec![invalid_spend_tx],
        &mut staged_b,
        200,
    );

    // Block B2 is evaluated: ScyVM executes the Vault contract and REJECTS the forged signature!
    let b2_result = node.submit_external_block(block_b2.clone());
    assert!(
        b2_result.is_err(),
        "Block B2 with forged Wasm signature must be rejected, but got: {:?}",
        b2_result.ok()
    );

    // Tip MUST remain A2
    assert_eq!(node.canonical_tip(), tip_a2, "Canonical tip must remain A2 after failed B2 spend");
    assert_eq!(node.canonical_height(), 2, "Height must remain 2");

    // Even if attacker mines B3 on top of B2 to accumulate higher PoW:
    let cb_b3 = Transaction::new_coinbase(3, vec![TxOut::new(subsidy, vec![0x51])]);
    let block_b3 = mine_block(
        3,
        block_b2.header.hash(),
        cb_b3,
        vec![],
        &mut staged_b,
        300,
    );
    let b3_result = node.submit_external_block(block_b3);
    assert!(
        b3_result.is_err(),
        "Block B3 extending invalid branch B must be rejected, but got: {:?}",
        b3_result.ok()
    );
    assert_eq!(node.canonical_tip(), tip_a2, "Canonical tip must remain A2 after B3 attempt");
    println!("[OK] Reorg with forged Wasm signature correctly rejected; canonical tip unchanged at A2.");

    // ── Positive Scenario: Challenger Branch C with VALID signature spend of Vault ──────
    let mut staged_c = staged_after_a1;
    let valid_spend_tx = make_valid_vault_spend(
        &signing_key,
        vault_outpoint,
        &wasm,
        subsidy - 500,
    );
    let cb_c2 = Transaction::new_coinbase(2, vec![TxOut::new(subsidy, vec![0x51])]);
    let block_c2 = mine_block(
        2,
        a1_hash,
        cb_c2,
        vec![valid_spend_tx],
        &mut staged_c,
        200,
    );
    let c2_result = node.submit_external_block(block_c2.clone());
    assert!(c2_result.is_ok(), "Block C2 must be accepted: {:?}", c2_result.err());
    assert!(!c2_result.unwrap(), "C2 must not displace A2 yet (equal work, first-seen)");
    assert_eq!(node.canonical_tip(), tip_a2, "Tip stays at A2 before C3");

    // Block C3 extends C2 -> cumulative work of Branch C (3) > Canonical tip A2 (2) -> triggers reorg!
    let cb_c3 = Transaction::new_coinbase(3, vec![TxOut::new(subsidy, vec![0x51])]);
    let block_c3 = mine_block(
        3,
        block_c2.header.hash(),
        cb_c3,
        vec![],
        &mut staged_c,
        300,
    );
    let c3_result = node.submit_external_block(block_c3.clone());
    assert!(c3_result.is_ok(), "Block C3 must be accepted: {:?}", c3_result.err());
    assert!(c3_result.unwrap(), "C3 must become canonical tip after valid Wasm spend reorg");
    assert_eq!(node.canonical_height(), 3, "Canonical height must advance to 3");
    assert_eq!(node.canonical_tip(), block_c3.header.hash(), "Canonical tip must shift to C3");
    println!("[OK] Reorg with valid Wasm signature succeeded; canonical tip shifted to C3.");

    node.shutdown().unwrap();
}


// ─────────────────────────────────────────────────────────────────────────────
// Test B: Reorg with VALID Wasm spend must succeed (tip shifts)
//
// Topology:
//   Genesis → A1 (canonical, plain coinbase, work W)
//          ↘ B1 (fork, coinbase with Vault eUTXO output, work W)
//              → B2 (fork, valid Wasm spend of B1's vault, work W)
//                   → B2 has cumulative work 2W > A1's W → reorg fires.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_reorg_with_valid_wasm_contract_spend_triggers_reorg() {
    let wasm = load_vault_wasm();
    let signing_key = SigningKey::from_bytes(&[0x88u8; 32]);
    let owner_pubkey = signing_key.verifying_key().to_bytes();
    let unlock_time: u64 = 50; // already unlocked at any block_time > 50

    let datum = VaultDatum {
        owner_pubkey,
        unlock_time,
        emergency_key: [0u8; 32],
        penalty_fee: 0,
    };
    let datum_bytes = bincode::serialize(&datum).unwrap();

    let node = make_test_node();
    let genesis_tip = node.canonical_tip();
    let subsidy = scytale_consensus::calculate_block_reward(1);

    // ── A1: canonical block 1 — plain coinbase ────────────────────────────────
    let cb_a1 = Transaction::new_coinbase(1, vec![TxOut::new(subsidy, vec![0x51])]);
    let mut staged_a = node.query_utxo_set();
    let block_a1 = mine_block(1, genesis_tip, cb_a1, vec![], &mut staged_a, 100);
    assert!(node.submit_external_block(block_a1.clone()).unwrap(), "A1 accepted");
    let tip_a1 = node.canonical_tip();
    assert_eq!(node.canonical_height(), 1);

    // ── B1: fork from genesis — coinbase with Vault eUTXO output ─────────────
    // B1 has equal work to A1, so it stays a side branch (first-seen rule).
    // Crucially, the vault UTXO is created *within* the B-branch, so it is
    // consensually spendable by B2.
    let script_hash = *blake3::hash(&wasm).as_bytes();
    let lock = OutputLock::Script { script_hash, datum: datum_bytes.clone() };
    let vault_txout = TxOutput::new(subsidy, lock).to_tx_out();
    let cb_b1 = Transaction::new_coinbase(1, vec![vault_txout.clone()]);
    let vault_outpoint = OutPoint::new(cb_b1.txid(), 0);

    // Build the B-fork's staged UTXO starting from genesis (not A1).
    let mut staged_b = {
        let canonical = node.query_canonical_chain().unwrap();
        let genesis_block = canonical
            .into_iter()
            .find(|(_, h)| *h == 0)
            .map(|(b, _)| b)
            .unwrap();
        let mut s = UtxoSet::new();
        s.apply_block_transactions(
            &genesis_block.transactions[0],
            &genesis_block.transactions[1..],
            0,
        )
        .unwrap();
        s
    };
    let block_b1 = mine_block(1, genesis_tip, cb_b1, vec![], &mut staged_b, 150);
    let b1_result = node.submit_external_block(block_b1.clone()).unwrap();
    assert!(!b1_result, "B1 must not become canonical (equal work, first-seen rule)");
    assert_eq!(node.canonical_tip(), tip_a1, "Tip stays at A1 after B1");

    // ── B2: extends B1 with VALID vault spend ─────────────────────────────────
    // B2 has cumulative work = 2W > A1's W → triggers reorg.
    // vault_outpoint is in staged_b (added by mine_block processing of B1's coinbase).
    // The Wasm validator must accept the genuine Ed25519 signature.
    let valid_spend_tx =
        make_valid_vault_spend(&signing_key, vault_outpoint, &wasm, subsidy - 500);

    let cb_b2 = Transaction::new_coinbase(2, vec![TxOut::new(subsidy, vec![0x51])]);
    let block_b2 = mine_block(
        2,
        block_b1.header.hash(),
        cb_b2,
        vec![valid_spend_tx],
        &mut staged_b,
        200,
    );

    let b2_result = node.submit_external_block(block_b2.clone());
    assert!(
        b2_result.is_ok(),
        "Block B2 with valid Wasm signature must be accepted: {:?}",
        b2_result.err()
    );
    assert!(b2_result.unwrap(), "B2 must become canonical tip (more cumulative work)");
    assert_eq!(node.canonical_height(), 2, "Canonical height must advance to 2");
    assert_ne!(
        node.canonical_tip(),
        tip_a1,
        "Canonical tip must no longer be A1 after reorg to B2"
    );
    println!("[OK] Reorg with valid Wasm signature succeeded; tip advanced to height 2 (branch B).");

    node.shutdown().unwrap();
}

