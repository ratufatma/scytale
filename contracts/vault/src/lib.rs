#![no_std]
extern crate alloc;

use scytale_sdk::{decode_payload, TxContext, VALIDATION_REJECT, VALIDATION_SUCCESS};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VaultDatum {
    pub owner_pubkey: [u8; 32],
    pub unlock_time: u64,
    pub emergency_key: [u8; 32],
    pub penalty_fee: u64,
}

#[derive(Serialize, Deserialize)]
pub enum VaultRedeemer {
    NormalWithdraw { sig_valid: bool },
    EmergencyRescue { penalty_accepted: bool },
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn validate(
    datum_ptr: *const u8,
    datum_len: usize,
    redeemer_ptr: *const u8,
    redeemer_len: usize,
    ctx_ptr: *const u8,
    ctx_len: usize,
) -> i32 {
    let datum_slice = unsafe { core::slice::from_raw_parts(datum_ptr, datum_len) };
    let redeemer_slice = unsafe { core::slice::from_raw_parts(redeemer_ptr, redeemer_len) };
    let ctx_slice = unsafe { core::slice::from_raw_parts(ctx_ptr, ctx_len) };

    let datum: VaultDatum = match decode_payload(datum_slice) {
        Ok(d) => d,
        Err(_) => return VALIDATION_REJECT,
    };

    let redeemer: VaultRedeemer = match decode_payload(redeemer_slice) {
        Ok(r) => r,
        Err(_) => return VALIDATION_REJECT,
    };

    let ctx: TxContext = match decode_payload(ctx_slice) {
        Ok(c) => c,
        Err(_) => return VALIDATION_REJECT,
    };

    match redeemer {
        VaultRedeemer::NormalWithdraw { sig_valid } => {
            // Wajib melewati batas waktu timelock & tanda tangan valid
            if ctx.block_time >= datum.unlock_time && sig_valid {
                VALIDATION_SUCCESS
            } else {
                VALIDATION_REJECT
            }
        }
        VaultRedeemer::EmergencyRescue { penalty_accepted } => {
            // Penyelamatan sebelum timelock wajib menyertakan pembakaran denda
            if penalty_accepted && ctx.fee_burned >= datum.penalty_fee {
                VALIDATION_SUCCESS
            } else {
                VALIDATION_REJECT
            }
        }
    }
}
