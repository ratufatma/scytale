#![no_std]
extern crate alloc;

use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// Konteks transaksi yang disuntikkan runtime saat validasi UTXO
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxContext {
    pub tx_hash: [u8; 32],
    pub block_time: u64,
    pub input_amount: u64,
    pub fee_burned: u64,
}

/// Helper untuk deserialisasi payload byte yang dikirim dari VM host
pub fn decode_payload<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, bincode::Error> {
    bincode::deserialize(bytes)
}

/// Helper untuk serialisasi data dari kontrak
pub fn encode_payload<T: Serialize>(val: &T) -> Result<Vec<u8>, bincode::Error> {
    bincode::serialize(val)
}

/// Indikator hasil evaluasi kontrak (1 = Lolos/Valid, 0 = Ditolak/Invalid)
pub const VALIDATION_SUCCESS: i32 = 1;
pub const VALIDATION_REJECT: i32 = 0;

// ── Host Functions FFI & Safe Wrappers ────────────────────────────────────────

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
extern "C" {
    fn scytale_crypto_ed25519_verify(
        pk_ptr: *const u8,
        pk_len: i32,
        sig_ptr: *const u8,
        sig_len: i32,
        msg_ptr: *const u8,
        msg_len: i32,
    ) -> i32;

    fn scytale_crypto_blake3(data_ptr: *const u8, data_len: i32, out_ptr: *mut u8);
}

#[cfg(not(target_arch = "wasm32"))]
mod host_mock {
    pub unsafe fn scytale_crypto_ed25519_verify(
        _pk_ptr: *const u8,
        _pk_len: i32,
        _sig_ptr: *const u8,
        _sig_len: i32,
        _msg_ptr: *const u8,
        _msg_len: i32,
    ) -> i32 {
        0
    }

    pub unsafe fn scytale_crypto_blake3(_data_ptr: *const u8, _data_len: i32, _out_ptr: *mut u8) {}
}

#[cfg(not(target_arch = "wasm32"))]
use host_mock::*;

/// Memverifikasi tanda tangan Ed25519 melalui host syscall ScyVM.
///
/// Mengembalikan `true` jika tanda tangan valid, `false` jika tidak valid.
pub fn verify_ed25519(public_key: &[u8; 32], signature: &[u8; 64], message: &[u8]) -> bool {
    let res = unsafe {
        scytale_crypto_ed25519_verify(
            public_key.as_ptr(),
            public_key.len() as i32,
            signature.as_ptr(),
            signature.len() as i32,
            message.as_ptr(),
            message.len() as i32,
        )
    };
    res == 1
}

/// Menghitung 32-byte hash BLAKE3 melalui host syscall ScyVM.
pub fn blake3_hash(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    unsafe {
        scytale_crypto_blake3(data.as_ptr(), data.len() as i32, out.as_mut_ptr());
    }
    out
}
