use ed25519_dalek::{Signer, SigningKey};
use scy20::codec::{serialize_datum, serialize_redeemer};
use scy20::{Scy20Datum, Scy20Redeemer, TokenRegistryDatum, TokenId, TxContext};
use scytale_vm::ScyVM;
use std::process::Command;

fn load_wasm() -> Vec<u8> {
    let status = Command::new("cargo")
        .args(["build", "--target", "wasm32-unknown-unknown", "--release", "-p", "scy20"])
        .status()
        .expect("WASM build must start");
    assert!(status.success(), "SCY20 WASM build must succeed");
    std::fs::read("../../target/wasm32-unknown-unknown/release/scy20.wasm")
        .expect("SCY20 WASM artifact must exist")
}

fn key(seed: u8) -> (SigningKey, [u8; 32]) {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let address = signing_key.verifying_key().to_bytes();
    (signing_key, address)
}

fn encoded<T: serde::Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).expect("canonical state must encode")
}

#[test]
fn wasm_validates_canonical_host_state_and_rejects_replay() {
    let wasm = load_wasm();
    let token_id: TokenId = [0x5a; 32];
    let (alice_key, alice) = key(1);
    let (_, bob) = key(2);
    let tx_hash = [0xa1; 32];
    let registry = TokenRegistryDatum {
        token_id,
        authority: alice,
        current_supply: 100_000,
        max_supply: Some(100_000),
        nonce: 4,
    };
    let next_registry = TokenRegistryDatum { nonce: 5, ..registry.clone() };
    let input = Scy20Datum { token_id, owner: alice, amount: 100_000 };
    let output = Scy20Datum { token_id, owner: bob, amount: 100_000 };
    let ctx = TxContext {
        tx_hash,
        block_time: 1_750_000_000,
        input_amount: 1_000_000,
        fee_burned: 0,
        input_datums: vec![encoded(&input), encoded(&registry)],
        output_datums: vec![encoded(&output), encoded(&next_registry)],
        ledger_state: vec![encoded(&registry)],
    };
    let datum = serialize_datum(&input).expect("datum must encode");
    let valid = Scy20Redeemer::Transfer {
        signature: alice_key.sign(&tx_hash).to_bytes(),
        nonce: 4,
    };
    let valid_redeemer = serialize_redeemer(&valid).expect("redeemer must encode");
    let result = ScyVM::execute_validator(&wasm, &datum, &valid_redeemer, &ctx, 1_000_000)
        .expect("WASM validation must execute");
    assert!(result.is_valid);

    let replay = Scy20Redeemer::Transfer {
        signature: alice_key.sign(&tx_hash).to_bytes(),
        nonce: 3,
    };
    let replay_redeemer = serialize_redeemer(&replay).expect("redeemer must encode");
    let result = ScyVM::execute_validator(&wasm, &datum, &replay_redeemer, &ctx, 1_000_000)
        .expect("WASM validation must execute");
    assert!(!result.is_valid);
}
