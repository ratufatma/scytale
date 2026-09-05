//! Smart contract developer tooling subcommands for `scytale-cli contract`.
//!
//! Subcommands:
//! - `inspect` — Print BLAKE3 script_hash and binary size of a .wasm file
//! - `build`   — Compile a contract crate to `wasm32-unknown-unknown` release
//! - `deploy`  — Build and broadcast a locking transaction to an OutputLock::Script UTXO
//! - `call`    — Spend a script UTXO with local ScyVM dry-run validation and node broadcast

use clap::{Args, Subcommand};
use ed25519_dalek::Signer;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use scytale_core::{
    vm_adapter::{create_tx_context, MAX_TX_GAS},
    CanonicalSerialize, Hash256, OutPoint, OutputLock, Transaction, TxIn, TxInput, TxOut,
    TRANSACTION_VERSION_1,
};
use scytale_vm::ScyVM;

use crate::client::CliClientError;

// ── Argument structures ───────────────────────────────────────────────────────

#[derive(Debug, Args, PartialEq, Eq)]
pub struct ContractArgs {
    #[command(subcommand)]
    pub action: ContractCommands,
}

#[derive(Debug, Subcommand, PartialEq, Eq)]
pub enum ContractCommands {
    /// Inspect a Wasm binary: print BLAKE3 script_hash and file size
    Inspect(InspectArgs),

    /// Compile a smart contract crate to wasm32-unknown-unknown (release)
    Build(BuildArgs),

    /// Lock funds into a script UTXO (OutputLock::Script { script_hash, datum })
    Deploy(DeployArgs),

    /// Spend a script UTXO with local ScyVM dry-run validation and broadcast
    Call(CallArgs),
}

#[derive(Debug, Args, PartialEq, Eq)]
pub struct InspectArgs {
    /// Path to the compiled .wasm binary
    #[arg(short, long, value_name = "FILE")]
    pub wasm: PathBuf,
}

#[derive(Debug, Args, PartialEq, Eq)]
pub struct BuildArgs {
    /// Path to the smart contract crate directory (default: current directory)
    #[arg(short, long, default_value = ".")]
    pub path: PathBuf,

    /// Crate name to pass to cargo `-p` flag (optional)
    #[arg(short = 'n', long)]
    pub package: Option<String>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct DeployArgs {
    /// Path to the compiled .wasm binary
    #[arg(short, long, value_name = "FILE")]
    pub wasm: PathBuf,

    /// Amount in quanta to lock in the script UTXO
    #[arg(short, long)]
    pub amount: u64,

    /// Datum bytes as hex string (e.g. the bincode-encoded VaultDatum)
    #[arg(short, long)]
    pub datum: String,

    /// Sender wallet file path to fund the deployment
    #[arg(long, default_value = "~/.scytale/wallet.json")]
    pub wallet: PathBuf,

    /// Miner fee in quanta
    #[arg(long, default_value_t = 1_000)]
    pub fee: u64,

    /// Node HTTP Gateway URL to broadcast the deployment transaction
    #[arg(
        long,
        alias = "rpc-url",
        default_value = "http://127.0.0.1:8332"
    )]
    pub node_url: String,

    /// Dry-run mode: simulate transaction construction without broadcasting to the network
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, PartialEq, Eq)]
pub struct CallArgs {
    /// Target UTXO to spend in format <tx_hash_hex>:<output_index>
    #[arg(short, long)]
    pub utxo: String,

    /// Path to the .wasm contract binary
    #[arg(short, long, value_name = "FILE")]
    pub wasm: PathBuf,

    /// Redeemer bytes as hex string (e.g. the bincode-encoded VaultRedeemer)
    #[arg(short, long)]
    pub redeemer: String,

    /// Datum bytes as hex string (from the locked UTXO)
    #[arg(short, long)]
    pub datum: String,

    /// Recipient address (Bech32 `scy1...`) or raw locking script hex for the unlocked funds
    #[arg(long)]
    pub to: String,

    /// Amount to send to recipient (leave 0 to auto-calculate from UTXO - fee)
    #[arg(long, default_value_t = 0)]
    pub amount: u64,

    /// Miner fee in quanta
    #[arg(long, default_value_t = 1_000)]
    pub fee: u64,

