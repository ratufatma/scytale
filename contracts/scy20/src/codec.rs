extern crate alloc;
use alloc::vec::Vec;
use crate::error::Scy20Error;
use crate::types::{Scy20Datum, Scy20Redeemer};
use scytale_sdk::{decode_payload, encode_payload};

pub fn serialize_datum(datum: &Scy20Datum) -> Result<Vec<u8>, Scy20Error> {
    encode_payload(datum).map_err(|_| Scy20Error::DeserializationFailed)
}

pub fn deserialize_datum(bytes: &[u8]) -> Result<Scy20Datum, Scy20Error> {
    decode_payload(bytes).map_err(|_| Scy20Error::DeserializationFailed)
}

pub fn serialize_redeemer(redeemer: &Scy20Redeemer) -> Result<Vec<u8>, Scy20Error> {
    encode_payload(redeemer).map_err(|_| Scy20Error::DeserializationFailed)
}

pub fn deserialize_redeemer(bytes: &[u8]) -> Result<Scy20Redeemer, Scy20Error> {
    decode_payload(bytes).map_err(|_| Scy20Error::DeserializationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use crate::types::{Address, TokenId};

    const TOKEN_ID: TokenId = [1; 32];
    const OWNER: Address = [2; 32];

    #[test]
    fn test_datum_codec_roundtrip() {
        let datum = Scy20Datum {
            token_id: TOKEN_ID,
            owner: OWNER,
            amount: 123,
        };

        let encoded = serialize_datum(&datum).expect("datum should serialize");
        let decoded = deserialize_datum(&encoded).expect("datum should deserialize");

        assert_eq!(decoded, datum);
    }

    #[test]
    fn test_redeemer_codec_roundtrip() {
        let redeemers = [
            Scy20Redeemer::Transfer {
                signature: [0x55; 64],
                outputs: vec![Scy20Datum {
                    token_id: TOKEN_ID,
                    owner: OWNER,
                    amount: 100,
                }],
                fee: 0,
            },
            Scy20Redeemer::Mint {
                amount: 123,
                signature: [0x77; 64],
                outputs: vec![Scy20Datum {
                    token_id: TOKEN_ID,
                    owner: OWNER,
                    amount: 123,
                }],
                metadata: None,
                current_supply: 0,
            },
            Scy20Redeemer::Burn {
                amount: 45,
                signature: [0x99; 64],
                outputs: Vec::new(),
            },
        ];

        for redeemer in redeemers {
            let encoded = serialize_redeemer(&redeemer).expect("redeemer should serialize");
            let decoded =
                deserialize_redeemer(&encoded).expect("redeemer should deserialize");

            assert_eq!(decoded, redeemer);
        }
    }
}