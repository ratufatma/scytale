extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use scytale_sdk::serde_signature;
pub use scytale_sdk::TxContext;
pub use scytale_sdk::TxContext as ScriptContext;
use serde::{Deserialize, Serialize};

pub type TokenId = [u8; 32];
pub type Address = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenMetadata {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub max_supply: Option<u128>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scy20Datum {
    pub token_id: TokenId,
    pub owner: Address,
    pub amount: u128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scy20Redeemer {
    Transfer {
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
        outputs: Vec<Scy20Datum>,
        fee: u128,
    },
    Mint {
        amount: u128,
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
        outputs: Vec<Scy20Datum>,
        metadata: Option<TokenMetadata>,
        current_supply: u128,
    },
    Burn {
        amount: u128,
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
        outputs: Vec<Scy20Datum>,
    },
}
