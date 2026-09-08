use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use thiserror::Error;

const ACCOUNT_PREFIX: &str = "SCY-";
const ACCOUNT_SPACE: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AccountNumber(String);

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AccountNumberError {
    #[error("account number must use the SCY-xxxxxx format")]
    InvalidFormat,
}

impl AccountNumber {
    pub fn new(value: impl Into<String>) -> Result<Self, AccountNumberError> {
        let value = value.into();
        let digits = value
            .strip_prefix(ACCOUNT_PREFIX)
            .ok_or(AccountNumberError::InvalidFormat)?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(AccountNumberError::InvalidFormat);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for AccountNumber {
    type Err = AccountNumberError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

/// Derives a deterministic six-digit candidate without claiming it is registered.
pub fn derive_candidate(key_id: &[u8], passbook_id: &str, attempt: u32) -> AccountNumber {
    let mut input = Vec::with_capacity(key_id.len() + passbook_id.len() + 4);
    input.extend_from_slice(key_id);
    input.extend_from_slice(passbook_id.as_bytes());
    input.extend_from_slice(&attempt.to_le_bytes());
    let digest = blake3::hash(&input);
    let raw = u32::from_le_bytes(
        digest.as_bytes()[..4]
            .try_into()
            .expect("fixed digest slice"),
    );
    AccountNumber(format!("{ACCOUNT_PREFIX}{:06}", raw % ACCOUNT_SPACE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_account_numbers() {
        assert!(AccountNumber::new("SCY-000001").is_ok());
        assert!(AccountNumber::new("SCY-1000000").is_err());
        assert!(AccountNumber::new("scy-000001").is_err());
    }

    #[test]
    fn candidate_is_deterministic_and_attempt_changes_input() {
        let first = derive_candidate(b"key", "passbook", 0);
        assert_eq!(first, derive_candidate(b"key", "passbook", 0));
        assert_ne!(first, derive_candidate(b"key", "passbook", 1));
    }
}
