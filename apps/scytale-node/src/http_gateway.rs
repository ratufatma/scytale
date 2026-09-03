//! Lightweight Read-Only REST / JSON HTTP Gateway for Scytale Blockchain.
//!
//! Provides real-time block explorer and monitoring endpoints over HTTP.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use scytale_bridge::{PassbookViewDto, ProvenanceTraceDto};
use scytale_core::codec::CanonicalDeserialize;
use scytale_core::{Address, Block, Transaction, QUANTA_PER_SCY};
use scytale_primitives::{from_hex, to_hex, Hash256, OutPoint};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::ipc::{map_passbook_view, map_provenance_trace};
use crate::node::Node;
use crate::passbook::Passbook;

/// Default HTTP Gateway bind address.
pub const DEFAULT_HTTP_BIND: &str = "127.0.0.1:8332";

/// Embedded static Web Explorer HTML single-page application.
const EXPLORER_HTML: &str = include_str!("../../../web/explorer/index.html");

async fn serve_explorer() -> Html<String> {
    if let Ok(content) = std::fs::read_to_string("web/explorer/index.html") {
        return Html(content);
    }
    if let Ok(content) = std::fs::read_to_string("/web/explorer/index.html") {
        return Html(content);
    }
    Html(EXPLORER_HTML.to_string())
}

/// Embedded static SVG favicon and branding asset.
const FAVICON_SVG: &str = include_str!("../../../web/explorer/favicon.svg");

async fn serve_favicon_svg() -> impl axum::response::IntoResponse {
    let content = if let Ok(c) = std::fs::read_to_string("web/explorer/favicon.svg") {
        c
    } else if let Ok(c) = std::fs::read_to_string("/web/explorer/favicon.svg") {
        c
    } else {
        FAVICON_SVG.to_string()
    };

    ([(axum::http::header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")], content)
}

/// Response payload for `GET /api/v1/status`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusResponse {
    pub runtime_state: String,
    pub canonical_height: u64,
    pub canonical_tip: String,
    #[serde(default)]
    pub utxo_root: String,
    #[serde(default)]
    pub peer_count: usize,
    pub mempool_tx_count: usize,
    pub mining_active: bool,
}

/// Summary representation of a block for list queries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockSummaryResponse {
    pub hash: String,
    pub height: u64,
    pub previous_block_hash: String,
    pub timestamp: u64,
    pub difficulty_target: String,
    pub nonce: u64,
    pub tx_count: usize,
    pub total_quanta: u64,
    pub total_scy: String,
}

/// Detailed block representation with all transactions.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockDetailResponse {
    pub hash: String,
    pub height: u64,
    pub version: u32,
    pub previous_block_hash: String,
    pub transaction_commitment: String,
    pub timestamp: u64,
    pub difficulty_target: String,
    pub nonce: u64,
    pub tx_count: usize,
    pub transactions: Vec<TransactionDetailResponse>,
}

/// Detailed transaction representation for block explorer inspection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionDetailResponse {
    pub txid: String,
    pub version: u32,
    pub is_coinbase: bool,
    pub inputs: Vec<TxInResponse>,
    pub outputs: Vec<TxOutResponse>,
    pub total_output_quanta: u64,
    pub total_output_scy: String,
    pub status: String,
    pub block_height: Option<u64>,
    pub block_hash: Option<String>,
}

/// Transaction input representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxInResponse {
    pub previous_output: String,
    pub authorization_proof_hex: String,
}

/// Transaction output representation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TxOutResponse {
    pub index: u32,
    pub value_quanta: u64,
    pub value_scy: String,
    pub locking_script_hex: String,
    #[serde(default)]
    pub locking_script: String,
    pub script_type: String,
    pub address: Option<String>,
    pub op_return_payload: Option<String>,
}

pub type TxOutputDto = TxOutResponse;

/// Mempool inspection response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MempoolResponse {
    pub count: usize,
    pub max_count: usize,
    pub total_bytes: usize,
    pub max_bytes: usize,
    pub total_fees_quanta: u64,
    pub min_relay_fee_milli: u64,
    pub transactions: Vec<MempoolTxSummary>,
}

pub type MempoolStatusDto = MempoolResponse;

/// Summary of a single mempool pending transaction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MempoolTxSummary {
    pub txid: String,
    pub fee_quanta: u64,
    pub size_bytes: usize,
    pub fee_rate_milli: u64,
    pub added_time: u64,
    #[serde(default)]
    pub fee_rate: u64,
    #[serde(default)]
    pub timestamp: u64,
}

/// Standard error response payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
}

