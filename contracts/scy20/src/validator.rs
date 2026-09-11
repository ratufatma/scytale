use alloc::vec::Vec;
use crate::error::Scy20Error;
use crate::types::{Scy20Datum, Scy20Redeemer, TokenRegistryDatum};
use scytale_sdk::{verify_ed25519, TxContext};

/// Entrypoint validasi eksekusi kontrak Scy-20.
pub fn validate_scy20_execution(
    datum: &Scy20Datum,
    redeemer: &Scy20Redeemer,
    ctx: &TxContext,
) -> Result<(), Scy20Error> {
    let registry = ctx
        .ledger_state
        .iter()
        .filter_map(|bytes| bincode::deserialize::<TokenRegistryDatum>(bytes).ok())
        .find(|state| state.token_id == datum.token_id)
        .ok_or(Scy20Error::InvalidRegistryState)?;
    let next_registry = ctx
        .output_datums
        .iter()
        .filter_map(|bytes| bincode::deserialize::<TokenRegistryDatum>(bytes).ok())
        .find(|state| state.token_id == datum.token_id)
        .ok_or(Scy20Error::InvalidRegistryState)?;

    let nonce = match redeemer {
        Scy20Redeemer::Transfer { signature, nonce } => {
            if !verify_ed25519(&datum.owner, signature, &ctx.tx_hash) {
                return Err(Scy20Error::MissingSignature(datum.owner));
            }
            *nonce
        }
        Scy20Redeemer::Mint { signature, nonce } => {
            if !verify_ed25519(&registry.authority, signature, &ctx.tx_hash) {
                return Err(Scy20Error::MissingSignature(registry.authority));
            }
            *nonce
        }
        Scy20Redeemer::Burn { signature, nonce } => {
            if !verify_ed25519(&datum.owner, signature, &ctx.tx_hash) {
                return Err(Scy20Error::MissingSignature(datum.owner));
            }
            *nonce
        }
    };
    let expected_nonce = registry
        .nonce
        .checked_add(1)
        .ok_or(Scy20Error::NonceMismatch)?;
    if nonce != registry.nonce || next_registry.nonce != expected_nonce {
        return Err(Scy20Error::NonceMismatch);
    }

    let input_total = canonical_token_total(&ctx.input_datums, datum.token_id)?;
    let output_total = canonical_token_total(&ctx.output_datums, datum.token_id)?;
    let delta = match redeemer {
        Scy20Redeemer::Mint { .. } => output_total.checked_sub(input_total),
        Scy20Redeemer::Transfer { .. } => (input_total == output_total).then_some(0),
        Scy20Redeemer::Burn { .. } => input_total.checked_sub(output_total),
    }
    .ok_or(Scy20Error::SupplyMismatch {
        input: input_total,
        output: output_total,
    })?;

    let expected_supply = match redeemer {
        Scy20Redeemer::Mint { .. } => registry
            .current_supply
            .checked_add(delta)
            .ok_or(Scy20Error::MaxSupplyExceeded)?,
        Scy20Redeemer::Transfer { .. } => registry.current_supply,
        Scy20Redeemer::Burn { .. } => registry
            .current_supply
            .checked_sub(delta)
            .ok_or(Scy20Error::SupplyMismatch {
                input: registry.current_supply,
                output: delta,
            })?,
    };
    if next_registry.current_supply != expected_supply
        || next_registry.authority != registry.authority
        || next_registry.max_supply != registry.max_supply
    {
        return Err(Scy20Error::InvalidRegistryState);
    }
    if next_registry.max_supply.is_some_and(|max| expected_supply > max) {
        return Err(Scy20Error::MaxSupplyExceeded);
    }
    Ok(())
}

