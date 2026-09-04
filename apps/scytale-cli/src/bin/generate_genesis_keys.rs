//! Standalone generator for Scytale Genesis Block official keypairs.
//! Derives 3 distinct Ed25519 keypairs for:
//! - Founder Allocation (21% / 8,820,000 SCY)
//! - Treasury & Developer Allocation (5% / 2,100,000 SCY)
//! - Community & Ecosystem Allocation (5% / 2,100,000 SCY)
//!
//! Writes secrets to `.genesis_keys.json` with strict 0600 POSIX permissions.

use ed25519_dalek::SigningKey;
use scytale_core::Address;
use scytale_primitives::to_hex;
use scytale_script::{builder::ScriptBuilder, opcode::OpCode};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisKeyEntry {
    pub role: String,
    pub allocation_percent: u8,
    pub allocation_scy: u64,
    pub allocation_quanta: u64,
    pub private_key_hex: String,
    pub public_key_hex: String,
    pub address_hash_hex: String,
    pub bech32_address: String,
    pub locking_script_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenesisKeysFile {
    pub generated_at_epoch: u64,
    pub total_allocation_scy: u64,
    pub total_allocation_quanta: u64,
    pub allocations: Vec<GenesisKeyEntry>,
}

fn build_p2pkh_locking_script(address_hash: &[u8; 32]) -> Vec<u8> {
    ScriptBuilder::new()
        .push_opcode(OpCode::OpDup)
        .push_opcode(OpCode::OpBlake3)
        .push_data(address_hash)
        .push_opcode(OpCode::OpEqualVerify)
        .push_opcode(OpCode::OpCheckSig)
        .build()
}

fn generate_entry(role: &str, percent: u8, scy: u64, quanta: u64) -> GenesisKeyEntry {
    let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
    let verifying_key = signing_key.verifying_key();
    let privkey_bytes = signing_key.to_bytes();
    let pubkey_bytes = verifying_key.to_bytes();
    let address_hash = *blake3::hash(&pubkey_bytes).as_bytes();
    let bech32_addr = Address::new(address_hash)
        .to_bech32()
        .expect("bech32 address encoding failed");
    let locking_script = build_p2pkh_locking_script(&address_hash);

    GenesisKeyEntry {
        role: role.to_string(),
        allocation_percent: percent,
        allocation_scy: scy,
        allocation_quanta: quanta,
        private_key_hex: to_hex(&privkey_bytes),
        public_key_hex: to_hex(&pubkey_bytes),
        address_hash_hex: to_hex(&address_hash),
        bech32_address: bech32_addr,
        locking_script_hex: to_hex(&locking_script),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let founder = generate_entry(
        "Founder Allocation",
        21,
        8_820_000,
        882_000_000_000_000,
    );

    let treasury = generate_entry(
        "Development / Treasury",
        5,
        2_100_000,
        210_000_000_000_000,
    );

    let community = generate_entry(
        "Ecosystem / Community",
        5,
        2_100_000,
        210_000_000_000_000,
    );

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let keys_file = GenesisKeysFile {
        generated_at_epoch: now,
        total_allocation_scy: 13_020_000,
        total_allocation_quanta: 1_302_000_000_000_000,
        allocations: vec![founder.clone(), treasury.clone(), community.clone()],
    };

    let target_path = Path::new(".genesis_keys.json");
    let json_content = serde_json::to_string_pretty(&keys_file)?;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(target_path)?;
    file.write_all(json_content.as_bytes())?;
    file.flush()?;

    println!("================================================================================");
    println!("               SCYTALE OFFICIAL GENESIS KEYPAIR GENERATION                      ");
    println!("================================================================================");
    println!("Generated file: .genesis_keys.json (Permissions: 0600 - Strictly Local)");
    println!();

    for entry in &keys_file.allocations {
        println!("--------------------------------------------------------------------------------");
        println!("Role                : {}", entry.role);
        println!("Allocation          : {}% ({} SCY / {} quanta)", entry.allocation_percent, entry.allocation_scy, entry.allocation_quanta);
        println!("Public Key (Hex)    : {}", entry.public_key_hex);
        println!("Address Hash (Hex)  : {}", entry.address_hash_hex);
        println!("Bech32 Address      : {}", entry.bech32_address);
        println!("Locking Script (Hex): {}", entry.locking_script_hex);
    }
    println!("================================================================================");
    println!("Total Genesis Quota : 31% (13,020,000 SCY / 1,302,000,000,000,000 quanta)");
    println!("================================================================================");


    Ok(())
}