/// Formats quanta into decimal SCY string representation without float arithmetic.
fn format_scy(quanta: u64) -> String {
    format!("{}.{:08}", quanta / QUANTA_PER_SCY, quanta % QUANTA_PER_SCY)
}

/// Strips optional "0x" or "0X" hex prefix.
fn clean_hex(s: &str) -> &str {
    s.strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s)
}

/// Analyzes a locking script to determine script type, human-readable address,
/// and OP_RETURN payload if present.
fn analyze_locking_script(script: &[u8]) -> (String, Option<String>, Option<String>) {
    // Check OP_RETURN (0x6a)
    if script.first() == Some(&0x6a) {
        let payload_bytes: &[u8] = if script.len() > 1 {
            if script[1] <= 75 {
                let end = (2 + script[1] as usize).min(script.len());
                &script[2..end]
            } else if script[1] == 0x4c && script.len() > 2 {
                let len = script[2] as usize;
                let end = (3 + len).min(script.len());
                &script[3..end]
            } else {
                &script[1..]
            }
        } else {
            &[]
        };
        let payload_str = String::from_utf8(payload_bytes.to_vec())
            .unwrap_or_else(|_| format!("0x{}", to_hex(payload_bytes)));
        return ("op_return".to_string(), None, Some(payload_str));
    }

    // Check standard P2PKH: OP_DUP(0x73) OP_BLAKE3(0xa0) OP_PUSHBYTES_32(0x20) [32B hash] OP_EQUALVERIFY(0x88) OP_CHECKSIG(0xac)
    if script.len() == 37
        && script[0] == 0x73
        && script[1] == 0xa0
        && script[2] == 0x20
        && script[35] == 0x88
        && script[36] == 0xac
    {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&script[3..35]);
        let addr = Address::new(hash);
        if let Ok(bech32_str) = addr.to_bech32() {
            return ("p2pkh".to_string(), Some(bech32_str), None);
        }
    }

    ("custom".to_string(), None, None)
}

/// Converts a domain Transaction into TransactionDetailResponse.
fn tx_to_detail(
    tx: &Transaction,
    height: Option<u64>,
    block_hash: Option<&str>,
    status: &str,
) -> TransactionDetailResponse {
    let inputs = tx
        .inputs
        .iter()
        .map(|input| TxInResponse {
            previous_output: format!(
                "{}:{}",
                input.previous_output.txid, input.previous_output.index
            ),
            authorization_proof_hex: to_hex(&input.authorization),
        })
        .collect();

    let mut total_output_quanta = 0u64;
    let outputs = tx
        .outputs
        .iter()
        .enumerate()
        .map(|(idx, out)| {
            total_output_quanta = total_output_quanta.saturating_add(out.value);
            let (script_type, address, op_return_payload) =
                analyze_locking_script(&out.locking_condition);
            let hex = to_hex(&out.locking_condition);
            TxOutResponse {
                index: idx as u32,
                value_quanta: out.value,
                value_scy: format_scy(out.value),
                locking_script_hex: hex.clone(),
                locking_script: hex,
                script_type,
                address,
                op_return_payload,
            }
        })
        .collect();

    TransactionDetailResponse {
        txid: format!("0x{}", tx.txid()),
        version: tx.version,
        is_coinbase: tx.is_coinbase(),
        inputs,
        outputs,
        total_output_quanta,
        total_output_scy: format_scy(total_output_quanta),
        status: status.to_string(),
        block_height: height,
        block_hash: block_hash.map(|s| s.to_string()),
    }
}

/// Converts a domain Block into BlockDetailResponse.
fn block_to_detail(block: &Block, height: u64) -> BlockDetailResponse {
    let block_hash = format!("0x{}", block.header.hash());
    let txs = block
        .transactions
        .iter()
        .map(|tx| tx_to_detail(tx, Some(height), Some(&block_hash), "Confirmed"))
        .collect();

    BlockDetailResponse {
        hash: block_hash,
        height,
        version: block.header.version,
        previous_block_hash: format!("0x{}", block.header.previous_block_hash),
        transaction_commitment: format!("0x{}", block.header.transaction_commitment),
        timestamp: block.header.timestamp,
        difficulty_target: format!("0x{:08x}", block.header.difficulty_target),
        nonce: block.header.nonce,
        tx_count: block.transactions.len(),
        transactions: txs,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Route Handlers
// ─────────────────────────────────────────────────────────────────────────────

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok"
    }))
}

async fn get_status(State(node): State<Arc<Node>>) -> Json<StatusResponse> {
    let utxo_root = format!("0x{}", node.query_utxo_set().compute_utxo_root());
    Json(StatusResponse {
        runtime_state: format!("{:?}", node.state()),
        canonical_height: node.canonical_height(),
        canonical_tip: format!("0x{}", node.canonical_tip()),
        utxo_root,
        peer_count: node.peer_count(),
        mempool_tx_count: node.mempool_len(),
        mining_active: node.mining_running(),
    })
}

