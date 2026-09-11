extern crate alloc;
use alloc::string::String;
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

/// Canonical token state. This datum must be spent and recreated for every
/// supply-changing operation, which makes both supply and replay state ledger-owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenRegistryDatum {
    pub token_id: TokenId,
    pub authority: Address,
    pub current_supply: u128,
    pub max_supply: Option<u128>,
    pub nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scy20Redeemer {
    Transfer {
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
        nonce: u64,
    },
    Mint {
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
        nonce: u64,
    },
    Burn {
        #[serde(with = "serde_signature")]
        signature: [u8; 64],
        nonce: u64,
    },
}
