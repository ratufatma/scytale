use crate::error::StratumRpcError;
use serde::{Deserialize, Serialize};

/// Top-level frame for Stratum JSON-RPC 2.0 messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum StratumRpcMessage {
    Request(StratumRequest),
    Response(StratumResponse),
    Notification(StratumNotification),
}

/// Inbound JSON-RPC request from worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StratumRequest {
    #[serde(default)]
    pub id: Option<serde_json::Value>,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

impl StratumRequest {
    pub fn new(id: Option<serde_json::Value>, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }
}

/// Outbound JSON-RPC response to worker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StratumResponse {
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<StratumRpcError>,
}

impl StratumResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, error: StratumRpcError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// Outbound JSON-RPC notification to worker (no `id` field).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StratumNotification {
    pub method: String,
    pub params: serde_json::Value,
}

impl StratumNotification {
    pub fn new(method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            method: method.into(),
            params,
        }
    }

    /// Creates a `mining.set_difficulty` notification.
    pub fn set_difficulty(difficulty: f64) -> Self {
        Self::new("mining.set_difficulty", serde_json::json!([difficulty]))
    }

    /// Creates a `mining.notify` notification according to SSP-1 standard.
    #[allow(clippy::too_many_arguments)]
    pub fn notify(
        job_id: &str,
        prev_hash: &str,
        coinbase1: &str,
        coinbase2: &str,
        merkle_branches: &[String],
        version: u32,
        bits: u32,
        curtime: u64,
        clean_jobs: bool,
    ) -> Self {
        Self::new(
            "mining.notify",
            serde_json::json!([
                job_id,
                prev_hash,
                coinbase1,
                coinbase2,
                merkle_branches,
                format!("{:08x}", version),
                format!("{:08x}", bits),
                format!("{:08x}", curtime),
                clean_jobs
            ]),
        )
    }
}