async fn get_block_tip(
    State(node): State<Arc<Node>>,
) -> Result<Json<BlockDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    let chain = node.query_canonical_chain().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    if let Some((block, height)) = chain.last() {
        Ok(Json(block_to_detail(block, *height)))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "No canonical tip found".into(),
            }),
        ))
    }
}

#[derive(Debug, Deserialize)]
struct BlocksQuery {
    limit: Option<usize>,
}

async fn get_blocks(
    State(node): State<Arc<Node>>,
    Query(query): Query<BlocksQuery>,
) -> Result<Json<Vec<BlockSummaryResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let chain = node.query_canonical_chain().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let summaries = chain
        .iter()
        .rev()
        .take(limit)
        .map(|(b, h)| {
            let total_quanta: u64 = b
                .transactions
                .iter()
                .flat_map(|t| &t.outputs)
                .map(|o| o.value)
                .sum();
            BlockSummaryResponse {
                hash: format!("0x{}", b.header.hash()),
                height: *h,
                previous_block_hash: format!("0x{}", b.header.previous_block_hash),
                timestamp: b.header.timestamp,
                difficulty_target: format!("0x{:08x}", b.header.difficulty_target),
                nonce: b.header.nonce,
                tx_count: b.transactions.len(),
                total_quanta,
                total_scy: format_scy(total_quanta),
            }
        })
        .collect();

    Ok(Json(summaries))
}

async fn get_block_by_identifier(
    State(node): State<Arc<Node>>,
    Path(identifier): Path<String>,
) -> Result<Json<BlockDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    let chain = node.query_canonical_chain().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // 1. Check if identifier is a height number
    if let Ok(height) = identifier.parse::<u64>() {
        if let Some((block, h)) = chain.iter().find(|(_, h)| *h == height) {
            return Ok(Json(block_to_detail(block, *h)));
        } else if identifier.chars().all(|c| c.is_ascii_digit()) && identifier.len() != 64 {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Block at height {height} not found"),
                }),
            ));
        }
    }

    // 2. Treat as hex hash
    let hash_hex = clean_hex(&identifier);
    let hash = Hash256::from_str(hash_hex).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid block identifier (not valid height or hex hash): {e}"),
            }),
        )
    })?;

    // Check canonical chain first
    if let Some((block, h)) = chain.iter().find(|(b, _)| b.header.hash() == hash) {
        return Ok(Json(block_to_detail(block, *h)));
    }

    // Check storage for side branch or orphaned block
    if let Ok(Some(block)) = node.storage_handle().get_block(&hash) {
        return Ok(Json(block_to_detail(&block, 0)));
    }

    Err((
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: format!("Block '{identifier}' not found"),
        }),
    ))
}

async fn get_transaction(
    State(node): State<Arc<Node>>,
    Path(txid_str): Path<String>,
) -> Result<Json<TransactionDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    let hash_hex = clean_hex(&txid_str);
    let txid = Hash256::from_str(hash_hex).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid txid hex: {e}"),
            }),
        )
    })?;

    // 1. Check mempool
    let mempool_entries = node.query_mempool();
    if let Some(entry) = mempool_entries.iter().find(|e| e.txid == txid) {
        return Ok(Json(tx_to_detail(
            &entry.transaction,
            None,
            None,
            "Pending",
        )));
    }

    // 2. Check storage
    if let Ok(Some(tx)) = node.lookup_transaction(&txid) {
        let chain = node.query_canonical_chain().unwrap_or_default();
        let (b_hash, b_height) = chain
            .iter()
            .find(|(b, _)| b.transactions.iter().any(|t| t.txid() == txid))
            .map(|(b, h)| (Some(format!("0x{}", b.header.hash())), Some(*h)))
            .unwrap_or((None, None));

        return Ok(Json(tx_to_detail(
            &tx,
            b_height,
            b_hash.as_deref(),
            "Confirmed",
        )));
    }

    Err((
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: format!("Transaction '{txid_str}' not found"),
        }),
    ))
}

async fn get_mempool(State(node): State<Arc<Node>>) -> Json<MempoolResponse> {
    let entries = node.query_mempool();
    let total_bytes = node.mempool_total_bytes();
    let total_fees_quanta = node.mempool_total_fees();
    let count = entries.len();

    let txs = entries
        .into_iter()
        .map(|e| MempoolTxSummary {
            txid: format!("0x{}", e.txid),
            fee_quanta: e.fee,
            size_bytes: e.size_bytes,
            fee_rate_milli: e.fee_rate,
            added_time: e.added_time,
            fee_rate: e.fee_rate,
            timestamp: e.added_time,
        })
        .collect::<Vec<_>>();

    Json(MempoolResponse {
        count,
        max_count: 5000,
        total_bytes,
        max_bytes: 5_000_000,
        total_fees_quanta,
        min_relay_fee_milli: 1000,
        transactions: txs,
    })
}

