//! Human-Readable Bech32 Address Encoding for Scytale.
//!
//! Implements BIP-173 Bech32 address encoding and decoding with the default
//! Human-Readable Part (HRP) `"scy"`, 6-character BCH error-detecting checksums,
//! and backward-compatible raw hexadecimal fallback parsing.

use bech32::{decode, encode, Bech32, Hrp};
use scytale_primitives::from_hex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Domain errors for Address operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AddressError {
    #[error("Invalid HRP: expected {expected}, got {got}")]
    InvalidHrp { expected: String, got: String },
    #[error("Invalid payload length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
    #[error("Bech32 decoding error: {0}")]
    Bech32Decode(String),
    #[error("Hex decoding error: {0}")]
    HexDecode(String),
    #[error("Unrecognized address format")]
    UnrecognizedFormat,
}

/// A human-readable Scytale address wrapping a 32-byte public key hash and an HRP.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Address {
    hrp: String,
    hash: [u8; 32],
}

impl Address {
    /// Canonical Human-Readable Part for Scytale addresses.
    pub const DEFAULT_HRP: &'static str = "scy";

    /// Creates an Address from a 32-byte public key hash using the default HRP (`"scy"`).
    pub fn new(hash: [u8; 32]) -> Self {
        Self::with_hrp(Self::DEFAULT_HRP, hash)
    }

    /// Alias for `new` for clarity when converting public key hashes.
    pub fn from_pubkey_hash(hash: [u8; 32]) -> Self {
        Self::new(hash)
    }

    /// Creates an Address with a custom HRP.
    pub fn with_hrp(hrp: &str, hash: [u8; 32]) -> Self {
        Self {
            hrp: hrp.to_ascii_lowercase(),
            hash,
        }
    }

    /// Returns the Human-Readable Part string.
    pub fn hrp(&self) -> &str {
        &self.hrp
    }

    /// Returns a reference to the underlying 32-byte hash.
    pub fn hash(&self) -> &[u8; 32] {
        &self.hash
    }

    /// Encodes the address into its canonical Bech32 string format (`scy1...`).
    pub fn to_bech32(&self) -> Result<String, AddressError> {
        let hrp = Hrp::parse(&self.hrp).map_err(|e| AddressError::Bech32Decode(e.to_string()))?;
        encode::<Bech32>(hrp, &self.hash).map_err(|e| AddressError::Bech32Decode(e.to_string()))
    }

    /// Parses an address from either a Bech32 string (`scy1...`) or a legacy
    /// 64-character raw hexadecimal string (with or without `0x` prefix).
    pub fn parse(input: &str) -> Result<Self, AddressError> {
        let trimmed = input.trim();

        // 1. Bech32 parsing (starts with "scy1" case-insensitively)
        if trimmed.len() > 4 && trimmed[..4].eq_ignore_ascii_case("scy1") {
            let (hrp, data) =
                decode(trimmed).map_err(|e| AddressError::Bech32Decode(e.to_string()))?;

            if !hrp.as_str().eq_ignore_ascii_case(Self::DEFAULT_HRP) {
                return Err(AddressError::InvalidHrp {
                    expected: Self::DEFAULT_HRP.to_string(),
                    got: hrp.as_str().to_string(),
                });
            }

            if data.len() != 32 {
                return Err(AddressError::InvalidLength(data.len()));
            }

            let mut hash = [0u8; 32];
            hash.copy_from_slice(&data);
            return Ok(Self {
                hrp: Self::DEFAULT_HRP.to_string(),
                hash,
            });
        }

        // 2. Legacy raw hexadecimal fallback
        let hex_clean = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        if hex_clean.len() == 64 {
            let bytes = from_hex(hex_clean).map_err(|e| AddressError::HexDecode(e.to_string()))?;
            if bytes.len() == 32 {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(&bytes);
                return Ok(Self::new(hash));
            }
        }

        Err(AddressError::UnrecognizedFormat)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_bech32() {
            Ok(s) => write!(f, "{}", s),
            Err(_) => write!(f, "scy1<invalid>"),
        }
    }
}

impl FromStr for Address {
    type Err = AddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for Address {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_bech32()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}
