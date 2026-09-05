use crate::error::Scy20Error;
use crate::types::{Scy20Datum, Scy20Redeemer, TokenId, TokenMetadata};
use scytale_sdk::{verify_ed25519, TxContext};

/// Validasi state transition eUTXO untuk transfer token Scy-20.
pub fn validate_transfer(
    datum: &Scy20Datum,
    signature: &[u8; 64],
    outputs: &[Scy20Datum],
    fee: u128,
    tx_hash: &[u8; 32],
) -> Result<(), Scy20Error> {
    if datum.amount == 0 {
        return Err(Scy20Error::ZeroAmount);
    }

    // 1. Verifikasi tanda tangan kriptografis Ed25519 pemilik UTXO atas tx_hash
    if !verify_ed25519(&datum.owner, signature, tx_hash) {
        return Err(Scy20Error::MissingSignature(datum.owner));
    }

    if outputs.is_empty() {
        return Err(Scy20Error::ZeroAmount);
    }

    // 2. Verifikasi keseragaman token ID dan non-zero amount pada output
    for output in outputs {
        if output.token_id != datum.token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
        if output.amount == 0 {
            return Err(Scy20Error::ZeroAmount);
        }
    }

    // 3. Verifikasi hukum konservasi nilai: Input == Total Output + Fee
    let total_out: u128 = outputs.iter().map(|d| d.amount).sum();
    let expected_in = total_out
        .checked_add(fee)
        .ok_or(Scy20Error::SupplyMismatch {
            input: datum.amount,
            output: total_out,
        })?;

    if datum.amount != expected_in {
        return Err(Scy20Error::SupplyMismatch {
            input: datum.amount,
            output: expected_in,
        });
    }

    Ok(())
}

/// Validasi pembuatan token Scy-20.
#[allow(clippy::too_many_arguments)]
pub fn validate_mint(
    token_id: &TokenId,
    minter_authority: &[u8; 32],
    signature: &[u8; 64],
    outputs: &[Scy20Datum],
    mint_amount: u128,
    current_supply: u128,
    metadata: Option<&TokenMetadata>,
    tx_hash: &[u8; 32],
) -> Result<(), Scy20Error> {
    if mint_amount == 0 {
        return Err(Scy20Error::ZeroAmount);
    }

    // 1. Verifikasi tanda tangan minter resmi
    if !verify_ed25519(minter_authority, signature, tx_hash) {
        return Err(Scy20Error::MissingSignature(*minter_authority));
    }

    if outputs.is_empty() {
        return Err(Scy20Error::ZeroAmount);
    }

    for output in outputs {
        if output.token_id != *token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
        if output.amount == 0 {
            return Err(Scy20Error::ZeroAmount);
        }
    }

    let total_out: u128 = outputs.iter().map(|datum| datum.amount).sum();
    if total_out != mint_amount {
        return Err(Scy20Error::SupplyMismatch {
            input: mint_amount,
            output: total_out,
        });
    }

    if let Some(meta) = metadata {
        if let Some(max_supply) = meta.max_supply {
            let resulting_supply = current_supply
                .checked_add(mint_amount)
                .ok_or(Scy20Error::MaxSupplyExceeded)?;
            if resulting_supply > max_supply {
                return Err(Scy20Error::MaxSupplyExceeded);
            }
        }
    }

    Ok(())
}

/// Validasi pembakaran token Scy-20.
pub fn validate_burn(
    datum: &Scy20Datum,
    signature: &[u8; 64],
    burn_amount: u128,
    outputs: &[Scy20Datum],
    tx_hash: &[u8; 32],
) -> Result<(), Scy20Error> {
    if burn_amount == 0 || burn_amount > datum.amount {
        return Err(Scy20Error::ZeroAmount);
    }

    // 1. Verifikasi tanda tangan pemilik UTXO yang dibakar
    if !verify_ed25519(&datum.owner, signature, tx_hash) {
        return Err(Scy20Error::MissingSignature(datum.owner));
    }

    for output in outputs {
        if output.token_id != datum.token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
        if output.amount == 0 {
            return Err(Scy20Error::ZeroAmount);
        }
    }

    let total_out: u128 = outputs.iter().map(|datum| datum.amount).sum();
    let expected_in = total_out
        .checked_add(burn_amount)
        .ok_or(Scy20Error::SupplyMismatch {
            input: datum.amount,
            output: total_out,
        })?;

    if datum.amount != expected_in {
        return Err(Scy20Error::SupplyMismatch {
            input: datum.amount,
            output: expected_in,
        });
    }

    Ok(())
}