async fn get_passbook(
    State(node): State<Arc<Node>>,
    Path(locking_script_or_addr): Path<String>,
) -> Result<Json<PassbookViewDto>, (StatusCode, Json<ErrorResponse>)> {
    let (lock_bytes, display_label) = if locking_script_or_addr
        .to_ascii_lowercase()
        .starts_with("scy1")
    {
        let addr = scytale_core::Address::parse(&locking_script_or_addr).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid Bech32 address: {e}"),
                }),
            )
        })?;
        let script = scytale_script::builder::ScriptBuilder::new()
            .push_opcode(scytale_script::opcode::OpCode::OpDup)
            .push_opcode(scytale_script::opcode::OpCode::OpBlake3)
            .push_data(addr.hash())
            .push_opcode(scytale_script::opcode::OpCode::OpEqualVerify)
            .push_opcode(scytale_script::opcode::OpCode::OpCheckSig)
            .build();
        (script, locking_script_or_addr)
    } else {
        let hex_clean = clean_hex(&locking_script_or_addr);
        let lock = from_hex(hex_clean).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Invalid locking script hex: {e}"),
                }),
            )
        })?;
        (lock, hex_clean.to_string())
    };

    let passbook = Passbook::new(vec![lock_bytes]);
    match passbook.view(&node) {
        Ok(view) => Ok(Json(map_passbook_view(display_label, view))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

async fn get_provenance(
    State(node): State<Arc<Node>>,
    Path((txid_str, index)): Path<(String, u32)>,
) -> Result<Json<ProvenanceTraceDto>, (StatusCode, Json<ErrorResponse>)> {
    let hash_hex = clean_hex(&txid_str);
    let txid = Hash256::from_str(hash_hex).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid txid hex: {e}"),
            }),
        )
    })?;

    let outpoint = OutPoint::new(txid, index);
    let passbook = Passbook::new(vec![]);
    match passbook.provenance(&node, &outpoint) {
        Ok(steps) => Ok(Json(map_provenance_trace(outpoint, steps))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SubmitTxRequest {
    pub tx_hex: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubmitTxResponse {
    pub txid: String,
    pub status: String,
}

async fn submit_tx(
    State(node): State<Arc<Node>>,
    Json(payload): Json<SubmitTxRequest>,
) -> Result<Json<SubmitTxResponse>, (StatusCode, Json<ErrorResponse>)> {
    let raw_hex = clean_hex(&payload.tx_hex);
    let bytes = from_hex(raw_hex).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid transaction hex: {e}"),
            }),
        )
    })?;
    let tx = Transaction::from_canonical_bytes(&bytes).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Transaction deserialization error: {e}"),
            }),
        )
    })?;
    let txid = node.submit_transaction(tx).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;
    Ok(Json(SubmitTxResponse {
        txid: format!("0x{txid}"),
        status: "Submitted".to_string(),
    }))
}

/// Builds the Axum router for the HTTP Gateway.
pub fn router(node: Arc<Node>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(serve_explorer))
        .route("/index.html", get(serve_explorer))
        .route("/favicon.svg", get(serve_favicon_svg))
        .route("/gemini-svg.svg", get(serve_favicon_svg))
        .route("/logo.svg", get(serve_favicon_svg))
        .route("/favicon.ico", get(serve_favicon_svg))
        .route("/health", get(health_check))
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/blocks", get(get_blocks))
        .route("/api/v1/blocks/tip", get(get_block_tip))
        .route("/api/v1/blocks/:identifier", get(get_block_by_identifier))
        .route("/api/v1/tx", post(submit_tx))
        .route("/api/v1/tx/:txid", get(get_transaction))
        .route("/api/v1/mempool", get(get_mempool))
        .route("/api/v1/passbook/:locking_script_hex", get(get_passbook))
        .route("/api/v1/provenance/:txid/:index", get(get_provenance))
        .layer(cors)
        .with_state(node)
}

/// Spawns the HTTP Gateway listener and handles graceful shutdown.
pub async fn run_http_gateway(
    bind_addr: &str,
    node: Arc<Node>,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app = router(node);
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    tracing::info!("HTTP read-only gateway listening on http://{}", local_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
            tracing::info!("HTTP gateway received shutdown signal");
        })
        .await?;

    tracing::info!("HTTP gateway terminated cleanly");
    Ok(())
}
