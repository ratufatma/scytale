#![deny(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![no_std]
extern crate alloc;

pub mod codec;
pub mod error;
pub mod types;
pub mod validator;

pub use codec::{deserialize_datum, deserialize_redeemer, serialize_datum, serialize_redeemer};
pub use error::Scy20Error;
pub use types::{
    Address, ScriptContext, Scy20Datum, Scy20Redeemer, TokenId, TokenMetadata,
    TokenRegistryDatum,
};
pub use validator::validate_scy20_execution;

pub use scytale_sdk::{
    decode_payload, parse_i32_slice, TxContext, VALIDATION_REJECT, VALIDATION_SUCCESS,
};

/// ABI entrypoint Wasm untuk validasi kontrak pintar eUTXO SCY-20.
#[no_mangle]
#[allow(unsafe_code)]
pub extern "C" fn validate(
    datum_ptr: i32,
    datum_len: i32,
    redeemer_ptr: i32,
    redeemer_len: i32,
    ctx_ptr: i32,
    ctx_len: i32,
) -> i32 {
    let datum_slice = match parse_i32_slice(datum_ptr, datum_len) {
        Some(s) => s,
        None => return VALIDATION_REJECT,
    };
    let redeemer_slice = match parse_i32_slice(redeemer_ptr, redeemer_len) {
        Some(s) => s,
        None => return VALIDATION_REJECT,
    };
    let ctx_slice = match parse_i32_slice(ctx_ptr, ctx_len) {
        Some(s) => s,
        None => return VALIDATION_REJECT,
    };

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
