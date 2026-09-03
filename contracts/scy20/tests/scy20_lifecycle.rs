use scy20::codec::{deserialize_datum, deserialize_redeemer, serialize_datum, serialize_redeemer};
use scy20::{
    validate_scy20_execution, Address, Scy20Datum, Scy20Error, Scy20Redeemer, ScriptContext,
    TokenId, TokenMetadata,
};

#[test]
fn test_scy20_complete_eutxo_lifecycle() {
    let token_id: TokenId = [1u8; 32];
    let issuer: Address = [0xAA; 32];
    let alice: Address = [0xBB; 32];
    let bob: Address = [0xCC; 32];
    let mallory: Address = [0xEE; 32];
    let metadata = TokenMetadata {
        name: "Scytale USD".to_string(),
        symbol: "sUSD".to_string(),
        decimals: 6,
        max_supply: Some(1_000_000),
    };

    // Genesis minting: issue the initial supply to Alice.
    let alice_genesis_utxo = Scy20Datum {
        token_id,
        owner: alice,
        amount: 100_000,
    };
    let mint_redeemer = Scy20Redeemer::Mint { amount: 100_000 };
    let mint_context = ScriptContext {
        token_id,
        inputs: Vec::new(),
        outputs: vec![alice_genesis_utxo.clone()],
        signers: vec![issuer],
        current_supply: 0,
        metadata: Some(metadata.clone()),
    };

    assert_eq!(
        validate_scy20_execution(&mint_context, &mint_redeemer),
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

    // Transfer: send 40,000 to Bob and return 60,000 to Alice.
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
    let transfer_context = ScriptContext {
        token_id,
        inputs: vec![alice_genesis_utxo],
        outputs: vec![bob_transfer_output.clone(), alice_change_output.clone()],
        signers: vec![alice],
        current_supply: 100_000,
        metadata: Some(metadata.clone()),
    };

    assert_eq!(
        validate_scy20_execution(&transfer_context, &Scy20Redeemer::Transfer),
        Ok(())
    );
    for datum in [&bob_transfer_output, &alice_change_output] {
        let encoded_datum = serialize_datum(datum).expect("datum should serialize");
        assert_eq!(
            deserialize_datum(&encoded_datum).expect("datum should deserialize"),
            *datum
        );
    }

    // Partial burn: Bob burns 15,000 and keeps 25,000.
    let bob_burn_output = Scy20Datum {
        token_id,
        owner: bob,
        amount: 25_000,
    };
    let burn_context = ScriptContext {
        token_id,
        inputs: vec![bob_transfer_output.clone()],
        outputs: vec![bob_burn_output],
        signers: vec![bob],
        current_supply: 100_000,
        metadata: Some(metadata.clone()),
    };

    assert_eq!(
        validate_scy20_execution(
            &burn_context,
            &Scy20Redeemer::Burn { amount: 15_000 },
        ),
        Ok(())
    );

    // Security checks: unauthorized spend, inflation, and cap violation.
    let unauthorized_transfer = ScriptContext {
        token_id,
        inputs: vec![bob_transfer_output.clone()],
        outputs: vec![Scy20Datum {
            token_id,
            owner: mallory,
            amount: 40_000,
        }],
        signers: vec![mallory],
        current_supply: 100_000,
        metadata: Some(metadata.clone()),
    };
    assert_eq!(
        validate_scy20_execution(&unauthorized_transfer, &Scy20Redeemer::Transfer),
        Err(Scy20Error::MissingSignature(bob))
    );

    let inflation_transfer = ScriptContext {
        token_id,
        inputs: vec![alice_change_output],
        outputs: vec![Scy20Datum {
            token_id,
            owner: bob,
            amount: 70_000,
        }],
        signers: vec![alice],
        current_supply: 100_000,
        metadata: Some(metadata.clone()),
    };
    assert_eq!(
        validate_scy20_execution(&inflation_transfer, &Scy20Redeemer::Transfer),
        Err(Scy20Error::SupplyMismatch {
            input: 60_000,
            output: 70_000,
        })
    );

    let cap_violation = ScriptContext {
        token_id,
        inputs: Vec::new(),
        outputs: vec![Scy20Datum {
            token_id,
            owner: alice,
            amount: 1_500_000,
        }],
        signers: vec![issuer],
        current_supply: 0,
        metadata: Some(metadata),
    };
    assert_eq!(
        validate_scy20_execution(
            &cap_violation,
            &Scy20Redeemer::Mint { amount: 1_500_000 },
        ),
        Err(Scy20Error::MaxSupplyExceeded)
    );
}