/// Entrypoint validasi eksekusi kontrak Scy-20.
pub fn validate_scy20_execution(
    datum: &Scy20Datum,
    redeemer: &Scy20Redeemer,
    ctx: &TxContext,
) -> Result<(), Scy20Error> {
    match redeemer {
        Scy20Redeemer::Transfer {
            signature,
            outputs,
            fee,
        } => validate_transfer(datum, signature, outputs, *fee, &ctx.tx_hash),
        Scy20Redeemer::Mint {
            amount,
            signature,
            outputs,
            metadata,
            current_supply,
        } => validate_mint(
            &datum.token_id,
            &datum.owner,
            signature,
            outputs,
            *amount,
            *current_supply,
            metadata.as_ref(),
            &ctx.tx_hash,
        ),
        Scy20Redeemer::Burn {
            amount,
            signature,
            outputs,
        } => validate_burn(datum, signature, *amount, outputs, &ctx.tx_hash),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};
    use crate::types::Address;
    use ed25519_dalek::{Signer, SigningKey};

    const TOKEN_ID: TokenId = [1; 32];
    const OTHER_TOKEN_ID: TokenId = [2; 32];

    fn test_ctx(tx_hash: [u8; 32]) -> TxContext {
        TxContext {
            tx_hash,
            block_time: 1_700_000_000,
            input_amount: 10_000,
            fee_burned: 100,
        }
    }

    fn make_keypair(seed: u8) -> (SigningKey, Address) {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let addr = key.verifying_key().to_bytes();
        (key, addr)
    }

    #[test]
    fn test_transfer_success() {
        let (alice_key, alice_addr) = make_keypair(1);
        let (_, bob_addr) = make_keypair(2);

        let ctx = test_ctx([0x42; 32]);
        let datum = Scy20Datum {
            token_id: TOKEN_ID,
            owner: alice_addr,
            amount: 100,
        };

        let outputs = vec![
            Scy20Datum {
                token_id: TOKEN_ID,
                owner: bob_addr,
                amount: 70,
            },
            Scy20Datum {
                token_id: TOKEN_ID,
                owner: alice_addr,
                amount: 30,
            },
        ];

        let signature = alice_key.sign(&ctx.tx_hash).to_bytes();
        let redeemer = Scy20Redeemer::Transfer {
            signature,
            outputs,
            fee: 0,
        };

        assert_eq!(validate_scy20_execution(&datum, &redeemer, &ctx), Ok(()));
    }

    #[test]
    fn test_transfer_supply_mismatch() {
        let (alice_key, alice_addr) = make_keypair(1);
        let (_, bob_addr) = make_keypair(2);

        let ctx = test_ctx([0x42; 32]);
        let datum = Scy20Datum {
            token_id: TOKEN_ID,
            owner: alice_addr,
            amount: 100,
        };

        let outputs = vec![Scy20Datum {
            token_id: TOKEN_ID,
            owner: bob_addr,
            amount: 110, // Trying to duplicate balance
        }];

        let signature = alice_key.sign(&ctx.tx_hash).to_bytes();
        let redeemer = Scy20Redeemer::Transfer {
            signature,
            outputs,
            fee: 0,
        };

        assert_eq!(
            validate_scy20_execution(&datum, &redeemer, &ctx),
            Err(Scy20Error::SupplyMismatch {
                input: 100,
                output: 110,
            })
        );
    }

    #[test]
    fn test_transfer_unauthorized() {
        let (_alice_key, alice_addr) = make_keypair(1);
        let (mallory_key, _mallory_addr) = make_keypair(99);

        let ctx = test_ctx([0x42; 32]);
        let datum = Scy20Datum {
            token_id: TOKEN_ID,
            owner: alice_addr,
            amount: 100,
        };

        let outputs = vec![Scy20Datum {
            token_id: TOKEN_ID,
            owner: alice_addr,
            amount: 100,
        }];

        // Mallory signs instead of Alice
        let signature = mallory_key.sign(&ctx.tx_hash).to_bytes();
        let redeemer = Scy20Redeemer::Transfer {
            signature,
            outputs,
            fee: 0,
        };

        assert_eq!(
            validate_scy20_execution(&datum, &redeemer, &ctx),
            Err(Scy20Error::MissingSignature(alice_addr))
        );
    }

    #[test]
    fn test_transfer_mixed_token_id() {
        let (alice_key, alice_addr) = make_keypair(1);

        let ctx = test_ctx([0x42; 32]);
        let datum = Scy20Datum {
            token_id: TOKEN_ID,
            owner: alice_addr,
            amount: 100,
        };

        let outputs = vec![Scy20Datum {
            token_id: OTHER_TOKEN_ID,
            owner: alice_addr,
            amount: 100,
        }];

        let signature = alice_key.sign(&ctx.tx_hash).to_bytes();
        let redeemer = Scy20Redeemer::Transfer {
            signature,
            outputs,
            fee: 0,
        };

        assert_eq!(
            validate_scy20_execution(&datum, &redeemer, &ctx),
            Err(Scy20Error::InvalidTokenId)
        );
    }

    #[test]
    fn test_mint_max_supply_cap() {
        let (minter_key, minter_addr) = make_keypair(1);

        let ctx = test_ctx([0x42; 32]);
        let datum = Scy20Datum {
            token_id: TOKEN_ID,
            owner: minter_addr,
            amount: 0, // Minter anchor UTXO
        };

        let metadata = TokenMetadata {
            name: "Test Token".to_string(),
            symbol: "TST".to_string(),
            decimals: 6,
            max_supply: Some(100),
        };

        let outputs = vec![Scy20Datum {
            token_id: TOKEN_ID,
            owner: minter_addr,
            amount: 51,
        }];

        let signature = minter_key.sign(&ctx.tx_hash).to_bytes();
        let redeemer = Scy20Redeemer::Mint {
            amount: 51,
            signature,
            outputs,
            metadata: Some(metadata),
            current_supply: 50, // 50 + 51 = 101 > 100 max
        };

        assert_eq!(
            validate_scy20_execution(&datum, &redeemer, &ctx),
            Err(Scy20Error::MaxSupplyExceeded)
        );
    }

    #[test]
    fn test_burn_success() {
        let (alice_key, alice_addr) = make_keypair(1);

        let ctx = test_ctx([0x42; 32]);
        let datum = Scy20Datum {
            token_id: TOKEN_ID,
            owner: alice_addr,
            amount: 100,
        };

        let outputs = vec![Scy20Datum {
            token_id: TOKEN_ID,
            owner: alice_addr,
            amount: 60,
        }];

        let signature = alice_key.sign(&ctx.tx_hash).to_bytes();
        let redeemer = Scy20Redeemer::Burn {
            amount: 40,
            signature,
            outputs,
        };

        assert_eq!(validate_scy20_execution(&datum, &redeemer, &ctx), Ok(()));
    }
}
