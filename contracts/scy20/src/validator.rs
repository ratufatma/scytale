use crate::error::Scy20Error;
use crate::types::{Address, Scy20Datum};

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
