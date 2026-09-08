use scytale_account::pin_vault::PinCode;
use serde_json::Value;
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub network: &'static str,
    pub node_url: &'static str,
    pub connected: bool,
}

#[derive(Debug, Serialize)]
pub struct NodeStatus {
    pub connected: bool,
    pub node_url: String,
    pub response_time_ms: u128,
    pub block_height: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AccountDetails {
    pub account_number: Option<String>,
    pub passbook_id: Option<String>,
    pub balance_quanta: Option<u64>,
    pub registered: bool,
}

#[derive(Debug, Serialize)]
pub struct WalletSummary {
    pub account_number: Option<String>,
    pub path: String,
    pub active: bool,
}

fn home_path(relative: &str) -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(relative)
}

fn config_path() -> PathBuf {
    home_path(".scytale/config.json")
}

fn active_wallet_path() -> Option<String> {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.get("active_wallet_path").and_then(Value::as_str).map(str::to_owned))
}

fn wallet_summary(path: PathBuf, active_path: &Option<String>) -> Option<WalletSummary> {
    let value: Value = serde_json::from_str(&fs::read_to_string(&path).ok()?).ok()?;
    let account_number = value.get("account_number").and_then(Value::as_str).map(str::to_owned);
    let path_string = path.to_string_lossy().into_owned();
    Some(WalletSummary {
        account_number,
        active: active_path.as_deref() == Some(path_string.as_str()),
        path: path_string,
    })
}

fn configured_node_url() -> String {
    let path = PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".scytale/config.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.get("node_url").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "http://116.212.72.89:8332".to_string())
}

fn json_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn validate_address(value: &str, field: &str) -> Result<(), String> {
    let scy_account = value.starts_with("SCY-")
        && value.len() == 10
        && value[4..].bytes().all(|byte| byte.is_ascii_digit());
    let scy_address = value.starts_with("scy1") && value.len() > 10;
    if scy_account || scy_address {
        Ok(())
    } else {
        Err(format!("Invalid {field}: use SCY-xxxxxx or scy1..."))
    }
}

#[tauri::command]
pub fn create_wallet_with_pin(pin: String) -> Result<String, String> {
    PinCode::new(&pin)
        .map(|_| "PIN accepted; wallet creation is ready.".to_string())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_system_status() -> SystemStatus {
    SystemStatus {
        network: "Global Testnet",
        node_url: "http://116.212.72.89:8332",
        connected: false,
    }
}

#[tauri::command]
pub async fn get_node_telemetry(node_url: Option<String>) -> Result<NodeStatus, String> {
    let node_url = node_url.unwrap_or_else(configured_node_url).trim_end_matches('/').to_string();
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(5)).build();
    let response = agent
        .get(&format!("{node_url}/health"))
        .call()
        .or_else(|_| agent.get(&format!("{node_url}/api/v1/status")).call());
    match response {
        Ok(response) => {
            let body: Value = response.into_json().unwrap_or(Value::Null);
            Ok(NodeStatus {
                connected: true,
                node_url,
                response_time_ms: started.elapsed().as_millis(),
                block_height: json_u64(&body, &["block_height", "height", "canonical_height"]),
                error: None,
            })
        }
        Err(error) => Ok(NodeStatus {
            connected: false,
            node_url,
            response_time_ms: started.elapsed().as_millis(),
            block_height: None,
            error: Some(error.to_string()),
        }),
    }
}

#[tauri::command]
pub fn get_active_account_details(wallet_path: Option<String>) -> Result<AccountDetails, String> {
    let path = wallet_path.map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".scytale/wallet.json")
    });
    let value: Value = serde_json::from_str(&fs::read_to_string(path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    let account_number = value.get("account_number").and_then(Value::as_str).map(str::to_owned);
    let passbook_id = value
        .get("passbook_id")
        .or_else(|| value.get("address"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let balance_quanta = json_u64(&value, &["balance_quanta", "balance", "confirmed_balance_quanta"]);
    Ok(AccountDetails {
        registered: account_number.is_some(),
        account_number,
        passbook_id,
        balance_quanta,
    })
}

#[tauri::command]
pub fn list_local_wallets() -> Result<Vec<WalletSummary>, String> {
    let active_path = active_wallet_path();
    let mut roots = vec![home_path(".scytale")];
    roots.push(PathBuf::from("/mnt/ssd/scytale-lab/scytale"));
    let mut wallets = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        let entries = fs::read_dir(root).map_err(|error| error.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                if let Some(summary) = wallet_summary(path, &active_path) {
                    wallets.push(summary);
                }
            }
        }
    }
    wallets.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(wallets)
}

#[tauri::command]
pub fn set_active_wallet(wallet_path: String) -> Result<bool, String> {
    let path = PathBuf::from(&wallet_path);
    if !path.is_file() {
        return Err("Wallet file does not exist".to_string());
    }
    let config_file = config_path();
    let mut config = fs::read_to_string(&config_file)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    config["active_wallet_path"] = Value::String(wallet_path);
    if let Some(parent) = config_file.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(config_file, serde_json::to_vec_pretty(&config).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    Ok(true)
}

#[tauri::command]
pub fn draft_transaction(
    sender: String,
    recipient: String,
    amount_quanta: u64,
    fee_quanta: u64,
) -> Result<Value, String> {
    validate_address(&sender, "sender")?;
    validate_address(&recipient, "recipient")?;
    if amount_quanta == 0 {
        return Err("Amount must be greater than zero".to_string());
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    Ok(serde_json::json!({
        "version": 1,
        "inputs": [{ "sender": sender }],
        "outputs": [{ "recipient": recipient, "amount_quanta": amount_quanta }],
        "fee_quanta": fee_quanta,
        "timestamp": timestamp,
        "status": "draft"
    }))
}

#[tauri::command]
pub async fn sign_and_broadcast_transaction(
    wallet_path: String,
    pin: String,
    draft: Value,
    node_url: Option<String>,
) -> Result<Value, String> {
    PinCode::new(&pin).map_err(|error| error.to_string())?;
    let wallet_bytes = fs::read(&wallet_path).map_err(|error| error.to_string())?;
    let draft_bytes = serde_json::to_vec(&draft).map_err(|error| error.to_string())?;
    let mut signing_material = Vec::with_capacity(wallet_bytes.len() + draft_bytes.len());
    signing_material.extend_from_slice(&wallet_bytes);
    signing_material.extend_from_slice(&draft_bytes);
    let signature = blake3::hash(&signing_material).to_hex().to_string();
    let payload = serde_json::json!({ "draft": draft, "signature": signature, "wallet_path": wallet_path });
    let endpoint = node_url.unwrap_or_else(configured_node_url).trim_end_matches('/').to_string();
    let response = ureq::post(&format!("{endpoint}/api/v1/transactions"))
        .send_json(payload.clone());
    match response {
        Ok(response) => {
            let body: Value = response.into_json().unwrap_or(payload);
            Ok(serde_json::json!({ "broadcast": true, "response": body }))
        }
        Err(error) => Err(format!("Broadcast failed: {error}")),
    }
}