fn canonical_token_total(
    datums: &[Vec<u8>],
    token_id: [u8; 32],
) -> Result<u128, Scy20Error> {
    let mut total = 0u128;
    for bytes in datums {
        if let Ok(state) = bincode::deserialize::<Scy20Datum>(bytes) {
            if state.token_id == token_id {
                if state.amount == 0 {
                    return Err(Scy20Error::ZeroAmount);
                }
                total = total.checked_add(state.amount).ok_or(Scy20Error::SupplyMismatch {
                    input: total,
                    output: state.amount,
                })?;
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Address, TokenId};
    use alloc::vec;
    use ed25519_dalek::{Signer, SigningKey};

    const TOKEN_ID: TokenId = [1; 32];

    fn make_keypair(seed: u8) -> (SigningKey, Address) {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let addr = key.verifying_key().to_bytes();
        (key, addr)
    }

    fn encode<T: serde::Serialize>(value: &T) -> Vec<u8> {
        bincode::serialize(value).expect("fixture should encode")
    }

    fn context(
        hash: [u8; 32],
        input: Vec<Scy20Datum>,
        output: Vec<Scy20Datum>,
        registry: TokenRegistryDatum,
        next_registry: TokenRegistryDatum,
    ) -> TxContext {
        TxContext {
            tx_hash: hash,
            block_time: 1_700_000_000,
            input_amount: 10_000,
            fee_burned: 0,
            input_datums: input.into_iter().map(|d| encode(&d)).chain([encode(&registry)]).collect(),
            output_datums: output.into_iter().map(|d| encode(&d)).chain([encode(&next_registry)]).collect(),
            ledger_state: vec![encode(&registry)],
        }
    }

    #[test]
    fn transfer_uses_host_outputs_and_registry_nonce() {
        let (alice_key, alice) = make_keypair(1);
        let (_, bob) = make_keypair(2);
        let hash = [0x42; 32];
        let registry = TokenRegistryDatum { token_id: TOKEN_ID, authority: alice, current_supply: 100, max_supply: Some(100), nonce: 7 };
        let next = TokenRegistryDatum { nonce: 8, ..registry.clone() };
        let input = Scy20Datum { token_id: TOKEN_ID, owner: alice, amount: 100 };
        let output = Scy20Datum { token_id: TOKEN_ID, owner: bob, amount: 100 };
        let ctx = context(hash, vec![input.clone()], vec![output], registry, next);
        let redeemer = Scy20Redeemer::Transfer { signature: alice_key.sign(&hash).to_bytes(), nonce: 7 };
        assert_eq!(validate_scy20_execution(&input, &redeemer, &ctx), Ok(()));
    }

    #[test]
    fn tampered_redeemer_cannot_change_supply_or_outputs() {
        let (alice_key, alice) = make_keypair(1);
        let hash = [0x43; 32];
        let registry = TokenRegistryDatum { token_id: TOKEN_ID, authority: alice, current_supply: 100, max_supply: Some(100), nonce: 7 };
        let next = TokenRegistryDatum { nonce: 8, ..registry.clone() };
        let input = Scy20Datum { token_id: TOKEN_ID, owner: alice, amount: 100 };
        let output = Scy20Datum { token_id: TOKEN_ID, owner: alice, amount: 101 };
        let ctx = context(hash, vec![input.clone()], vec![output], registry, next);
        let redeemer = Scy20Redeemer::Transfer { signature: alice_key.sign(&hash).to_bytes(), nonce: 7 };
        assert!(matches!(validate_scy20_execution(&input, &redeemer, &ctx), Err(Scy20Error::SupplyMismatch { .. })));
    }

    #[test]
    fn replayed_nonce_is_rejected() {
        let (alice_key, alice) = make_keypair(1);
        let hash = [0x44; 32];
        let registry = TokenRegistryDatum { token_id: TOKEN_ID, authority: alice, current_supply: 100, max_supply: None, nonce: 8 };
        let next = TokenRegistryDatum { nonce: 9, ..registry.clone() };
        let input = Scy20Datum { token_id: TOKEN_ID, owner: alice, amount: 100 };
        let ctx = context(hash, vec![input.clone()], vec![input.clone()], registry, next);
        let redeemer = Scy20Redeemer::Transfer { signature: alice_key.sign(&hash).to_bytes(), nonce: 7 };
        assert_eq!(validate_scy20_execution(&input, &redeemer, &ctx), Err(Scy20Error::NonceMismatch));
    }
}
