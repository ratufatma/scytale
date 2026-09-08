//! Optional account-number alias wrapper around a Passbook ID.
//!
//! This crate is deliberately outside the consensus dependency graph. It owns
//! local PIN protection, candidate derivation, bind messages, and sidecar data.

pub mod candidate;
pub mod pin_vault;
pub mod protocol;
pub mod store;

pub use candidate::{derive_candidate, AccountNumber, AccountNumberError};
pub use pin_vault::{
    decrypt_key, encrypt_key, is_being_traced, EncryptedKeyEnvelope, PinCode, VaultError,
};
pub use protocol::{BindRequest, BindResponse};
pub use store::{AliasStore, StoreError};
