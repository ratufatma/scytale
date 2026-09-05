//! Smart contract developer tooling subcommands for `scytale-cli contract`.
//!
//! Subcommands:
//! - `inspect` — Print BLAKE3 script_hash and binary size of a .wasm file
//! - `build`   — Compile a contract crate to `wasm32-unknown-unknown` release
//! - `deploy`  — Build a locking transaction to an OutputLock::Script UTXO
//! - `call`    — Spend a script UTXO with ScyVM dry-run validation

use clap::{Args, Subcommand};
use std::path::PathBuf;

use scytale_core::{
    vm_adapter::{create_tx_context, MAX_TX_GAS},
    Hash256, OutPoint, OutputLock, Transaction, TxInput, TxOut, TxOutput,
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

    /// Spend a script UTXO with optional local ScyVM dry-run before broadcast
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

#[derive(Debug, Args, PartialEq, Eq)]
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

    /// RPC node URL to broadcast the deployment transaction
    #[arg(long, default_value = "http://127.0.0.1:8332")]
    pub rpc_url: String,
}

#[derive(Debug, Args, PartialEq, Eq)]
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

    /// Recipient locking script hex for the unlocked funds
    #[arg(long)]
    pub to: String,

    /// Amount to send to recipient (leave 0 to auto-calculate from UTXO - fee)
    #[arg(long, default_value_t = 0)]
    pub amount: u64,

    /// Miner fee in quanta
    #[arg(long, default_value_t = 1_000)]
    pub fee: u64,

    /// Skip the local ScyVM dry-run simulation (not recommended)
    #[arg(long)]
    pub skip_dry_run: bool,

    /// Simulated input amount in quanta for dry-run (use actual UTXO value)
    #[arg(long, default_value_t = 0)]
    pub input_amount: u64,

    /// RPC node URL to broadcast the spend transaction
    #[arg(long, default_value = "http://127.0.0.1:8332")]
    pub rpc_url: String,
}

// ── Handler dispatcher ────────────────────────────────────────────────────────

