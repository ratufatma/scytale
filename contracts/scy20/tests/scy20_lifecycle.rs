use ed25519_dalek::{Signer, SigningKey};
use scy20::codec::{deserialize_datum, deserialize_redeemer, serialize_datum, serialize_redeemer};
use scy20::{
    validate_scy20_execution, Address, Scy20Datum, Scy20Error, Scy20Redeemer,
    TokenId, TokenMetadata, TxContext,
};

fn make_keypair(seed: u8) -> (SigningKey, Address) {
    let key = SigningKey::from_bytes(&[seed; 32]);
    let addr = key.verifying_key().to_bytes();
    (key, addr)
}

fn make_ctx(seed: u8) -> TxContext {
    TxContext {
        tx_hash: [seed; 32],
        block_time: 1_700_000_000,
        input_amount: 10_000,
        fee_burned: 100,
    }
}

#[test]
fn test_scy20_complete_eutxo_lifecycle() {
    let token_id: TokenId = [1u8; 32];
    let (issuer_key, issuer) = make_keypair(0xAA);
    let (alice_key, alice) = make_keypair(0xBB);
    let (_bob_key, bob) = make_keypair(0xCC);
    let (mallory_key, _mallory) = make_keypair(0xEE);

    let metadata = TokenMetadata {
        name: "Scytale USD".to_string(),
        symbol: "sUSD".to_string(),
        decimals: 6,
        max_supply: Some(1_000_000),
    };

    // 1. Genesis minting: Issuer anchor datum issues 100,000 to Alice
    let issuer_anchor_datum = Scy20Datum {
        token_id,
        owner: issuer,
        amount: 0,
    };
    let alice_genesis_utxo = Scy20Datum {
        token_id,
        owner: alice,
        amount: 100_000,
    };

    let mint_ctx = make_ctx(1);
    let mint_sig = issuer_key.sign(&mint_ctx.tx_hash).to_bytes();
    let mint_redeemer = Scy20Redeemer::Mint {
        amount: 100_000,
        signature: mint_sig,
        outputs: vec![alice_genesis_utxo.clone()],
        metadata: Some(metadata.clone()),
        current_supply: 0,
    };

    assert_eq!(
        validate_scy20_execution(&issuer_anchor_datum, &mint_redeemer, &mint_ctx),
        Ok(())
    );
    let encoded_mint_datum = serialize_datum(&alice_genesis_utxo).expect("datum should serialize");
    assert_eq!(
        deserialize_datum(&encoded_mint_datum).expect("datum should deserialize"),
        alice_genesis_utxo
    );
    let encoded_mint_redeemer =
        serialize_redeemer(&mint_redeemer).expect("redeemer should serialize");
    assert_eq!(
        deserialize_redeemer(&encoded_mint_redeemer).expect("redeemer should deserialize"),
        mint_redeemer
    );

    // 2. Transfer: Alice sends 40,000 to Bob and returns 60,000 change to herself
    let bob_transfer_output = Scy20Datum {
        token_id,
        owner: bob,
        amount: 40_000,
    };
    let alice_change_output = Scy20Datum {
        token_id,
        owner: alice,
        amount: 60_000,
    };

    let transfer_ctx = make_ctx(2);
    let alice_sig = alice_key.sign(&transfer_ctx.tx_hash).to_bytes();
    let transfer_redeemer = Scy20Redeemer::Transfer {
        signature: alice_sig,
        outputs: vec![bob_transfer_output.clone(), alice_change_output.clone()],
        fee: 0,
    };

    assert_eq!(
        validate_scy20_execution(&alice_genesis_utxo, &transfer_redeemer, &transfer_ctx),
        Ok(())
    );
    for datum in [&bob_transfer_output, &alice_change_output] {
        let encoded_datum = serialize_datum(datum).expect("datum should serialize");
        assert_eq!(
            deserialize_datum(&encoded_datum).expect("datum should deserialize"),
            *datum
        );
    }

    // 3. Partial burn: Alice burns 15,000 from change output (60,000) and keeps 45,000
    let alice_burn_output = Scy20Datum {
        token_id,
        owner: alice,
        amount: 45_000,
    };
    let burn_ctx = make_ctx(3);
    let burn_sig = alice_key.sign(&burn_ctx.tx_hash).to_bytes();
    let burn_redeemer = Scy20Redeemer::Burn {
        amount: 15_000,
        signature: burn_sig,
        outputs: vec![alice_burn_output],
    };

    assert_eq!(
        validate_scy20_execution(&alice_change_output, &burn_redeemer, &burn_ctx),
        Ok(())
    );

    // 4. Security checks: unauthorized spend, inflation, and cap violation
    // 4a. Unauthorized transfer: Mallory tries to spend Bob's tokens
    let unauth_ctx = make_ctx(4);
    let mallory_sig = mallory_key.sign(&unauth_ctx.tx_hash).to_bytes();
    let unauthorized_transfer = Scy20Redeemer::Transfer {
        signature: mallory_sig,
        outputs: vec![Scy20Datum {
            token_id,
            owner: mallory_key.verifying_key().to_bytes(),
            amount: 40_000,
        }],
        fee: 0,
    };
    assert_eq!(
        validate_scy20_execution(&bob_transfer_output, &unauthorized_transfer, &unauth_ctx),
        Err(Scy20Error::MissingSignature(bob))
    );

    // 4b. Inflation transfer: Trying to output 70,000 from 60,000 input
    let inflation_ctx = make_ctx(5);
    let alice_sig_inf = alice_key.sign(&inflation_ctx.tx_hash).to_bytes();
    let inflation_transfer = Scy20Redeemer::Transfer {
        signature: alice_sig_inf,
        outputs: vec![Scy20Datum {
            token_id,
            owner: bob,
            amount: 70_000,
        }],
        fee: 0,
    };
    assert_eq!(
        validate_scy20_execution(&alice_change_output, &inflation_transfer, &inflation_ctx),
        Err(Scy20Error::SupplyMismatch {
            input: 60_000,
            output: 70_000,
        })
    );

    // 4c. Cap violation: Minting 1,500,000 exceeds max_supply 1,000,000
    let cap_ctx = make_ctx(6);
    let cap_sig = issuer_key.sign(&cap_ctx.tx_hash).to_bytes();
    let cap_violation_redeemer = Scy20Redeemer::Mint {
        amount: 1_500_000,
        signature: cap_sig,
        outputs: vec![Scy20Datum {
            token_id,
            owner: alice,
            amount: 1_500_000,
        }],
        metadata: Some(metadata),
        current_supply: 0,
    };
    assert_eq!(
        validate_scy20_execution(&issuer_anchor_datum, &cap_violation_redeemer, &cap_ctx),
        Err(Scy20Error::MaxSupplyExceeded)
    );
}
