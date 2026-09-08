use crate::candidate::AccountNumber;
use serde::{Deserialize, Serialize};

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
