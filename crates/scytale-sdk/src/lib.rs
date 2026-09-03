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
