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
    Transfer,
    Mint { amount: u128 },
    Burn { amount: u128 },
}