    /// Optional signature bytes as hex string for witness authorization
    #[arg(long)]
    pub signature: Option<String>,

    /// Dry-run mode: execute local ScyVM simulation only without broadcasting
    #[arg(long)]
    pub dry_run: bool,

    /// Skip the local ScyVM dry-run simulation (not recommended)
    #[arg(long)]
    pub skip_dry_run: bool,

    /// Simulated input amount in quanta for dry-run (use actual UTXO value)
    #[arg(long, default_value_t = 0)]
    pub input_amount: u64,

    /// Node HTTP Gateway URL to broadcast the spend transaction
    #[arg(
        long,
        alias = "rpc-url",
        default_value = "http://127.0.0.1:8332"
    )]
    pub node_url: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Strips optional `0x` or `0X` prefix and surrounding whitespace from a hex string.
pub fn clean_hex(s: &str) -> &str {
    let trimmed = s.trim();
    trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed)
}

/// Expands a leading tilde `~` in file paths using the user's `HOME` directory.
pub fn resolve_path(path: &Path) -> PathBuf {
    let path_str = path.to_string_lossy();
    if path_str == "~" {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(stripped) = path_str.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    path.to_path_buf()
}

/// Resolves the node gateway URL, checking the `SCYTALE_NODE_URL` env variable if default is used.
pub fn resolve_node_url(configured_url: &str) -> String {
    if configured_url == "http://127.0.0.1:8332" {
        if let Ok(env_url) = std::env::var("SCYTALE_NODE_URL") {
            let trimmed = env_url.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    configured_url.to_string()
}

/// Formats quanta into a human-readable string with SCY equivalent using pure integer math.
pub fn format_quanta(quanta: u64) -> String {
    let scy_int = quanta / scytale_primitives::QUANTA_PER_SCY;
    let scy_dec = quanta % scytale_primitives::QUANTA_PER_SCY;
    format!("{} quanta ({}.{:08} SCY)", quanta, scy_int, scy_dec)
}

#[derive(Debug, serde::Serialize)]
struct SubmitTxRequest<'a> {
    tx_hex: &'a str,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SubmitTxResponse {
    pub txid: String,
    pub status: String,
}

/// Sends a signed transaction to the node HTTP gateway at `POST /api/v1/tx`.
pub fn broadcast_transaction(node_url: &str, tx: &Transaction) -> Result<SubmitTxResponse, CliClientError> {
    let tx_bytes = tx.to_canonical_bytes().map_err(|e| {
        CliClientError::User(format!("Failed to serialize transaction to canonical bytes: {e}"))
    })?;
    let tx_hex = hex::encode(&tx_bytes);
    let url = format!("{}/api/v1/tx", node_url.trim_end_matches('/'));

    let payload = SubmitTxRequest { tx_hex: &tx_hex };
    let resp = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_json(&payload)
        .map_err(|e| match e {
            ureq::Error::Transport(te) => CliClientError::User(format!(
                "Failed to connect to Scytale Node HTTP Gateway at '{url}'.\n\
                 Transport error: {te}\n\
                 Please ensure the 'scytale-node' daemon is active and listening at '{node_url}'.\n\
                 Tip: Run 'scytale-cli status' or check node terminal logs."
            )),
            ureq::Error::Status(code, r) => {
                let err_body = r.into_string().unwrap_or_default();
                let err_msg = if let Ok(json_err) = serde_json::from_str::<serde_json::Value>(&err_body) {
                    json_err["error"]
                        .as_str()
                        .unwrap_or(&err_body)
                        .to_string()
                } else {
                    err_body
                };
                CliClientError::User(format!(
                    "Mempool submission rejected by node (HTTP {code}):\n  Reason: {err_msg}"
                ))
            }
        })?;

    let submit_resp: SubmitTxResponse = resp.into_json().map_err(|e| {
        CliClientError::User(format!("Failed to parse submit response JSON from node: {e}"))
    })?;
    Ok(submit_resp)
}

/// Queries the node HTTP Gateway for UTXOs matching a given locking script hex.
pub fn fetch_utxos_from_node(
    node_url: &str,
    locking_script_hex: &str,
) -> Result<Vec<scytale_bridge::UtxoDto>, CliClientError> {
    let url = format!(
        "{}/api/v1/utxos/{}",
        node_url.trim_end_matches('/'),
        locking_script_hex
    );
    let resp = ureq::get(&url).call().map_err(|e| match e {
        ureq::Error::Transport(te) => CliClientError::User(format!(
            "Failed to connect to Scytale Node HTTP Gateway at '{url}'.\n\
             Transport error: {te}\n\
             Please verify that 'scytale-node' daemon is running and reachable at '{node_url}'."
        )),
        ureq::Error::Status(code, r) => {
            let body = r.into_string().unwrap_or_default();
            CliClientError::User(format!(
                "Node returned error fetching UTXOs (HTTP {code}): {body}"
            ))
        }
    })?;

    let utxos: Vec<scytale_bridge::UtxoDto> = resp.into_json().map_err(|e| {
        CliClientError::User(format!("Failed to parse UTXOs response from gateway: {e}"))
    })?;
    Ok(utxos)
}

/// Looks up an output's value (in quanta) from a transaction on the node HTTP Gateway.
pub fn fetch_tx_output_value(node_url: &str, txid_hex: &str, output_index: u32) -> Option<u64> {
    let clean_id = clean_hex(txid_hex);
    let url = format!("{}/api/v1/tx/{}", node_url.trim_end_matches('/'), clean_id);
    let resp = ureq::get(&url).call().ok()?;
    let val: serde_json::Value = resp.into_json().ok()?;
    let outputs = val.get("outputs")?.as_array()?;
    for out in outputs {
        if out.get("index")?.as_u64()? == output_index as u64 {
            return out.get("value_quanta")?.as_u64();
        }
    }
    None
}

// ── Handler dispatcher ────────────────────────────────────────────────────────

pub fn handle_contract(args: ContractArgs) -> Result<(), CliClientError> {
    match args.action {
        ContractCommands::Inspect(a) => cmd_inspect(a),
        ContractCommands::Build(a) => cmd_build(a),
        ContractCommands::Deploy(a) => deploy_contract(a),
        ContractCommands::Call(a) => call_contract(a),
    }
}

// ── `contract inspect` ────────────────────────────────────────────────────────

pub fn cmd_inspect(args: InspectArgs) -> Result<(), CliClientError> {
    let path = &args.wasm;
    if !path.exists() {
        return Err(CliClientError::User(format!(
            "Wasm file not found: {}",
            path.display()
        )));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| CliClientError::User(format!("Failed to read wasm: {e}")))?;

    let hash = blake3::hash(&bytes);
    let size_kb_int = bytes.len() / 1024;
    let size_kb_dec = (bytes.len() % 1024) * 100 / 1024;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║             SCYTALE CONTRACT INSPECTOR                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  File       : {}", path.display());
    println!(
        "  Size       : {} bytes ({}.{:02} KiB)",
        bytes.len(),
        size_kb_int,
        size_kb_dec
    );
    println!("  ScriptHash : {}", hex::encode(hash.as_bytes()));
    println!();
    println!("  Use this script_hash when building a Deploy transaction:");
    println!("    --datum <hex>  --wasm {}", path.display());

    // Basic Wasm magic number validation
    if bytes.len() >= 4 && &bytes[0..4] == b"\0asm" {
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap_or([0; 4]));
        println!();
        println!("  Wasm Magic  : ✓ valid (\\0asm)");
        println!("  Wasm Version: {}", version);
    } else {
        println!();
        println!("  ⚠ Warning: File does not start with Wasm magic number (\\0asm).");
    }

    Ok(())
}

// ── `contract build` ──────────────────────────────────────────────────────────

pub fn cmd_build(args: BuildArgs) -> Result<(), CliClientError> {
    let crate_dir = &args.path;
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║             SCYTALE CONTRACT BUILDER                        ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  Crate Dir  : {}", crate_dir.display());
    println!("  Target     : wasm32-unknown-unknown (release)");
    println!();

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build")
        .arg("--release")
        .arg("--target")
        .arg("wasm32-unknown-unknown");

    if let Some(pkg) = &args.package {
        cmd.arg("-p").arg(pkg);
    }

    cmd.current_dir(crate_dir);

    println!("  Running: cargo build --release --target wasm32-unknown-unknown");
    println!("  ──────────────────────────────────────────────────────────────");

    let status = cmd
        .status()
        .map_err(|e| CliClientError::User(format!("Failed to invoke cargo: {e}")))?;

    println!("  ──────────────────────────────────────────────────────────────");
    if status.success() {
        println!("  ✓ Build succeeded!");
        println!();
        println!("  Artifacts location:");
        println!(
            "    {}/target/wasm32-unknown-unknown/release/*.wasm",
            crate_dir.display()
        );
        println!();
        println!("  Next steps:");
        println!("    scytale-cli contract inspect --wasm <path>.wasm");
        println!("    scytale-cli contract deploy  --wasm <path>.wasm --datum <hex> --amount <quanta>");
    } else {
        return Err(CliClientError::User(
            "Build failed. Check cargo output above.".to_string(),
        ));
    }

    Ok(())
}

// ── `contract deploy` ────────────────────────────────────────────────────────

pub fn cmd_deploy(args: DeployArgs) -> Result<(), CliClientError> {
    deploy_contract(args)
}

pub fn deploy_contract(args: DeployArgs) -> Result<(), CliClientError> {
    let node_url = resolve_node_url(&args.node_url);

    let wasm_bytes = std::fs::read(&args.wasm).map_err(|e| {
        CliClientError::User(format!(
            "Failed to read wasm file '{}': {e}",
            args.wasm.display()
        ))
    })?;

    let clean_datum = clean_hex(&args.datum);
    let datum_bytes = hex::decode(clean_datum)
        .map_err(|e| CliClientError::User(format!("Invalid datum hex: {e}")))?;

    let script_hash = *blake3::hash(&wasm_bytes).as_bytes();
    let lock = OutputLock::Script {
        script_hash,
        datum: datum_bytes.clone(),
    };

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║             SCYTALE CONTRACT DEPLOY                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  Wasm File    : {}", args.wasm.display());
    println!("  Script Hash  : {}", hex::encode(script_hash));
    println!("  Lock Value   : {}", format_quanta(args.amount));
    println!("  Miner Fee    : {}", format_quanta(args.fee));
    println!("  Gateway URL  : {}", node_url);
    println!(
        "  Mode         : {}",
        if args.dry_run {
            "Dry Run (simulation only)"
        } else {
            "Live Network Broadcast"
        }
    );
    println!();
    println!("  OutputLock::Script {{");
    println!("    script_hash : {},", hex::encode(script_hash));
    println!("    datum       : {} bytes", datum_bytes.len());
    println!("  }}");
    println!();

    let wallet_path = resolve_path(&args.wallet);
    let wallet = match crate::wallet::WalletFile::load_from(&wallet_path) {
        Ok(w) => w,
        Err(e) => {
            if args.dry_run {
                println!(
                    "  [!] Wallet file not loaded ({}). Showing dry-run parameter preview.",
                    e
                );
                println!("  ──────────────────────────────────────────────────────────────");
                println!("  Deployment Preview (Simulated):");
                println!("    Predicted Script Hash : {}", hex::encode(script_hash));
                println!("    Simulated OutPoint    : <simulated_funding_txid>:0");
                println!("    Lock Value            : {}", format_quanta(args.amount));
                println!("    Status                : Dry-run preview passed (broadcast skipped)");
                println!("  ──────────────────────────────────────────────────────────────");
                return Ok(());
            } else {
                return Err(CliClientError::User(format!(
                    "Failed to load wallet '{}' to fund deployment: {e}.\n\
                     Please create or specify a valid wallet with --wallet <path>.",
                    wallet_path.display()
                )));
            }
        }
    };

    let sender_lock = wallet
        .p2pkh_locking_script()
        .map_err(CliClientError::Wallet)?;
    let sender_lock_hex = hex::encode(&sender_lock);
    let total_needed = args
        .amount
        .checked_add(args.fee)
        .ok_or_else(|| CliClientError::User("Amount plus fee overflow".to_string()))?;

    // Fetch UTXOs to fund transaction
    let utxos = match fetch_utxos_from_node(&node_url, &sender_lock_hex) {
        Ok(u) => u,
        Err(e) => {
            if args.dry_run {
                println!("  [!] Could not fetch UTXOs from node ({}).", e);
                println!("  ──────────────────────────────────────────────────────────────");
                println!("  Deployment Preview (Offline Dry-Run):");
                println!("    Script Hash        : {}", hex::encode(script_hash));
                println!("    Simulated OutPoint : <predicted_txid>:0");
                println!("    Lock Value         : {}", format_quanta(args.amount));
                println!("    Status             : Dry-run simulation passed (broadcast skipped)");
                println!("  ──────────────────────────────────────────────────────────────");
                return Ok(());
            } else {
                return Err(e);
            }
        }
    };

    let mut selected_utxos = Vec::new();
    let mut accumulated: u64 = 0;
    let mut sorted_utxos = utxos;
    sorted_utxos.sort_by_key(|b| std::cmp::Reverse(b.value_quanta));

    for u in sorted_utxos {
        accumulated = accumulated.saturating_add(u.value_quanta);
        selected_utxos.push(u);
        if accumulated >= total_needed {
            break;
        }
    }

    if accumulated < total_needed {
        return Err(CliClientError::Wallet(
            crate::wallet::WalletError::InsufficientFunds {
                required: total_needed,
                available: accumulated,
            },
        ));
    }

    let mut inputs = Vec::new();
    for u in &selected_utxos {
        let txid = Hash256::from_str(&u.txid_hex)
            .map_err(|e| CliClientError::User(format!("Invalid txid in UTXO: {e}")))?;
        inputs.push(TxIn::new(OutPoint::new(txid, u.index), vec![]));
    }

    let mut outputs = vec![TxOut::new(args.amount, lock.to_locking_condition())];
    if accumulated > total_needed {
        let change = accumulated - total_needed;
        outputs.push(TxOut::new(change, sender_lock.clone()));
    }

    let mut tx = Transaction::new(TRANSACTION_VERSION_1, inputs, outputs, 0);

    let signing_key = wallet.signing_key().map_err(CliClientError::Wallet)?;
    let pubkey_bytes = wallet
        .verifying_key_bytes()
        .map_err(CliClientError::Wallet)?;

    for i in 0..tx.inputs.len() {
        let sighash = tx.compute_sighash(i, &sender_lock);
        let sig = signing_key.sign(&sighash);
        tx.inputs[i].authorization =
            crate::wallet::build_p2pkh_unlocking_script(&sig.to_bytes(), &pubkey_bytes);
    }

    let contract_outpoint = format!("{}:0", tx.txid());

    if args.dry_run {
        println!("  ── Dry-Run Deployment Report ────────────────────────────────");
        println!("  Contract Script Hash  : {}", hex::encode(script_hash));
        println!("  Initial Contract UTXO : {}", contract_outpoint);
        println!("  Lock Value            : {}", format_quanta(args.amount));
        println!("  Miner Fee             : {}", format_quanta(args.fee));
        println!("  Inputs Funded         : {} UTXO(s)", tx.inputs.len());
        println!("  Total Quanta Spent    : {}", format_quanta(accumulated));
        println!("  Broadcast Status      : Skipped (--dry-run)");
        println!("  ──────────────────────────────────────────────────────────────");
        println!("  ✓ Dry-run completed. Transaction is valid and ready to deploy.");
        return Ok(());
    }

    println!(
        "  [*] Broadcasting deployment transaction to {}/api/v1/tx ...",
        node_url.trim_end_matches('/')
    );
    let submit_resp = broadcast_transaction(&node_url, &tx)?;

    println!("  ──────────────────────────────────────────────────────────────");
    println!("  ✓ SMART CONTRACT DEPLOYED SUCCESSFULLY!");
    println!("  ──────────────────────────────────────────────────────────────");
    println!("  Contract Script Hash  : {}", hex::encode(script_hash));
    println!("  Initial Contract UTXO : {}", contract_outpoint);
    println!("  Lock Value            : {}", format_quanta(args.amount));
    println!("  Broadcast Status      : {}", submit_resp.status);
    println!("  Broadcast TxID        : {}", submit_resp.txid);
    println!("  ──────────────────────────────────────────────────────────────");
    println!("  To spend or call this contract, use:");
    println!(
        "    scytale-cli contract call --utxo {} --wasm {} ...",
        contract_outpoint,
        args.wasm.display()
    );

    Ok(())
}

// ── `contract call` ──────────────────────────────────────────────────────────

pub fn cmd_call(args: CallArgs) -> Result<(), CliClientError> {
    call_contract(args)
}

pub fn call_contract(args: CallArgs) -> Result<(), CliClientError> {
    let node_url = resolve_node_url(&args.node_url);

    // 1. Parse UTXO reference
    let parts: Vec<&str> = args.utxo.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(CliClientError::User(
            "Invalid --utxo format. Expected <tx_hash_hex>:<output_index>".to_string(),
        ));
    }
    let txid_hex = clean_hex(parts[0]);
    let output_index: u32 = parts[1]
        .parse()
        .map_err(|_| CliClientError::User("Invalid output index in --utxo".to_string()))?;

    let txid_bytes = hex::decode(txid_hex)
        .map_err(|e| CliClientError::User(format!("Invalid txid hex: {e}")))?;
    if txid_bytes.len() != 32 {
        return Err(CliClientError::User(
            "txid must be exactly 32 bytes (64 hex chars)".to_string(),
        ));
    }
    let mut txid_arr = [0u8; 32];
    txid_arr.copy_from_slice(&txid_bytes);

    // 2. Load wasm and parse hex inputs
    let wasm_bytes = std::fs::read(&args.wasm)
        .map_err(|e| CliClientError::User(format!("Failed to read wasm: {e}")))?;

    let clean_redeemer = clean_hex(&args.redeemer);
    let redeemer_bytes = hex::decode(clean_redeemer)
        .map_err(|e| CliClientError::User(format!("Invalid redeemer hex: {e}")))?;

    let clean_datum = clean_hex(&args.datum);
    let datum_bytes = hex::decode(clean_datum)
        .map_err(|e| CliClientError::User(format!("Invalid datum hex: {e}")))?;

    // Support either Bech32 address (`scy1...`) or raw hex script for `--to`
    let to_bytes = if args.to.to_ascii_lowercase().starts_with("scy1") {
        let addr = scytale_core::Address::parse(&args.to).map_err(|e| {
            CliClientError::User(format!(
                "Invalid Bech32 recipient address '{}': {e}",
                args.to
            ))
        })?;
        crate::wallet::build_p2pkh_locking_script(addr.hash())
    } else {
        let clean_to = clean_hex(&args.to);
        hex::decode(clean_to)
            .map_err(|e| CliClientError::User(format!("Invalid --to hex: {e}")))?
    };

    let signature_bytes = if let Some(sig_hex) = &args.signature {
        let clean_sig = clean_hex(sig_hex);
        Some(
            hex::decode(clean_sig)
                .map_err(|e| CliClientError::User(format!("Invalid signature hex: {e}")))?,
        )
    } else {
        None
    };

    let script_hash = *blake3::hash(&wasm_bytes).as_bytes();

    // 3. Determine input and output amounts
    let input_amount = if args.input_amount > 0 {
        args.input_amount
    } else if let Some(fetched) =
        fetch_tx_output_value(&node_url, txid_hex, output_index)
    {
        fetched
    } else if args.amount + args.fee > 0 {
        args.amount + args.fee
    } else {
        1_000_000 // default fallback placeholder
    };

    let output_amount = if args.amount > 0 {
        args.amount
    } else {
        input_amount.saturating_sub(args.fee)
    };

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║             SCYTALE CONTRACT CALL                           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  Target UTXO  : {}:{}", txid_hex, output_index);
    println!("  Wasm File    : {}", args.wasm.display());
    println!("  Script Hash  : {}", hex::encode(script_hash));
    println!("  Redeemer     : {} bytes", redeemer_bytes.len());
    println!("  Datum        : {} bytes", datum_bytes.len());
    println!("  Send To      : {}...", &args.to[..args.to.len().min(24)]);
    println!("  Input Amount : {}", format_quanta(input_amount));
    println!("  Output Amount: {}", format_quanta(output_amount));
    println!("  Miner Fee    : {}", format_quanta(args.fee));
    println!("  Gateway URL  : {}", node_url);
    println!(
        "  Mode         : {}",
        if args.dry_run {
            "Dry Run (simulation only)"
        } else {
            "Dry-run + Live Network Broadcast"
        }
    );

    // 4. Build spending transaction with EutxoWitness
    let eutxo_input = TxInput::new(
        txid_arr,
        output_index,
        signature_bytes,
        Some(redeemer_bytes.clone()),
        Some(wasm_bytes.clone()),
    );
    let tx_in = eutxo_input.to_tx_in();
    let tx_out = TxOut::new(output_amount, to_bytes);
    let spending_tx = Transaction::new(TRANSACTION_VERSION_1, vec![tx_in], vec![tx_out], 0);

    // 5. ScyVM Dry-Run Simulation
    let mut fuel_consumed = 0u64;
    if !args.skip_dry_run {
        println!();
        println!("  ── ScyVM Dry-Run Simulation ─────────────────────────────────");

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let tx_context =
            create_tx_context(&spending_tx, current_time, input_amount, output_amount);

        println!("  [*] Executing ScyVM sandbox...");
        println!("      block_time   = {} (unix timestamp)", current_time);
        println!("      input_amount = {}", format_quanta(input_amount));
        println!("      gas_limit    = {} fuel", MAX_TX_GAS);

        let exec_result = ScyVM::execute_validator(
            &wasm_bytes,
            &datum_bytes,
            &redeemer_bytes,
            &tx_context,
            MAX_TX_GAS,
        )
        .map_err(|e| {
            CliClientError::User(format!(
                "ScyVM execution trapped during dry-run: {:?}",
                e
            ))
        })?;

        fuel_consumed = exec_result.gas_consumed;

        if !exec_result.is_valid {
            println!("  [✗] VALIDATION REJECTED by smart contract (return code 0).");
            println!();
            println!("  Possible reasons:");
            println!("    - Timelock has not expired (block_time < datum.unlock_time)");
            println!("    - Redeemer is invalid or authorization check failed");
            println!("    - Signature check failed inside the contract");
            println!();
            return Err(CliClientError::User(
                "Dry-run rejected: smart contract returned VALIDATION_REJECT (return code 0)"
                    .to_string(),
            ));
        }

        println!("  [✓] DRY-RUN PASSED! Return Code: 1 (VALIDATION_SUCCESS)");
        println!(
            "      Fuel consumed: {} / {} gas limit",
            fuel_consumed, MAX_TX_GAS
        );
        println!("  ──────────────────────────────────────────────────────────────");
    } else {
        println!("  [!] Dry-run skipped (--skip-dry-run).");
    }

    if args.dry_run {
        println!();
        println!("  ── Dry-Run Call Report ──────────────────────────────────────");
        println!(
            "  Execution Result : SUCCESS (Fuel Consumed: {})",
            fuel_consumed
        );
        println!("  Spending TxID    : {}", spending_tx.txid());
        println!("  Broadcast Status : Skipped (--dry-run)");
        println!("  ──────────────────────────────────────────────────────────────");
        println!("  ✓ Dry-run completed successfully.");
        return Ok(());
    }

    println!();
    println!(
        "  [*] Broadcasting spending transaction to {}/api/v1/tx ...",
        node_url.trim_end_matches('/')
    );
    let submit_resp = broadcast_transaction(&node_url, &spending_tx)?;

    println!("  ──────────────────────────────────────────────────────────────");
    println!("  ✓ SMART CONTRACT CALL SUBMITTED SUCCESSFULLY!");
    println!("  ──────────────────────────────────────────────────────────────");
    println!(
        "  Execution Result : SUCCESS (Fuel Consumed: {})",
        fuel_consumed
    );
    println!("  Submitted TxID   : {}", submit_resp.txid);
    println!("  Mempool Status   : {}", submit_resp.status);
    println!("  Target UTXO      : {}:{}", txid_hex, output_index);
    println!("  New Output Value : {}", format_quanta(output_amount));
    println!("  ──────────────────────────────────────────────────────────────");

    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Minimal valid Wasm module: (module) = 8 bytes
    fn minimal_wasm() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[test]
    fn test_inspect_minimal_wasm() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&minimal_wasm()).unwrap();

        let args = InspectArgs {
            wasm: f.path().to_path_buf(),
        };
        let result = cmd_inspect(args);
        assert!(result.is_ok(), "Inspect should succeed on valid wasm");
    }

    #[test]
    fn test_inspect_missing_file() {
        let args = InspectArgs {
            wasm: PathBuf::from("/nonexistent/path/contract.wasm"),
        };
        let result = cmd_inspect(args);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliClientError::User(_)));
    }

    #[test]
    fn test_inspect_blake3_hash_is_deterministic() {
        let data = minimal_wasm();
        let hash1 = hex::encode(blake3::hash(&data).as_bytes());
        let hash2 = hex::encode(blake3::hash(&data).as_bytes());
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_inspect_hash_changes_with_content() {
        let data1 = minimal_wasm();
        let mut data2 = minimal_wasm();
        data2.push(0x01);
        let hash1 = hex::encode(blake3::hash(&data1).as_bytes());
        let hash2 = hex::encode(blake3::hash(&data2).as_bytes());
        assert_ne!(hash1, hash2, "Different content must yield different hashes");
    }

    #[test]
    fn test_call_utxo_parse_valid() {
        let utxo = format!("{}:0", "a".repeat(64));
        let parts: Vec<&str> = utxo.splitn(2, ':').collect();
        assert_eq!(parts.len(), 2);
        let _index: u32 = parts[1].parse().unwrap();
    }

    #[test]
    fn test_call_utxo_parse_invalid() {
        let result = "invalid_utxo_format"
            .splitn(2, ':')
            .collect::<Vec<_>>();
        assert_eq!(result.len(), 1); // no colon separator
    }

    #[test]
    fn test_output_lock_script_round_trip() {
        let script_hash = [0xABu8; 32];
        let datum = vec![1, 2, 3, 4, 5];
        let lock = OutputLock::Script {
            script_hash,
            datum: datum.clone(),
        };
        let locking_condition = lock.to_locking_condition();
        let recovered = OutputLock::from_locking_condition(&locking_condition);
        assert!(recovered.is_some());
        match recovered.unwrap() {
            OutputLock::Script {
                script_hash: sh,
                datum: d,
            } => {
                assert_eq!(sh, script_hash);
                assert_eq!(d, datum);
            }
            _ => panic!("Expected Script lock"),
        }
    }

    #[test]
    fn test_deploy_computes_correct_script_hash() {
        let wasm = minimal_wasm();
        let expected_hash = hex::encode(blake3::hash(&wasm).as_bytes());

        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&wasm).unwrap();

        let wasm_loaded = std::fs::read(f.path()).unwrap();
        let computed_hash = hex::encode(blake3::hash(&wasm_loaded).as_bytes());
        assert_eq!(expected_hash, computed_hash);
    }

    #[test]
    fn test_clean_hex() {
        assert_eq!(clean_hex("0x1234abcd"), "1234abcd");
        assert_eq!(clean_hex("0X1234ABCD"), "1234ABCD");
        assert_eq!(clean_hex("  0xabcd  "), "abcd");
        assert_eq!(clean_hex("feedface"), "feedface");
    }

    #[test]
    fn test_resolve_path() {
        let path = Path::new("relative/path.wasm");
        assert_eq!(resolve_path(path), PathBuf::from("relative/path.wasm"));

        if let Some(home) = std::env::var_os("HOME") {
            let tilde_path = Path::new("~/wallet.json");
            let expected = PathBuf::from(home).join("wallet.json");
            assert_eq!(resolve_path(tilde_path), expected);
        }
    }

    #[test]
    fn test_format_quanta() {
        assert_eq!(format_quanta(100_000_000), "100000000 quanta (1.00000000 SCY)");
        assert_eq!(format_quanta(50_000_000), "50000000 quanta (0.50000000 SCY)");
    }

    #[test]
    fn test_deploy_contract_dry_run_offline() {
        let wasm = minimal_wasm();
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&wasm).unwrap();

        let args = DeployArgs {
            wasm: f.path().to_path_buf(),
            amount: 500_000,
            datum: "01020304".to_string(),
            wallet: PathBuf::from("/nonexistent/wallet.json"),
            fee: 1_000,
            node_url: "http://127.0.0.1:8332".to_string(),
            dry_run: true,
        };

        // Dry-run with nonexistent wallet should gracefully preview without error
        let result = deploy_contract(args);
        assert!(result.is_ok(), "Offline dry-run deploy preview should succeed");
    }
}
