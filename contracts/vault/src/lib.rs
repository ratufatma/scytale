#![no_std]
extern crate alloc;

use scytale_sdk::{
    decode_payload, verify_ed25519, TxContext, VALIDATION_REJECT, VALIDATION_SUCCESS,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct VaultDatum {
    pub owner_pubkey: [u8; 32],
    pub unlock_time: u64,
    pub emergency_key: [u8; 32],
    pub penalty_fee: u64,
}

mod serde_signature {
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

#[derive(Serialize, Deserialize)]
pub enum VaultRedeemer {
    NormalWithdraw {
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
    },
    EmergencyRescue {
        penalty_accepted: bool,
    },
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
        VaultRedeemer::NormalWithdraw { signature } => {
            // Wajib melewati batas waktu timelock & tanda tangan valid atas hash transaksi
            if ctx.block_time >= datum.unlock_time
                && verify_ed25519(&datum.owner_pubkey, &signature, &ctx.tx_hash)
            {
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

#[cfg(test)]
mod tests {
    use super::{validate, VaultDatum, VaultRedeemer};
    use ed25519_dalek::{Signer, SigningKey};
    use scytale_sdk::{encode_payload, TxContext, VALIDATION_REJECT, VALIDATION_SUCCESS};

    fn keypair(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn context(block_time: u64, fee_burned: u64) -> TxContext {
        TxContext {
            tx_hash: [0x42; 32],
            block_time,
            input_amount: 100_000,
            fee_burned,
        }
    }

    fn invoke(datum: &VaultDatum, redeemer: &VaultRedeemer, context: &TxContext) -> i32 {
        let datum_bytes = encode_payload(datum).expect("datum should serialize");
        let redeemer_bytes = encode_payload(redeemer).expect("redeemer should serialize");
        let context_bytes = encode_payload(context).expect("context should serialize");

        validate(
            datum_bytes.as_ptr(),
            datum_bytes.len(),
            redeemer_bytes.as_ptr(),
            redeemer_bytes.len(),
            context_bytes.as_ptr(),
            context_bytes.len(),
        )
    }

    fn vault_datum(owner: &SigningKey, emergency_key: &SigningKey) -> VaultDatum {
        VaultDatum {
            owner_pubkey: owner.verifying_key().to_bytes(),
            unlock_time: 1_700_000_000,
            emergency_key: emergency_key.verifying_key().to_bytes(),
            penalty_fee: 500,
        }
    }

    #[test]
    fn normal_withdraw_requires_unlock_time_and_valid_signature() {
        let owner = keypair(1);
        let emergency = keypair(2);
        let datum = vault_datum(&owner, &emergency);
        let unlocked_context = context(datum.unlock_time, 0);
        let signature = owner.sign(&unlocked_context.tx_hash).to_bytes();

        assert_eq!(
            invoke(
                &datum,
                &VaultRedeemer::NormalWithdraw { signature },
                &unlocked_context
            ),
            VALIDATION_SUCCESS
        );

        let early_context = context(datum.unlock_time - 1, 0);
        assert_eq!(
            invoke(
                &datum,
                &VaultRedeemer::NormalWithdraw { signature },
                &early_context
            ),
            VALIDATION_REJECT
        );

        let wrong_signature = emergency.sign(&unlocked_context.tx_hash).to_bytes();
        assert_eq!(
            invoke(
                &datum,
                &VaultRedeemer::NormalWithdraw {
                    signature: wrong_signature,
                },
                &unlocked_context
            ),
            VALIDATION_REJECT
        );
    }

    #[test]
    fn emergency_rescue_requires_accepted_penalty_and_fee_burn() {
        let owner = keypair(3);
        let emergency = keypair(4);
        let datum = vault_datum(&owner, &emergency);

        assert_eq!(
            invoke(
                &datum,
                &VaultRedeemer::EmergencyRescue {
                    penalty_accepted: true,
                },
                &context(1, datum.penalty_fee)
            ),
            VALIDATION_SUCCESS
        );
        assert_eq!(
            invoke(
                &datum,
                &VaultRedeemer::EmergencyRescue {
                    penalty_accepted: false,
                },
                &context(1, datum.penalty_fee)
            ),
            VALIDATION_REJECT
        );
        assert_eq!(
            invoke(
                &datum,
                &VaultRedeemer::EmergencyRescue {
                    penalty_accepted: true,
                },
                &context(1, datum.penalty_fee - 1)
            ),
            VALIDATION_REJECT
        );
    }

    #[test]
    fn malformed_payloads_are_rejected() {
        let invalid = [0xff; 3];
        assert_eq!(
            validate(
                invalid.as_ptr(),
                invalid.len(),
                invalid.as_ptr(),
                invalid.len(),
                invalid.as_ptr(),
                invalid.len(),
            ),
            VALIDATION_REJECT
        );
    }
}
