use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Custom error types for Stratum mining pool operations.
#[derive(Debug, Error)]
pub enum StratumError {
    #[error("High hash - share does not meet difficulty target")]
    HighHash,

    #[error("Job not found or expired: {0}")]
    JobNotFound(String),

    #[error("Duplicate share submitted")]
    DuplicateShare,

    #[error("Invalid extranonce2: {0}")]
    InvalidExtranonce2(String),

    #[error("Invalid nonce: {0}")]
    InvalidNonce(String),

    #[error("Invalid timestamp: {0}")]
    InvalidTime(String),

    #[error("Worker not authorized")]
    Unauthorized,

    #[error("Stale job")]
    StaleJob,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Arithmetic overflow: {0}")]
    Arithmetic(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl From<std::io::Error> for StratumError {
    fn from(err: std::io::Error) -> Self {
        StratumError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for StratumError {
    fn from(err: serde_json::Error) -> Self {
        StratumError::Serialization(err.to_string())
    }
}

impl From<hex::FromHexError> for StratumError {
    fn from(err: hex::FromHexError) -> Self {
        StratumError::Protocol(format!("Hex decoding failure: {}", err))
    }
}

impl From<tokio_util::codec::LinesCodecError> for StratumError {
    fn from(err: tokio_util::codec::LinesCodecError) -> Self {
        StratumError::Protocol(format!("Framing error: {}", err))
    }
}

/// JSON-RPC 2.0 error representation for Stratum protocol responses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StratumRpcError {
    pub code: i32,
    pub message: String,
}

impl StratumRpcError {
    pub const OTHER_UNKNOWN: i32 = 20;
    pub const JOB_NOT_FOUND: i32 = 21;
    pub const DUPLICATE_SHARE: i32 = 22;
    pub const LOW_DIFFICULTY_SHARE: i32 = 23;
    pub const UNAUTHORIZED: i32 = 24;
    pub const NOT_SUBSCRIBED: i32 = 25;

    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(
            Self::METHOD_NOT_FOUND,
            format!("Method not found: {}", method),
        )
    }

    pub fn invalid_params(reason: &str) -> Self {
        Self::new(
            Self::INVALID_PARAMS,
            format!("Invalid params: {}", reason),
        )
    }

    pub fn unauthorized() -> Self {
        Self::new(Self::UNAUTHORIZED, "Worker not authorized")
    }

    pub fn job_not_found(job_id: &str) -> Self {
        Self::new(
            Self::JOB_NOT_FOUND,
            format!("Job not found: {}", job_id),
        )
    }

    pub fn duplicate_share() -> Self {
        Self::new(Self::DUPLICATE_SHARE, "Duplicate share submitted")
    }

    pub fn low_difficulty_share() -> Self {
        Self::new(
            Self::LOW_DIFFICULTY_SHARE,
            "High hash - share does not meet target",
        )
    }
}

impl From<StratumError> for StratumRpcError {
    fn from(err: StratumError) -> Self {
        match err {
            StratumError::HighHash => StratumRpcError::low_difficulty_share(),
            StratumError::JobNotFound(job_id) => StratumRpcError::job_not_found(&job_id),
            StratumError::DuplicateShare => StratumRpcError::duplicate_share(),
            StratumError::Unauthorized => StratumRpcError::unauthorized(),
            StratumError::InvalidExtranonce2(msg) => StratumRpcError::invalid_params(&msg),
            StratumError::InvalidNonce(msg) => StratumRpcError::invalid_params(&msg),
            StratumError::InvalidTime(msg) => StratumRpcError::invalid_params(&msg),
            StratumError::StaleJob => StratumRpcError::new(StratumRpcError::JOB_NOT_FOUND, "Stale job"),
            StratumError::Serialization(msg) => StratumRpcError::new(StratumRpcError::PARSE_ERROR, msg),
            StratumError::Io(msg) => StratumRpcError::new(StratumRpcError::INTERNAL_ERROR, msg),
            StratumError::Arithmetic(msg) => StratumRpcError::new(StratumRpcError::INTERNAL_ERROR, msg),
            StratumError::Protocol(msg) => StratumRpcError::new(StratumRpcError::INVALID_REQUEST, msg),
        }
    }
}
