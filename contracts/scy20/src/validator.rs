use crate::error::Scy20Error;
use crate::types::{Address, Scy20Datum, Scy20Redeemer, ScriptContext, TokenId, TokenMetadata};

/// Validasi state transition eUTXO untuk transfer token Scy-20
pub fn validate_transfer(
    inputs: &[Scy20Datum],
    outputs: &[Scy20Datum],
    signers: &[Address],
) -> Result<(), Scy20Error> {
    if inputs.is_empty() {
        return Err(Scy20Error::ZeroAmount);
    }

    let token_id = inputs[0].token_id;

    // 1. Verifikasi tanda tangan pemilik input
    for input in inputs {
        if input.token_id != token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
        if !signers.contains(&input.owner) {
            return Err(Scy20Error::MissingSignature(input.owner));
        }
    }

    // 2. Verifikasi keseragaman token ID pada output
    for output in outputs {
        if output.token_id != token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
    }

    // 3. Verifikasi hukum konservasi nilai (Total Input == Total Output)
    let total_in: u128 = inputs.iter().map(|d| d.amount).sum();
    let total_out: u128 = outputs.iter().map(|d| d.amount).sum();

    if total_in != total_out {
        return Err(Scy20Error::SupplyMismatch {
            input: total_in,
            output: total_out,
        });
    }

    Ok(())
}

/// Validasi pembuatan token Scy-20.
pub fn validate_mint(
    token_id: &TokenId,
    outputs: &[Scy20Datum],
    mint_amount: u128,
    current_supply: u128,
    metadata: &TokenMetadata,
    policy_signers: &[Address],
) -> Result<(), Scy20Error> {
    for output in outputs {
        if output.token_id != *token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
    }

    let total_out: u128 = outputs.iter().map(|datum| datum.amount).sum();
    if total_out != mint_amount {
        return Err(Scy20Error::SupplyMismatch {
            input: mint_amount,
            output: total_out,
        });
    }

    if let Some(max_supply) = metadata.max_supply {
        let resulting_supply = current_supply
            .checked_add(mint_amount)
            .ok_or(Scy20Error::MaxSupplyExceeded)?;
        if resulting_supply > max_supply {
            return Err(Scy20Error::MaxSupplyExceeded);
        }
    }

    if policy_signers.is_empty() {
        return Err(Scy20Error::MissingSignature([0; 32]));
    }

    Ok(())
}

/// Validasi pembakaran token Scy-20.
pub fn validate_burn(
    token_id: &TokenId,
    inputs: &[Scy20Datum],
    outputs: &[Scy20Datum],
    burn_amount: u128,
    signers: &[Address],
) -> Result<(), Scy20Error> {
    for input in inputs {
        if input.token_id != *token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
        if !signers.contains(&input.owner) {
            return Err(Scy20Error::MissingSignature(input.owner));
        }
    }

    for output in outputs {
        if output.token_id != *token_id {
            return Err(Scy20Error::InvalidTokenId);
        }
    }

    let total_in: u128 = inputs.iter().map(|datum| datum.amount).sum();
    let total_out: u128 = outputs.iter().map(|datum| datum.amount).sum();
    let expected_in = total_out.saturating_add(burn_amount);

    if total_in != expected_in {
        return Err(Scy20Error::SupplyMismatch {
            input: total_in,
            output: expected_in,
        });
    }

    Ok(())
}

