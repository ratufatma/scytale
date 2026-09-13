//! Mass-Scale Adversarial Penetration & Fuzz Testing Suite for Scytale Nodes.
//!
//! Generates and fires 120+ distinct adversarial payloads across 5 vulnerability categories:
//! 1. Malformed & Corrupted JSON Bodies (25 cases)
//! 2. eUTXO Quanta Overflow, Underflow & Balance Violations (25 cases)
//! 3. Fuzzed Bytecode, Malformed Scripts & Opcode Explosions (30 cases)
//! 4. Transaction Structure, Locktime & Replay Violations (20 cases)
//! 5. Alias Hijacking, Cryptographic Signature & Identity Fuzzing (20 cases)

use ed25519_dalek::SigningKey;
use scytale_core::{
    codec::CanonicalSerialize, Address, OutPoint, Transaction, TxIn, TxOut,
    TRANSACTION_VERSION_1,
};
use scytale_primitives::{to_hex, Hash256};
use scytale_script::{builder::ScriptBuilder, opcode::OpCode};
use serde_json::json;
use std::time::Duration;

fn get_target_url() -> String {
    std::env::var("TARGET_URL").unwrap_or_else(|_| "http://127.0.0.1:8333".to_string())
}

fn send_raw_tx_json(body: serde_json::Value) -> (u16, String) {
    let url = format!("{}/api/v1/tx", get_target_url());
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_json(body);

    match resp {
        Ok(res) => (res.status(), res.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(code, res)) => (code, res.into_string().unwrap_or_default()),
        Err(ureq::Error::Transport(e)) => (599, e.to_string()),
    }
}

fn send_tx_hex(tx_hex: &str) -> (u16, String) {
    send_raw_tx_json(json!({ "tx_hex": tx_hex }))
}

fn send_alias_bind_raw(body: serde_json::Value) -> (u16, String) {
    let url = format!("{}/api/v1/alias/bind", get_target_url());
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_json(body);

    match resp {
        Ok(res) => (res.status(), res.into_string().unwrap_or_default()),
        Err(ureq::Error::Status(code, res)) => (code, res.into_string().unwrap_or_default()),
        Err(ureq::Error::Transport(e)) => (599, e.to_string()),
    }
}

fn verify_rejected(test_num: usize, name: &str, code: u16, body: &str) -> bool {
    // A test passes if the server rejects it cleanly (4xx) and does not accept it (200)
    // or crash (5xx/599).
    let is_clean_rejection = (400..=499).contains(&code);
    if is_clean_rejection {
        println!("  [{test_num:03}] PASS: {name} -> HTTP {code} (Safe Rejection)");
        true
    } else {
        println!("  [{test_num:03}] FAIL: {name} -> HTTP {code} (UNEXPECTED: {body})");
        false
    }
}

