use crate::candidate::AccountNumber;
use serde::{Deserialize, Serialize};

/// Canonical bytes signed by a wallet when binding an account alias.
pub fn bind_message(
    public_key: &[u8; 32],
    passbook_id: &str,
    candidate: &AccountNumber,
) -> Vec<u8> {
    let mut message = b"SCYTALE-ACCOUNT-BIND-V1\0".to_vec();
    message.extend_from_slice(public_key);
    message.extend_from_slice(&(passbook_id.len() as u32).to_le_bytes());
    message.extend_from_slice(passbook_id.as_bytes());
    message.extend_from_slice(candidate.as_str().as_bytes());
    message
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BindRequest {
    pub passbook_id: String,
    pub candidate: AccountNumber,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BindResponse {
    Accepted { account_number: AccountNumber },
    RejectedConflict { reason: String },
}
