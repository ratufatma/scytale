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

/// Modul pembantu serde untuk serialisasi dan deserialisasi array [u8; 64] tanda tangan kriptografis.
pub mod serde_signature {
    use core::fmt;
    use serde::{de, Deserializer, Serializer};

    pub fn serialize<S>(sig: &[u8; 64], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bytes(sig)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SigVisitor;
        impl<'de> de::Visitor<'de> for SigVisitor {
            type Value = [u8; 64];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a 64-byte signature array")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if v.len() == 64 {
                    let mut arr = [0u8; 64];
                    arr.copy_from_slice(v);
                    Ok(arr)
                } else {
                    Err(de::Error::invalid_length(v.len(), &self))
                }
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                let mut arr = [0u8; 64];
                for (i, byte) in arr.iter_mut().enumerate() {
                    *byte = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_bytes(SigVisitor)
    }
}

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

#[cfg(all(not(target_arch = "wasm32"), feature = "std"))]
/// Memverifikasi tanda tangan Ed25519 secara native pada target host (std).
pub fn verify_ed25519(public_key: &[u8; 32], signature: &[u8; 64], message: &[u8]) -> bool {
    let vk = match ed25519_dalek::VerifyingKey::from_bytes(public_key) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let sig = ed25519_dalek::Signature::from_bytes(signature);
    vk.verify_strict(message, &sig).is_ok()
}

#[cfg(all(not(target_arch = "wasm32"), feature = "std"))]
/// Menghitung 32-byte hash BLAKE3 secara native pada target host (std).
pub fn blake3_hash(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "std")))]
pub fn verify_ed25519(_public_key: &[u8; 32], _signature: &[u8; 64], _message: &[u8]) -> bool {
    false
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "std")))]
pub fn blake3_hash(_data: &[u8]) -> [u8; 32] {
    [0u8; 32]
}

#[cfg(target_arch = "wasm32")]
/// Memverifikasi tanda tangan Ed25519 melalui host syscall ScyVM ("env"."scytale_crypto_ed25519_verify").
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

#[cfg(target_arch = "wasm32")]
/// Menghitung 32-byte hash BLAKE3 melalui host syscall ScyVM ("env"."scytale_crypto_blake3").
pub fn blake3_hash(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    unsafe {
        scytale_crypto_blake3(data.as_ptr(), data.len() as i32, out.as_mut_ptr());
    }
    out
}