fn main() {
    let target = get_target_url();
    println!("================================================================================");
    println!("SCYTALE MASS-SCALE ADVERSARIAL PENETRATION & FUZZING SUITE");
    println!("Target Node: {target}");
    println!("================================================================================\n");

    let mut total_tests = 0;
    let mut passed_tests = 0;

    // =========================================================================
    // CATEGORY 1: Malformed & Corrupted JSON Payloads (25 cases)
    // =========================================================================
    println!("--- CATEGORY 1: Malformed & Corrupted JSON Bodies (25 cases) ---");
    let json_cases: Vec<(&str, serde_json::Value)> = vec![
        ("Empty JSON Object", json!({})),
        ("Integer tx_hex", json!({ "tx_hex": 12345678 })),
        ("Boolean tx_hex", json!({ "tx_hex": true })),
        ("Array tx_hex", json!({ "tx_hex": ["dead", "beef"] })),
        ("Null tx_hex", json!({ "tx_hex": null })),
        ("Nested Object tx_hex", json!({ "tx_hex": { "inner": "payload" } })),
        ("Odd length hex string", json!({ "tx_hex": "12345" })),
        ("Single hex character", json!({ "tx_hex": "a" })),
        ("Non-hex characters in payload", json!({ "tx_hex": "xyzg1234hello!" })),
        ("SQL injection payload in hex", json!({ "tx_hex": "' OR '1'='1" })),
        ("Path traversal attempt", json!({ "tx_hex": "../../../../etc/passwd" })),
        ("Null byte injection", json!({ "tx_hex": "1200\u{0000}3456" })),
        ("Control characters in hex", json!({ "tx_hex": "12\r\n\t34" })),
        ("0x prefix only", json!({ "tx_hex": "0x" })),
        ("0x with odd length", json!({ "tx_hex": "0xabc" })),
        ("Uppercase invalid hex", json!({ "tx_hex": "0xGHIJKLMNOP" })),
        ("Unicode emoji in hex", json!({ "tx_hex": "0x1234🔥5678" })),
        ("Empty string tx_hex", json!({ "tx_hex": "" })),
        ("Whitespace only tx_hex", json!({ "tx_hex": "    " })),
        ("Large junk hex 1KB", json!({ "tx_hex": "ab".repeat(512) })),
        ("Large junk hex 10KB", json!({ "tx_hex": "cd".repeat(5120) })),
        ("Extra unknown field injection", json!({ "tx_hex": "deadbeef", "admin_override": true })),
        ("Root JSON array instead of object", json!(["tx_hex", "deadbeef"])),
        ("Root JSON string", json!("just_a_raw_string")),
        ("Root JSON integer", json!(999999)),
    ];

    for (name, payload) in json_cases {
        total_tests += 1;
        let (code, body) = send_raw_tx_json(payload);
        if verify_rejected(total_tests, name, code, &body) {
            passed_tests += 1;
        }
        std::thread::sleep(Duration::from_millis(35));
    }

    // =========================================================================
    // CATEGORY 2: eUTXO Quanta Overflow & Balance Violations (25 cases)
    // =========================================================================
    println!("\n--- CATEGORY 2: eUTXO Quanta Overflow & Balance Violations (25 cases) ---");
    let amounts_to_test: Vec<(&str, u64)> = vec![
        ("Max u64 value", u64::MAX),
        ("Max u64 - 1", u64::MAX - 1),
        ("Half max u64 + 100", u64::MAX / 2 + 100),
        ("Near max u64 value", u64::MAX - 5000),
        ("21 million SCY inflation", 21_000_000 * 100_000_000),
        ("One hundred million SCY", 100_000_000 * 100_000_000),
        ("Fifty million SCY", 50_000_000 * 100_000_000),
        ("Zero quanta output", 0),
        ("Single satoshi/quanta output with fake parent", 1),
        ("Slightly above block subsidy (26 SCY)", 26 * 100_000_000),
    ];

    for (desc, amt) in amounts_to_test {
        total_tests += 1;
        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(
                OutPoint::new(Hash256::hash(desc.as_bytes()), 0),
                vec![0x51],
            )],
            vec![TxOut::new(amt, vec![0x51])],
            0,
        );
        let tx_hex = to_hex(&tx.to_canonical_bytes().unwrap());
        let (code, body) = send_tx_hex(&tx_hex);
        if verify_rejected(total_tests, &format!("Single Output {desc}"), code, &body) {
            passed_tests += 1;
        }
        std::thread::sleep(Duration::from_millis(35));
    }

    // Multi-output overflow pairs
    for i in 1..=15 {
        total_tests += 1;
        let val1 = u64::MAX / 2 + (i as u64 * 1000);
        let val2 = u64::MAX / 2 + (i as u64 * 2000);
        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(
                OutPoint::new(Hash256::hash(format!("overflow_pair_{i}").as_bytes()), 0),
                vec![0x51],
            )],
            vec![
                TxOut::new(val1, vec![0x51]),
                TxOut::new(val2, vec![0x51]),
            ],
            0,
        );
        let tx_hex = to_hex(&tx.to_canonical_bytes().unwrap());
        let (code, body) = send_tx_hex(&tx_hex);
        if verify_rejected(total_tests, &format!("Cumulative Sum Overflow Pair #{i}"), code, &body) {
            passed_tests += 1;
        }
        std::thread::sleep(Duration::from_millis(35));
    }

    // =========================================================================
    // CATEGORY 3: Fuzzed Bytecode, Malformed Scripts & Opcode Bombs (30 cases)
    // =========================================================================
    println!("\n--- CATEGORY 3: Fuzzed Bytecode & Opcode Explosions (30 cases) ---");
    let script_attack_templates: Vec<(&str, Vec<u8>)> = vec![
        ("Stack underflow OP_DROP", vec![OpCode::OpDrop as u8]),
        ("Stack underflow OP_DUP", vec![OpCode::OpDup as u8]),
        ("Stack underflow OP_2DUP", vec![OpCode::Op2Dup as u8]),
        ("Stack underflow OP_SWAP", vec![OpCode::OpSwap as u8]),
        ("Stack underflow OP_ROT", vec![OpCode::OpRot as u8]),
        ("Stack underflow OP_ADD", vec![OpCode::OpAdd as u8]),
        ("Stack underflow OP_SUB", vec![OpCode::OpSub as u8]),
        ("Stack underflow OP_LESSTHAN", vec![OpCode::OpLessThan as u8]),
        ("Stack underflow OP_GREATERTHAN", vec![OpCode::OpGreaterThan as u8]),
        ("Stack underflow OP_EQUAL", vec![OpCode::OpEqual as u8]),
        ("Stack underflow OP_EQUALVERIFY", vec![OpCode::OpEqualVerify as u8]),
        ("OP_EQUALVERIFY on mismatched operands", vec![OpCode::Op1 as u8, OpCode::Op2 as u8, OpCode::OpEqualVerify as u8]),
        ("Unclosed OP_IF block", vec![OpCode::Op1 as u8, OpCode::OpIf as u8, OpCode::Op1 as u8]),
        ("Orphaned OP_ELSE without IF", vec![OpCode::OpElse as u8]),
        ("Orphaned OP_ENDIF without IF", vec![OpCode::OpEndIf as u8]),
        ("Multiple nested OP_IF without closure", vec![OpCode::Op1 as u8, OpCode::OpIf as u8, OpCode::Op1 as u8, OpCode::OpIf as u8]),
        ("CheckSig on empty stack", vec![OpCode::OpCheckSig as u8]),
        ("CheckSig with garbage pubkey", vec![OpCode::Op1 as u8, OpCode::OpCheckSig as u8]),
        ("CheckSigVerify with empty stack", vec![OpCode::OpCheckSigVerify as u8]),
        ("CheckLockTimeVerify on empty stack", vec![OpCode::OpCheckLockTimeVerify as u8]),
        ("Blake3 hash on empty stack", vec![OpCode::OpBlake3 as u8]),
        ("Raw match bypass trick (unlocking == locking == 20 bytes)", vec![0x77; 20]),
        ("Raw match bypass trick (unlocking == locking == 32 bytes)", vec![0x88; 32]),
        ("Undefined opcode 0xFF", vec![0xFF]),
        ("Undefined opcode 0xFE", vec![0xFE]),
        ("Undefined opcode 0xEE", vec![0xEE]),
        ("Long chain of 100 OP_DUPs", vec![OpCode::OpDup as u8; 100]),
        ("Long chain of 200 OP_DROPs", vec![OpCode::OpDrop as u8; 200]),
        ("Random binary fuzz (64 bytes)", (0..64).map(|x| (x * 37) as u8).collect()),
        ("Random binary fuzz (128 bytes)", (0..128).map(|x| (x * 73) as u8).collect()),
    ];

    for (name, script) in script_attack_templates {
        total_tests += 1;
        let tx = Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(
                OutPoint::new(Hash256::hash(name.as_bytes()), 0),
                script.clone(),
            )],
            vec![TxOut::new(10_000, script)],
            0,
        );
        let tx_hex = to_hex(&tx.to_canonical_bytes().unwrap());
        let (code, body) = send_tx_hex(&tx_hex);
        if verify_rejected(total_tests, name, code, &body) {
            passed_tests += 1;
        }
        std::thread::sleep(Duration::from_millis(35));
    }

    // =========================================================================
    // CATEGORY 4: Structural, Locktime & Replay Violations (20 cases)
    // =========================================================================
    println!("\n--- CATEGORY 4: Structural, Locktime & Replay Violations (20 cases) ---");
    let structural_cases: Vec<(&str, Transaction)> = vec![
        ("Zero inputs standard tx", Transaction::new(TRANSACTION_VERSION_1, vec![], vec![TxOut::new(1000, vec![0x51])], 0)),
        ("Zero outputs standard tx", Transaction::new(TRANSACTION_VERSION_1, vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51])], vec![], 0)),
        ("Zero inputs and zero outputs", Transaction::new(TRANSACTION_VERSION_1, vec![], vec![], 0)),
        ("Version 0 transaction", Transaction::new(0, vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51])], vec![TxOut::new(1000, vec![0x51])], 0)),
        ("Version 2 transaction", Transaction::new(2, vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51])], vec![TxOut::new(1000, vec![0x51])], 0)),
        ("Version 255 transaction", Transaction::new(255, vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51])], vec![TxOut::new(1000, vec![0x51])], 0)),
        ("Future locktime u64::MAX", Transaction::new(TRANSACTION_VERSION_1, vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51])], vec![TxOut::new(1000, vec![0x51])], u64::MAX)),
        ("Future locktime 2_000_000_000", Transaction::new(TRANSACTION_VERSION_1, vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51])], vec![TxOut::new(1000, vec![0x51])], 2_000_000_000)),
        ("Duplicate inputs spending same OutPoint", Transaction::new(
            TRANSACTION_VERSION_1,
            vec![
                TxIn::new(OutPoint::new(Hash256::hash(b"dup_input"), 0), vec![0x51]),
                TxIn::new(OutPoint::new(Hash256::hash(b"dup_input"), 0), vec![0x51]),
            ],
            vec![TxOut::new(1000, vec![0x51])],
            0,
        )),
        ("Triplicate inputs spending same OutPoint", Transaction::new(
            TRANSACTION_VERSION_1,
            vec![
                TxIn::new(OutPoint::new(Hash256::hash(b"triplicate"), 0), vec![0x51]),
                TxIn::new(OutPoint::new(Hash256::hash(b"triplicate"), 0), vec![0x51]),
                TxIn::new(OutPoint::new(Hash256::hash(b"triplicate"), 0), vec![0x51]),
            ],
            vec![TxOut::new(1000, vec![0x51])],
            0,
        )),
        ("Direct coinbase height 1 to mempool", Transaction::new_coinbase(1, vec![TxOut::new(2_500_000_000, vec![0x51])])),
        ("Direct coinbase height 100 to mempool", Transaction::new_coinbase(100, vec![TxOut::new(2_500_000_000, vec![0x51])])),
        ("Direct coinbase with 0 subsidy", Transaction::new_coinbase(2, vec![TxOut::new(0, vec![0x51])])),
        ("Direct coinbase with inflated subsidy (100 SCY)", Transaction::new_coinbase(2, vec![TxOut::new(10_000_000_000, vec![0x51])])),
        ("Multi-input with 5 random fake OutPoints", Transaction::new(
            TRANSACTION_VERSION_1,
            (0..5).map(|i| TxIn::new(OutPoint::new(Hash256::hash(&[i as u8]), i), vec![0x51])).collect(),
            vec![TxOut::new(5000, vec![0x51])],
            0,
        )),
        ("Multi-input with 10 random fake OutPoints", Transaction::new(
            TRANSACTION_VERSION_1,
            (0..10).map(|i| TxIn::new(OutPoint::new(Hash256::hash(&[i as u8]), i), vec![0x51])).collect(),
            vec![TxOut::new(10000, vec![0x51])],
            0,
        )),
        ("Multi-output with 10 zero value outputs", Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51])],
            (0..10).map(|_| TxOut::new(0, vec![0x51])).collect(),
            0,
        )),
        ("Sequence number 0x00000000", Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51]).with_sequence(0)],
            vec![TxOut::new(1000, vec![0x51])],
            0,
        )),
        ("Sequence number 0xFFFFFFFE (RBF signal)", Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(OutPoint::new(Hash256::ZERO, 0), vec![0x51]).with_sequence(0xFFFFFFFE)],
            vec![TxOut::new(1000, vec![0x51])],
            0,
        )),
        ("High index OutPoint u32::MAX", Transaction::new(
            TRANSACTION_VERSION_1,
            vec![TxIn::new(OutPoint::new(Hash256::ZERO, u32::MAX), vec![0x51])],
            vec![TxOut::new(1000, vec![0x51])],
            0,
        )),
    ];

    for (name, tx) in structural_cases {
        total_tests += 1;
        let tx_hex = to_hex(&tx.to_canonical_bytes().unwrap());
        let (code, body) = send_tx_hex(&tx_hex);
        if verify_rejected(total_tests, name, code, &body) {
            passed_tests += 1;
        }
        std::thread::sleep(Duration::from_millis(35));
    }

    // =========================================================================
    // CATEGORY 5: Alias Hijacking, Signature & Identity Fuzzing (20 cases)
    // =========================================================================
    println!("\n--- CATEGORY 5: Alias Hijacking & Identity Fuzzing (20 cases) ---");
    let key_bytes = [0x77u8; 32];
    let sk = SigningKey::from_bytes(&key_bytes);
    let pk = sk.verifying_key().to_bytes();
    let valid_addr = Address::from_pubkey_hash(*blake3::hash(&pk).as_bytes()).to_string();

    let alias_fuzz_cases: Vec<(&str, serde_json::Value)> = vec![
        ("Invalid Account Format 'admin'", json!({ "passbook_id": valid_addr, "candidate": "admin", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Invalid Account Format 'root'", json!({ "passbook_id": valid_addr, "candidate": "root", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Invalid Account Format '12345'", json!({ "passbook_id": valid_addr, "candidate": "12345", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Invalid Account Lowercase 'scy-000001'", json!({ "passbook_id": valid_addr, "candidate": "scy-000001", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Empty candidate", json!({ "passbook_id": valid_addr, "candidate": "", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Candidate with special chars", json!({ "passbook_id": valid_addr, "candidate": "SCY-!@#$%", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Empty passbook_id", json!({ "passbook_id": "", "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Passbook ID invalid checksum", json!({ "passbook_id": "scy1invalidchecksum1234567890", "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Passbook ID random string", json!({ "passbook_id": "random_junk_string", "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
        ("Mismatched public key and passbook", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&[0x11; 32]), "signature": vec![0xaa; 64] })),
        ("Truncated public key (16 bytes)", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&[0x22; 16]), "signature": vec![0xaa; 64] })),
        ("Extended public key (48 bytes)", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&[0x33; 48]), "signature": vec![0xaa; 64] })),
        ("Invalid hex public key", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": "not_hex!", "signature": vec![0xaa; 64] })),
        ("Truncated signature (32 bytes)", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": vec![0xaa; 32] })),
        ("Truncated signature (0 bytes)", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": Vec::<u8>::new() })),
        ("Extended signature (128 bytes)", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": vec![0xaa; 128] })),
        ("Forged signature with 0x00 bytes", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": vec![0x00; 64] })),
        ("Forged signature with 0xFF bytes", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": vec![0xFF; 64] })),
        ("Forged signature with alternating pattern", json!({ "passbook_id": valid_addr, "candidate": "SCY-000001", "public_key": to_hex(&pk), "signature": (0..64).map(|x| (x % 256) as u8).collect::<Vec<_>>() })),
        ("Candidate overflow 'SCY-9999999'", json!({ "passbook_id": valid_addr, "candidate": "SCY-9999999", "public_key": to_hex(&pk), "signature": vec![0xaa; 64] })),
    ];

    for (name, payload) in alias_fuzz_cases {
        total_tests += 1;
        let (code, body) = send_alias_bind_raw(payload);
        if verify_rejected(total_tests, name, code, &body) {
            passed_tests += 1;
        }
        std::thread::sleep(Duration::from_millis(35));
    }

    println!("\n================================================================================");
    println!("MASS-SCALE PENETRATION RESULTS SUMMARY");
    println!("Total Adversarial Tests Fired: {total_tests}");
    println!("Successfully Blocked (Fail-Closed): {passed_tests}");
    println!("Unsafely Accepted / Breached: {}", total_tests - passed_tests);
    println!("Success Rate: {:.2}%", (passed_tests as f64 / total_tests as f64) * 100.0);
    println!("================================================================================");

    if passed_tests == total_tests {
        println!("VERDICT: ALL 120 FUZZING & INJECTION ATTACKS COMPLETELY BLOCKED.");
        println!("FAIL-CLOSED INTEGRITY AND ROBUSTNESS: 100% PERFECT.");
    } else {
        println!("VERDICT: VULNERABILITY DETECTED! INVESTIGATION REQUIRED.");
        std::process::exit(1);
    }
}
