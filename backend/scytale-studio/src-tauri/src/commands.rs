use ed25519_dalek::Signer;
use scytale_account::{decrypt_key, encrypt_key, pin_vault::EncryptedKeyEnvelope, PinCode};
use scytale_core::{Address, CanonicalSerialize, Hash256, OutPoint, Transaction, TxIn, TxOut};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub network: String,
    pub node_url: String,
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

#[derive(Debug, Serialize)]
pub struct PassbookUtxo {
    pub tx_id: String,
    pub output_index: u64,
    pub amount_quanta: u64,
    pub confirmations: u64,
}

#[derive(Debug, Serialize)]
pub struct LedgerMutation {
    pub timestamp: String,
    pub mutation_type: String,
    pub tx_hash: String,
    pub delta_quanta: i64,
    pub running_balance_quanta: u64,
}

#[derive(Debug, Serialize)]
pub struct PassbookLedgerData {
    pub passbook_id: Option<String>,
    pub account_number: Option<String>,
    pub public_key: Option<String>,
    pub balance_scy: f64,
    pub balance_quanta: u64,
    pub sync_status: String,
    pub node_url: String,
    pub block_height: Option<u64>,
    pub utxos: Vec<PassbookUtxo>,
    pub ledger: Vec<LedgerMutation>,
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
        .and_then(|value| {
            value
                .get("active_wallet_path")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
}

fn wallet_summary(path: PathBuf, active_path: &Option<String>) -> Option<WalletSummary> {
    let value: Value = serde_json::from_str(&fs::read_to_string(&path).ok()?).ok()?;
    let account_number = value
        .get("account_number")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let path_string = path.to_string_lossy().into_owned();
    Some(WalletSummary {
        account_number,
        active: active_path.as_deref() == Some(path_string.as_str()),
        path: path_string,
    })
}

fn configured_node_url() -> String {
    let path =
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".scytale/config.json");
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| {
            value
                .get("node_url")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "http://127.0.0.1:8332".to_string())
}

fn json_u64(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_u64))
}

fn json_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str).map(str::to_owned))
}

fn json_hash(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(v) = value.get(*key) {
            if let Some(s) = v.as_str() {
                return Some(s.to_string());
            }
            if let Some(arr) = v.as_array() {
                let bytes: Vec<u8> = arr
                    .iter()
                    .filter_map(|x| x.as_u64().map(|n| n as u8))
                    .collect();
                if bytes.len() == 32 {
                    return Some(hex::encode(bytes));
                }
            }
        }
    }
    None
}

fn json_array<'a>(value: &'a Value, keys: &[&str]) -> &'a [Value] {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_array).map(Vec::as_slice))
        .unwrap_or(&[])
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
    PinCode::new(&pin).map_err(|error| error.to_string())?;

    let mut seed = [0u8; 32];
    use rand::RngCore;
    rand::rngs::OsRng.fill_bytes(&mut seed);

    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();
    let pubkey_bytes = verifying_key.to_bytes();
    let address_bytes = *blake3::hash(&pubkey_bytes).as_bytes();
    let bech32_addr = Address::new(address_bytes)
        .to_bech32()
        .map_err(|e| e.to_string())?;

    let account_num = scytale_account::derive_candidate(&pubkey_bytes, &bech32_addr, 0).to_string();
    let envelope = encrypt_key(&seed, &pin).map_err(|e| e.to_string())?;

    let wallet_data = serde_json::json!({
        "version": 1,
        "public_key": hex::encode(&pubkey_bytes),
        "address": bech32_addr,
        "p2pkh_address": bech32_addr,
        "account_number": account_num,
        "passbook_id": bech32_addr,
        "encrypted_key": envelope,
    });

    let wallet_dir = home_path(".scytale");
    fs::create_dir_all(&wallet_dir).map_err(|e| e.to_string())?;

    let wallet_file = wallet_dir.join("wallet.json");
    let target_file = if wallet_file.exists() {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        wallet_dir.join(format!("wallet_{ts}.json"))
    } else {
        wallet_file
    };

    fs::write(
        &target_file,
        serde_json::to_vec_pretty(&wallet_data).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let path_str = target_file.to_string_lossy().into_owned();
    let _ = set_active_wallet(path_str);

    Ok(format!("Wallet created successfully: {bech32_addr} ({account_num})"))
}

