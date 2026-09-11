use ed25519_dalek::{Signer, SigningKey};
use scy20::{
    validate_scy20_execution, Scy20Datum, Scy20Redeemer, TokenRegistryDatum, TokenId,
    TxContext,
};

fn key(seed: u8) -> (SigningKey, [u8; 32]) {
    let signing_key = SigningKey::from_bytes(&[seed; 32]);
    let address = signing_key.verifying_key().to_bytes();
    (signing_key, address)
}

fn encoded<T: serde::Serialize>(value: &T) -> Vec<u8> {
    bincode::serialize(value).expect("canonical fixture must encode")
}

fn context(
    hash: [u8; 32],
    input_tokens: &[Scy20Datum],
    output_tokens: &[Scy20Datum],
    registry: &TokenRegistryDatum,
    next_registry: &TokenRegistryDatum,
) -> TxContext {
    TxContext {
        tx_hash: hash,
        block_time: 1_750_000_000,
        input_amount: 1_000_000,
        fee_burned: 0,
        input_datums: input_tokens.iter().map(encoded).chain([encoded(registry)]).collect(),
        output_datums: output_tokens.iter().map(encoded).chain([encoded(next_registry)]).collect(),
        ledger_state: vec![encoded(registry)],
    }
}

#[test]
fn canonical_transfer_lifecycle_uses_registry_and_host_outputs() {
    let token_id: TokenId = [7; 32];
    let (alice_key, alice) = key(1);
    let (_, bob) = key(2);
    let hash = [9; 32];
    let registry = TokenRegistryDatum {
        token_id,
        authority: alice,
        current_supply: 100_000,
        max_supply: Some(100_000),
        nonce: 0,
    };
    let next_registry = TokenRegistryDatum { nonce: 1, ..registry.clone() };
    let input = Scy20Datum { token_id, owner: alice, amount: 100_000 };
    let output = Scy20Datum { token_id, owner: bob, amount: 100_000 };
    let ctx = context(hash, &[input.clone()], &[output], &registry, &next_registry);
    let redeemer = Scy20Redeemer::Transfer {
        signature: alice_key.sign(&hash).to_bytes(),
        nonce: 0,
    };

    assert_eq!(validate_scy20_execution(&input, &redeemer, &ctx), Ok(()));
}

#[test]
fn canonical_lifecycle_rejects_replay_and_output_inflation() {
    let token_id: TokenId = [8; 32];
    let (alice_key, alice) = key(3);
    let hash = [10; 32];
    let registry = TokenRegistryDatum {
        token_id,
        authority: alice,
        current_supply: 100,
        max_supply: Some(100),
        nonce: 2,
    };
    let next_registry = TokenRegistryDatum { nonce: 3, ..registry.clone() };
    let input = Scy20Datum { token_id, owner: alice, amount: 100 };
    let inflated = Scy20Datum { token_id, owner: alice, amount: 101 };
    let ctx = context(hash, &[input.clone()], &[inflated], &registry, &next_registry);

    let replay = Scy20Redeemer::Transfer {
        signature: alice_key.sign(&hash).to_bytes(),
        nonce: 1,
    };
    assert!(validate_scy20_execution(&input, &replay, &ctx).is_err());

    let current_nonce = Scy20Redeemer::Transfer {
        signature: alice_key.sign(&hash).to_bytes(),
        nonce: 2,
    };
    assert!(validate_scy20_execution(&input, &current_nonce, &ctx).is_err());
}