/// Entrypoint validasi eksekusi kontrak Scy-20.
pub fn validate_scy20_execution(
    context: &ScriptContext,
    redeemer: &Scy20Redeemer,
) -> Result<(), Scy20Error> {
    match redeemer {
        Scy20Redeemer::Transfer => {
            validate_transfer(&context.inputs, &context.outputs, &context.signers)
        }
        Scy20Redeemer::Mint { amount } => {
            let metadata = context
                .metadata
                .as_ref()
                .ok_or(Scy20Error::DeserializationFailed)?;
            validate_mint(
                &context.token_id,
                &context.outputs,
                *amount,
                context.current_supply,
                metadata,
                &context.signers,
            )
        }
        Scy20Redeemer::Burn { amount } => validate_burn(
            &context.token_id,
            &context.inputs,
            &context.outputs,
            *amount,
            &context.signers,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN_ID: TokenId = [1; 32];
    const OTHER_TOKEN_ID: TokenId = [2; 32];
    const OWNER: Address = [3; 32];

    fn datum(token_id: TokenId, owner: Address, amount: u128) -> Scy20Datum {
        Scy20Datum {
            token_id,
            owner,
            amount,
        }
    }

    fn metadata(max_supply: Option<u128>) -> TokenMetadata {
        TokenMetadata {
            name: "Test Token".to_owned(),
            symbol: "TST".to_owned(),
            decimals: 0,
            max_supply,
        }
    }

    fn context(
        inputs: Vec<Scy20Datum>,
        outputs: Vec<Scy20Datum>,
        signers: Vec<Address>,
        metadata: Option<TokenMetadata>,
    ) -> ScriptContext {
        ScriptContext {
            token_id: TOKEN_ID,
            inputs,
            outputs,
            signers,
            current_supply: 0,
            metadata,
        }
    }

    #[test]
    fn test_transfer_success() {
        let inputs = [datum(TOKEN_ID, OWNER, 100)];
        let outputs = [datum(TOKEN_ID, [4; 32], 70), datum(TOKEN_ID, OWNER, 30)];

        assert_eq!(validate_transfer(&inputs, &outputs, &[OWNER]), Ok(()));
    }

    #[test]
    fn test_transfer_supply_mismatch() {
        let inputs = [datum(TOKEN_ID, OWNER, 100)];
        let outputs = [datum(TOKEN_ID, [4; 32], 110)];

        assert_eq!(
            validate_transfer(&inputs, &outputs, &[OWNER]),
            Err(Scy20Error::SupplyMismatch {
                input: 100,
                output: 110,
            })
        );
    }

    #[test]
    fn test_transfer_unauthorized() {
        let inputs = [datum(TOKEN_ID, OWNER, 100)];
        let outputs = [datum(TOKEN_ID, [4; 32], 100)];

        assert_eq!(
            validate_transfer(&inputs, &outputs, &[]),
            Err(Scy20Error::MissingSignature(OWNER))
        );
    }

    #[test]
    fn test_transfer_mixed_token_id() {
        let inputs = [datum(TOKEN_ID, OWNER, 100), datum(OTHER_TOKEN_ID, OWNER, 10)];
        let outputs = [datum(TOKEN_ID, [4; 32], 110)];

        assert_eq!(
            validate_transfer(&inputs, &outputs, &[OWNER]),
            Err(Scy20Error::InvalidTokenId)
        );
    }

    #[test]
    fn test_mint_max_supply_cap() {
        let outputs = [datum(TOKEN_ID, OWNER, 51)];

        assert_eq!(
            validate_mint(
                &TOKEN_ID,
                &outputs,
                51,
                50,
                &metadata(Some(100)),
                &[OWNER],
            ),
            Err(Scy20Error::MaxSupplyExceeded)
        );
    }

    #[test]
    fn test_burn_success() {
        let inputs = [datum(TOKEN_ID, OWNER, 100)];
        let outputs = [datum(TOKEN_ID, [4; 32], 60)];

        assert_eq!(
            validate_burn(&TOKEN_ID, &inputs, &outputs, 40, &[OWNER]),
            Ok(())
        );
    }

    #[test]
    fn test_dispatcher_transfer_route() {
        let execution_context = context(
            vec![datum(TOKEN_ID, OWNER, 100)],
            vec![datum(TOKEN_ID, [4; 32], 70), datum(TOKEN_ID, OWNER, 30)],
            vec![OWNER],
            None,
        );

        assert_eq!(
            validate_scy20_execution(&execution_context, &Scy20Redeemer::Transfer),
            Ok(())
        );
    }

    #[test]
    fn test_dispatcher_mint_missing_metadata() {
        let execution_context = context(
            Vec::new(),
            vec![datum(TOKEN_ID, OWNER, 100)],
            vec![OWNER],
            None,
        );

        assert_eq!(
            validate_scy20_execution(
                &execution_context,
                &Scy20Redeemer::Mint { amount: 100 },
            ),
            Err(Scy20Error::DeserializationFailed)
        );
    }
}
