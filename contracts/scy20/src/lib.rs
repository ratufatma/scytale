#![no_std]
extern crate alloc;

pub mod codec;
pub mod error;
pub mod types;
pub mod validator;

pub use codec::{
    deserialize_datum, deserialize_redeemer, serialize_datum, serialize_redeemer,
};
pub use error::Scy20Error;
pub use types::{Address, Scy20Datum, Scy20Redeemer, ScriptContext, TokenId, TokenMetadata};
pub use validator::{
    validate_burn, validate_mint, validate_scy20_execution, validate_transfer,
};

pub use scytale_sdk::{decode_payload, TxContext, VALIDATION_REJECT, VALIDATION_SUCCESS};

/// ABI entrypoint Wasm untuk validasi kontrak pintar eUTXO SCY-20.
#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn validate(
    datum_ptr: i32,
    datum_len: i32,
    redeemer_ptr: i32,
    redeemer_len: i32,
    ctx_ptr: i32,
    ctx_len: i32,
) -> i32 {
    if datum_ptr < 0
        || datum_len < 0
        || redeemer_ptr < 0
        || redeemer_len < 0
        || ctx_ptr < 0
        || ctx_len < 0
    {
        return VALIDATION_REJECT;
    }

    let datum_slice =
        unsafe { core::slice::from_raw_parts(datum_ptr as *const u8, datum_len as usize) };
    let redeemer_slice =
        unsafe { core::slice::from_raw_parts(redeemer_ptr as *const u8, redeemer_len as usize) };
    let ctx_slice =
        unsafe { core::slice::from_raw_parts(ctx_ptr as *const u8, ctx_len as usize) };

    let datum: Scy20Datum = match decode_payload(datum_slice) {
        Ok(d) => d,
        Err(_) => return VALIDATION_REJECT,
    };

    let redeemer: Scy20Redeemer = match decode_payload(redeemer_slice) {
        Ok(r) => r,
        Err(_) => return VALIDATION_REJECT,
    };

    let ctx: TxContext = match decode_payload(ctx_slice) {
        Ok(c) => c,
        Err(_) => return VALIDATION_REJECT,
    };

    match validate_scy20_execution(&datum, &redeemer, &ctx) {
        Ok(()) => VALIDATION_SUCCESS,
        Err(_) => VALIDATION_REJECT,
    }
}