#[tauri::command]
pub fn get_system_status() -> SystemStatus {
    let node_url = configured_node_url();
    SystemStatus {
        network: "Configured Node".to_string(),
        node_url,
        connected: false,
    }
}

#[tauri::command]
pub async fn get_node_telemetry(node_url: Option<String>) -> Result<NodeStatus, String> {
    let node_url = node_url
        .unwrap_or_else(configured_node_url)
        .trim_end_matches('/')
        .to_string();
    let started = Instant::now();
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();
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
pub async fn get_passbook_ledger(
    wallet_path: Option<String>,
    node_url: Option<String>,
) -> Result<PassbookLedgerData, String> {
    let wallet_path = wallet_path
        .map(PathBuf::from)
        .or_else(|| active_wallet_path().map(PathBuf::from))
        .ok_or_else(|| "No active wallet configured".to_string())?;
    let wallet: Value =
        serde_json::from_str(&fs::read_to_string(wallet_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let passbook_id = json_string(&wallet, &["passbook_id", "address", "p2pkh_address"]);
    let account_number = json_string(&wallet, &["account_number"]);
    let public_key = json_string(&wallet, &["public_key", "publicKey"]);
    let node_url = node_url
        .unwrap_or_else(configured_node_url)
        .trim_end_matches('/')
        .to_string();

    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    // Determine target Bech32 address
    let address = if let Some(ref pid) = passbook_id {
        if pid.starts_with("scy1") {
            pid.clone()
        } else if let Some(ref acc) = account_number {
            let resolve_res: Result<Value, _> = agent
                .get(&format!("{node_url}/api/v1/alias/resolve/{acc}"))
                .call()
                .map_err(|e| e.to_string())?
                .into_json();
            resolve_res
                .ok()
                .and_then(|v| json_string(&v, &["address", "resolved_address"]))
                .unwrap_or_else(|| pid.clone())
        } else {
            pid.clone()
        }
    } else if let Some(ref acc) = account_number {
        let resolve_res: Result<Value, _> = agent
            .get(&format!("{node_url}/api/v1/alias/resolve/{acc}"))
            .call()
            .map_err(|e| e.to_string())?
            .into_json();
        resolve_res
            .ok()
            .and_then(|v| json_string(&v, &["address", "resolved_address"]))
            .ok_or_else(|| "Could not resolve account number to address".to_string())?
    } else {
        return Err("Wallet has no account identity or address".to_string());
    };

    // 1. Query canonical Passbook view from node
    let passbook_res: Value = agent
        .get(&format!("{node_url}/api/v1/passbook?address={address}"))
        .call()
        .map_err(|error| format!("Passbook query failed: {error}"))?
        .into_json()
        .map_err(|error| error.to_string())?;

    // 2. Query node status to get canonical tip height
    let status_res: Option<Value> = agent
        .get(&format!("{node_url}/api/v1/status"))
        .call()
        .ok()
        .and_then(|r| r.into_json().ok());
    let canonical_height = status_res
        .as_ref()
        .and_then(|v| json_u64(v, &["canonical_height", "block_height", "height"]));

    // 3. Query unspent transaction outputs (UTXOs)
    let utxos_res: Vec<Value> = agent
        .get(&format!("{node_url}/api/v1/utxos/{address}"))
        .call()
        .ok()
        .and_then(|r| r.into_json().ok())
        .unwrap_or_default();

    let utxos: Vec<PassbookUtxo> = utxos_res
        .iter()
        .map(|item| {
            let bh = json_u64(item, &["block_height"]).unwrap_or(0);
            let confirmations = canonical_height
                .map(|tip| tip.saturating_sub(bh).saturating_add(1))
                .unwrap_or(1);
            PassbookUtxo {
                tx_id: json_string(item, &["txid_hex", "txid", "tx_id", "hash"]).unwrap_or_default(),
                output_index: json_u64(item, &["index", "output_index", "vout"]).unwrap_or(0),
                amount_quanta: json_u64(item, &["value_quanta", "amount_quanta", "value"]).unwrap_or(0),
                confirmations,
            }
        })
        .collect();

    // 4. Parse Ledger from Passbook entries
    let balance_quanta = json_u64(
        &passbook_res,
        &["confirmed_native_balance_quanta", "balance_quanta", "balance"],
    )
    .unwrap_or(0);

    let entries = json_array(&passbook_res, &["entries", "ledger", "transactions"]);
    let mut running = 0u64;
    let mut ledger = Vec::new();
    for item in entries {
        let action = json_string(item, &["action", "mutation_type", "type"])
            .unwrap_or_else(|| "Inbound".to_string());
        let amount = json_u64(item, &["amount_quanta", "amount", "value"]).unwrap_or(0);
        let is_outgoing = action == "Sent" || action == "Scy20Burn";
        let delta: i64 = if is_outgoing {
            -(amount as i64)
        } else {
            amount as i64
        };

        if delta.is_negative() {
            running = running.saturating_sub(delta.unsigned_abs());
        } else {
            running = running.saturating_add(delta as u64);
        }

        let ts_val = item.get("timestamp");
        let ts_str = if let Some(n) = ts_val.and_then(Value::as_u64) {
            if n == 0 {
                "Genesis".to_string()
            } else {
                chrono::DateTime::from_timestamp(n as i64, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_else(|| n.to_string())
            }
        } else if let Some(s) = ts_val.and_then(Value::as_str) {
            s.to_string()
        } else {
            "—".to_string()
        };

        let tx_hash = json_hash(item, &["txid", "tx_hash", "txid_hex", "hash"]).unwrap_or_default();

        ledger.push(LedgerMutation {
            timestamp: ts_str,
            mutation_type: action,
            tx_hash,
            delta_quanta: delta,
            running_balance_quanta: running,
        });
    }

    Ok(PassbookLedgerData {
        passbook_id: Some(address),
        account_number,
        public_key,
        balance_scy: balance_quanta as f64 / 100_000_000.0,
        balance_quanta,
        sync_status: "Synced with node".to_string(),
        node_url,
        block_height: canonical_height,
        utxos,
        ledger,
    })
}

#[tauri::command]
pub fn get_active_account_details(wallet_path: Option<String>) -> Result<AccountDetails, String> {
    let path = wallet_path
        .map(PathBuf::from)
        .or_else(|| active_wallet_path().map(PathBuf::from))
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".scytale/wallet.json")
        });
    let value: Value =
        serde_json::from_str(&fs::read_to_string(path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let account_number = value
        .get("account_number")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let passbook_id = value
        .get("passbook_id")
        .or_else(|| value.get("address"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let balance_quanta = json_u64(
        &value,
        &["balance_quanta", "balance", "confirmed_balance_quanta"],
    );
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
    let roots = vec![home_path(".scytale")];
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
    fs::write(
        config_file,
        serde_json::to_vec_pretty(&config).map_err(|error| error.to_string())?,
    )
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
    let wallet: WalletSecrets = serde_json::from_str(
        &fs::read_to_string(&wallet_path).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let public_key = hex::decode(&wallet.public_key).map_err(|error| error.to_string())?;
    let public_key: [u8; 32] = public_key
        .try_into()
        .map_err(|_| "Wallet public key must be 32 bytes".to_string())?;
    let envelope = wallet
        .encrypted_key
        .ok_or_else(|| "Wallet must contain an encrypted private key".to_string())?;
    let private_key = decrypt_key(&envelope, &pin).map_err(|error| error.to_string())?;
    let seed: [u8; 32] = private_key
        .as_slice()
        .try_into()
        .map_err(|_| "Decrypted wallet key must be 32 bytes".to_string())?;
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);
    if signing_key.verifying_key().to_bytes() != public_key {
        return Err("Wallet public key does not match encrypted private key".to_string());
    }
    let recipient = draft
        .get("outputs")
        .and_then(Value::as_array)
        .and_then(|outputs| outputs.first())
        .and_then(|output| output.get("recipient"))
        .and_then(Value::as_str)
        .ok_or_else(|| "Transaction draft must contain an output recipient".to_string())?;
    let amount = draft
        .get("outputs")
        .and_then(Value::as_array)
        .and_then(|outputs| outputs.first())
        .and_then(|output| output.get("amount_quanta"))
        .and_then(Value::as_u64)
        .ok_or_else(|| "Transaction draft must contain output amount_quanta".to_string())?;
    let fee = draft
        .get("fee_quanta")
        .and_then(Value::as_u64)
        .ok_or_else(|| "Transaction draft must contain fee_quanta".to_string())?;
    let sender = scytale_core::Address::parse(&wallet.address).map_err(|e| e.to_string())?;
    let recipient = scytale_core::Address::parse(recipient).map_err(|e| e.to_string())?;
    let sender_lock = p2pkh_lock(sender.hash());
    let recipient_lock = p2pkh_lock(recipient.hash());
    let endpoint = node_url
        .unwrap_or_else(configured_node_url)
        .trim_end_matches('/')
        .to_string();
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(10))
        .build();
    let response = agent
        .get(&format!("{endpoint}/api/v1/utxos/{}", hex::encode(&sender_lock)))
        .call()
        .map_err(|error| format!("UTXO query failed: {error}"))?;
    let utxos: Vec<UtxoResponse> = response.into_json().map_err(|error| error.to_string())?;
    let needed = amount.checked_add(fee).ok_or("Amount plus fee overflow")?;
    let mut selected = Vec::new();
    let mut total = 0u64;
    for utxo in utxos {
        total = total.checked_add(utxo.value_quanta).ok_or("Input overflow")?;
        selected.push(utxo);
        if total >= needed { break; }
    }
    if total < needed { return Err("Insufficient funds".to_string()); }
    let inputs = selected.iter().map(|utxo| {
        let txid = utxo.txid_hex.parse::<Hash256>().map_err(|e| e.to_string())?;
        Ok(TxIn::new(OutPoint::new(txid, utxo.index), Vec::new()))
    }).collect::<Result<Vec<_>, String>>()?;
    let mut outputs = vec![TxOut::new(amount, recipient_lock)];
    if total > needed { outputs.push(TxOut::new(total - needed, sender_lock.clone())); }
    let mut tx = Transaction::new(1, inputs, outputs, 0);
    for index in 0..tx.inputs.len() {
        let sighash = tx.compute_sighash(index, &sender_lock);
        let signature = signing_key.sign(&sighash);
        tx.inputs[index].authorization = p2pkh_unlock(&signature.to_bytes(), &public_key);
    }
    let payload = serde_json::json!({ "tx_hex": hex::encode(tx.to_canonical_bytes().map_err(|e| e.to_string())?) });
    let response = agent.post(&format!("{endpoint}/api/v1/tx")).send_json(payload);
    match response {
        Ok(response) => {
            let body: Value = response.into_json().map_err(|error| error.to_string())?;
            Ok(serde_json::json!({ "broadcast": true, "response": body }))
        }
        Err(error) => Err(format!("Broadcast failed: {error}")),
    }
}

#[derive(serde::Deserialize)]
struct WalletSecrets {
    public_key: String,
    address: String,
    encrypted_key: Option<EncryptedKeyEnvelope>,
}

#[derive(serde::Deserialize)]
struct UtxoResponse {
    txid_hex: String,
    index: u32,
    value_quanta: u64,
}

fn p2pkh_lock(address: &[u8; 32]) -> Vec<u8> {
    let mut script = vec![0x73, 0xa0, 0x20];
    script.extend_from_slice(address);
    script.extend_from_slice(&[0x88, 0xac]);
    script
}

fn p2pkh_unlock(signature: &[u8; 64], public_key: &[u8; 32]) -> Vec<u8> {
    let mut script = vec![0x40];
    script.extend_from_slice(signature);
    script.push(0x20);
    script.extend_from_slice(public_key);
    script
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_p2pkh_lock() {
        let addr = [0xaa; 32];
        let script = p2pkh_lock(&addr);
        assert_eq!(script.len(), 37);
        assert_eq!(script[0], 0x73); // OpDup
        assert_eq!(script[1], 0xa0); // OpBlake3
        assert_eq!(script[2], 0x20); // 32-byte pushdata
        assert_eq!(&script[3..35], &addr);
        assert_eq!(script[35], 0x88); // OpEqualVerify
        assert_eq!(script[36], 0xac); // OpCheckSig
    }

    #[test]
    fn test_p2pkh_unlock_structure() {
        let sig = [0xbb; 64];
        let pk = [0xcc; 32];
        let script = p2pkh_unlock(&sig, &pk);
        assert_eq!(script.len(), 1 + 64 + 1 + 32);
        assert_eq!(script[0], 0x40);
        assert_eq!(&script[1..65], &sig);
        assert_eq!(script[65], 0x20);
        assert_eq!(&script[66..98], &pk);
    }
}