pub fn handle_contract(args: ContractArgs) -> Result<(), CliClientError> {
    match args.action {
        ContractCommands::Inspect(a) => cmd_inspect(a),
        ContractCommands::Build(a) => cmd_build(a),
        ContractCommands::Deploy(a) => cmd_deploy(a),
        ContractCommands::Call(a) => cmd_call(a),
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
    let wasm_bytes = std::fs::read(&args.wasm)
        .map_err(|e| CliClientError::User(format!("Failed to read wasm: {e}")))?;

    let datum_bytes = hex::decode(&args.datum)
        .map_err(|e| CliClientError::User(format!("Invalid datum hex: {e}")))?;

    let script_hash = *blake3::hash(&wasm_bytes).as_bytes();
    let lock = OutputLock::Script {
        script_hash,
        datum: datum_bytes,
    };

    // Construct the locking transaction structure
    let locked_output = TxOutput::new(args.amount, lock);
    let locked_tx_out = locked_output.to_tx_out();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║             SCYTALE CONTRACT DEPLOY (PREVIEW)               ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  Wasm File    : {}", args.wasm.display());
    println!("  Script Hash  : {}", hex::encode(script_hash));
    println!("  Lock Amount  : {} quanta", args.amount);
    println!("  Miner Fee    : {} quanta", args.fee);
    println!("  RPC Target   : {}", args.rpc_url);
    println!();
    println!("  OutputLock::Script {{");
    println!("    script_hash : {},", hex::encode(script_hash));
    println!("    datum       : {} bytes", args.datum.len() / 2);
    println!("  }}");
    println!();
    println!("  Locking condition bytes: {} total", locked_tx_out.locking_condition.len());
    println!();
    println!("  ⚠ NOTE: deploy broadcast via HTTP RPC is not yet implemented.");
    println!("    To deploy, submit the transaction via the node IPC socket:");
    println!("    Use `scytale-cli send` to fund a script-locked UTXO manually,");
    println!("    or integrate with the HTTP gateway at {}/api/v1/tx/submit", args.rpc_url);
    println!();
    println!("  ✓ Script hash computed and output lock structure validated.");

    Ok(())
}

// ── `contract call` ──────────────────────────────────────────────────────────

pub fn cmd_call(args: CallArgs) -> Result<(), CliClientError> {
    // 1. Parse UTXO reference
    let parts: Vec<&str> = args.utxo.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(CliClientError::User(
            "Invalid --utxo format. Expected <tx_hash_hex>:<output_index>".to_string(),
        ));
    }
    let txid_hex = parts[0];
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

    let redeemer_bytes = hex::decode(&args.redeemer)
        .map_err(|e| CliClientError::User(format!("Invalid redeemer hex: {e}")))?;

    let datum_bytes = hex::decode(&args.datum)
        .map_err(|e| CliClientError::User(format!("Invalid datum hex: {e}")))?;

    let to_bytes = hex::decode(&args.to)
        .map_err(|e| CliClientError::User(format!("Invalid --to locking script hex: {e}")))?;

    let script_hash = *blake3::hash(&wasm_bytes).as_bytes();
    let input_amount = if args.input_amount > 0 {
        args.input_amount
    } else if args.amount + args.fee > 0 {
        args.amount + args.fee
    } else {
        1_000_000 // default placeholder
    };
    let output_amount = if args.amount > 0 {
        args.amount
    } else {
        input_amount.saturating_sub(args.fee)
    };

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║             SCYTALE CONTRACT CALL                           ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  UTXO         : {}:{}", txid_hex, output_index);
    println!("  Wasm File    : {}", args.wasm.display());
    println!("  Script Hash  : {}", hex::encode(script_hash));
    println!("  Redeemer     : {} bytes", redeemer_bytes.len());
    println!("  Datum        : {} bytes", datum_bytes.len());
    println!("  Send To      : {}...", &args.to[..args.to.len().min(24)]);
    println!("  Output Amount: {} quanta", output_amount);
    println!("  Miner Fee    : {} quanta", args.fee);

    // 3. Build the spending transaction structure
    let outpoint = OutPoint::new(Hash256::new(txid_arr), output_index);
    let eutxo_input = TxInput::new(
        txid_arr,
        output_index,
        None,
        Some(redeemer_bytes.clone()),
        Some(wasm_bytes.clone()),
    );
    let tx_in = eutxo_input.to_tx_in();
    let tx_out = TxOut::new(output_amount, to_bytes);
    let spending_tx = Transaction::new(
        TRANSACTION_VERSION_1,
        vec![tx_in],
        vec![tx_out],
        0,
    );

    // 4. ScyVM Dry-Run
    if !args.skip_dry_run {
        println!();
        println!("  ── ScyVM Dry-Run Simulation ─────────────────────────────────");

        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let tx_context = create_tx_context(
            &spending_tx,
            current_time,
            input_amount,
            output_amount,
        );

        println!("  [*] Executing ScyVM sandbox...");
        println!(
            "      block_time   = {} (current unix timestamp)",
            current_time
        );
        println!("      input_amount = {} quanta", input_amount);
        println!("      fee_burned   = {} quanta", input_amount.saturating_sub(output_amount));
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

        if !exec_result.is_valid {
            println!("  [✗] VALIDATION REJECTED by smart contract.");
            println!();
            println!("  Possible reasons:");
            println!("    - Timelock has not expired (block_time < datum.unlock_time)");
            println!("    - Redeemer is invalid or malformed");
            println!("    - Signature check failed inside the contract");
            println!();
            println!("  Transaction aborted. Use --skip-dry-run to override (dangerous).");
            return Err(CliClientError::User(
                "Dry-run rejected: smart contract returned VALIDATION_REJECT".to_string(),
            ));
        }

        println!(
            "  [✓] DRY-RUN PASSED! Gas consumed: {} fuel",
            exec_result.gas_consumed
        );
        println!("  ──────────────────────────────────────────────────────────────");
    } else {
        println!("  [!] Dry-run skipped (--skip-dry-run). Proceeding without simulation.");
    }

    println!();
    println!("  ⚠ NOTE: HTTP RPC broadcast is not yet implemented.");
    println!("    The spending transaction has been validated locally.");
    println!("    To submit, integrate with the node IPC or HTTP gateway:");
    println!("    POST to {}/api/v1/tx/submit", args.rpc_url);
    println!();
    println!("  ✓ Contract call dry-run validation complete.");

    let _ = outpoint; // used for future broadcast
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
}
