use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::broadcast;

use scytale_core::{
    Block, BlockHeader, Hash256, OutPoint, Transaction, TxOut,
};
use scytale_node::{Node, NodeConfig};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[path = "../src/client.rs"]
mod client;
#[allow(dead_code)]
#[path = "../src/contract.rs"]
mod contract;
#[allow(dead_code)]
#[path = "../src/formatter.rs"]
mod formatter;
#[allow(dead_code)]
#[path = "../src/identity.rs"]
mod identity;
#[allow(dead_code)]
#[path = "../src/wallet.rs"]
mod wallet;

use contract::{
    call_contract, clean_hex, deploy_contract, fetch_utxos_from_node, CallArgs, DeployArgs,
};
use wallet::WalletFile;

#[derive(Serialize, Deserialize)]
struct VaultDatum {
    owner_pubkey: [u8; 32],
    unlock_time: u64,
    emergency_key: [u8; 32],
    penalty_fee: u64,
}

mod serde_sig {
    use serde::{Deserializer, Serializer};
    pub fn serialize<S>(sig: &[u8; 64], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_bytes(sig)
    }
    pub fn deserialize<'de, D>(_d: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok([0u8; 64])
    }
}

#[derive(Serialize, Deserialize)]
enum VaultRedeemer {
    NormalWithdraw {
        #[serde(with = "serde_sig")]
        signature: [u8; 64],
    },
    EmergencyRescue {
        penalty_accepted: bool,
    },
}

fn get_vault_wasm_path() -> PathBuf {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm");
    if !path.exists() {
        panic!("Vault wasm not found at {}. Build it first!", path.display());
    }
    path
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_contract_deploy_and_call_e2e_broadcast() {
    let wasm_path = get_vault_wasm_path();

    // 1. Start an ephemeral Node
    let temp = tempdir().unwrap();
    let config = NodeConfig {
        data_dir: temp.path().to_path_buf(),
        mining_enabled: false,
        miner_payout_script: vec![0x01, 0x02, 0x03],
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).unwrap();
    node.start().unwrap();
    let node = Arc::new(node);

    // 2. Start HTTP Gateway on random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let bind_addr = format!("127.0.0.1:{port}");
    let node_url = format!("http://{bind_addr}");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let bind_copy = bind_addr.clone();
    let node_copy = Arc::clone(&node);
    let gateway_handle = tokio::spawn(async move {
        let _ = scytale_node::run_http_gateway(&bind_copy, node_copy, shutdown_rx).await;
    });

    // Give gateway a moment to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 3. Create wallet and fund it via mined Block 1
    let wallet_dir = tempdir().unwrap();
    let wallet_path = wallet_dir.path().join("wallet.json");
    let wallet = WalletFile::generate_new(&wallet_path, true).unwrap();
    let wallet_script = wallet.p2pkh_locking_script().unwrap();

    let genesis_tip = node.canonical_tip();
    let reward = scytale_consensus::calculate_block_reward(1);
    let cb = Transaction::new_coinbase(1, vec![TxOut::new(reward, wallet_script.clone())]);
    let mut staging = node.query_utxo_set();
    staging.insert(
        OutPoint::new(cb.txid(), 0),
        scytale_core::UtxoEntry::new(TxOut::new(reward, wallet_script), 1, true),
    );
    let utxo_root = staging.compute_utxo_root();
    let header = BlockHeader::new(1, genesis_tip, Hash256::ZERO, utxo_root, 100, 0x207fffff, 0);
    let block1 = Block::new(header, vec![cb]);
    assert!(node.submit_external_block(block1).unwrap());

    // Verify wallet has funded UTXO via gateway
    let utxos = fetch_utxos_from_node(
        &node_url,
        &hex::encode(wallet.p2pkh_locking_script().unwrap()),
    )
    .unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].value_quanta, reward);

    // 4. Test Dry-Run deploy
    let owner_pubkey = wallet.verifying_key_bytes().unwrap();
    let datum = VaultDatum {
        owner_pubkey,
        unlock_time: 0,
        emergency_key: [0u8; 32],
        penalty_fee: 1_000,
    };
    let datum_bytes = bincode::serialize(&datum).unwrap();
    let datum_hex = hex::encode(&datum_bytes);

    let dry_deploy_args = DeployArgs {
        wasm: wasm_path.clone(),
        amount: 10_000_000,
        datum: datum_hex.clone(),
        wallet: wallet_path.clone(),
        fee: 1_000,
        node_url: node_url.clone(),
        dry_run: true,
    };
    assert!(deploy_contract(dry_deploy_args).is_ok());
    assert_eq!(node.query_mempool().len(), 0, "Dry run must not broadcast to mempool");

    // 5. Test Live Broadcast deploy
    let live_deploy_args = DeployArgs {
        wasm: wasm_path.clone(),
        amount: 10_000_000,
        datum: datum_hex.clone(),
        wallet: wallet_path.clone(),
        fee: 1_000,
        node_url: node_url.clone(),
        dry_run: false,
    };
    let deploy_res = deploy_contract(live_deploy_args);
    assert!(deploy_res.is_ok(), "Live deploy must succeed: {:?}", deploy_res);

    // Verify deployment transaction landed in node mempool
    let mempool_after_deploy = node.query_mempool();
    assert_eq!(mempool_after_deploy.len(), 1, "Mempool should hold 1 deployed tx");
    let deploy_tx = mempool_after_deploy[0].transaction.clone();
    let deploy_txid = deploy_tx.txid();

    // 6. Mine Block 2 to confirm deploy transaction into UTXO set
    let tip1 = node.canonical_tip();
    let reward2 = scytale_consensus::calculate_block_reward(2);
    let cb2 = Transaction::new_coinbase(2, vec![TxOut::new(reward2, vec![0x51])]);

    let mut staging2 = node.query_utxo_set();
    for input in &deploy_tx.inputs {
        staging2.remove(&input.previous_output);
    }
    staging2.insert(
        OutPoint::new(cb2.txid(), 0),
        scytale_core::UtxoEntry::new(TxOut::new(reward2, vec![0x51]), 2, true),
    );
    for (idx, out) in deploy_tx.outputs.iter().enumerate() {
        staging2.insert(
            OutPoint::new(deploy_txid, idx as u32),
            scytale_core::UtxoEntry::new(out.clone(), 2, false),
        );
    }
    let utxo_root2 = staging2.compute_utxo_root();
    let header2 = BlockHeader::new(2, tip1, Hash256::ZERO, utxo_root2, 200, 0x207fffff, 0);
    let block2 = Block::new(header2, vec![cb2, deploy_tx.clone()]);
    assert!(node.submit_external_block(block2).unwrap());

    // Verify contract UTXO is in UTXO set
    let contract_outpoint = OutPoint::new(deploy_txid, 0);
    assert!(node.query_utxo_set().get(&contract_outpoint).is_some());

    // 7. Test `contract call` with dry-run
    let redeemer = VaultRedeemer::EmergencyRescue {
        penalty_accepted: true,
    };
    let redeemer_bytes = bincode::serialize(&redeemer).unwrap();
    let redeemer_hex = hex::encode(&redeemer_bytes);

    let dry_call_args = CallArgs {
        utxo: format!("{}:0", deploy_txid),
        wasm: wasm_path.clone(),
        redeemer: redeemer_hex.clone(),
        datum: datum_hex.clone(),
        to: "010203040506".to_string(),
        amount: 9_800_000,
        fee: 200_000,
        signature: None,
        dry_run: true,
        skip_dry_run: false,
        input_amount: 10_000_000,
        node_url: node_url.clone(),
    };
    let dry_call_res = call_contract(dry_call_args);
    assert!(dry_call_res.is_ok(), "Dry-run call should pass ScyVM: {:?}", dry_call_res);

    // 8. Test `contract call` live broadcast
    let live_call_args = CallArgs {
        utxo: format!("{}:0", deploy_txid),
        wasm: wasm_path.clone(),
        redeemer: redeemer_hex.clone(),
        datum: datum_hex.clone(),
        to: "010203040506".to_string(),
        amount: 9_800_000,
        fee: 200_000,
        signature: None,
        dry_run: false,
        skip_dry_run: false,
        input_amount: 10_000_000,
        node_url: node_url.clone(),
    };
    let live_call_res = call_contract(live_call_args);
    assert!(live_call_res.is_ok(), "Live call should broadcast successfully: {:?}", live_call_res);

    // Verify spending transaction is now in node mempool!
    let mempool_after_call = node.query_mempool();
    assert!(
        mempool_after_call.iter().any(|entry| {
            entry.transaction.inputs.iter().any(|i| i.previous_output == contract_outpoint)
        }),
        "Contract spending transaction must be admitted to mempool"
    );

    // 9. Clean up gateway
    let _ = shutdown_tx.send(());
    let _ = gateway_handle.await;
}

#[tokio::test]
async fn test_contract_deploy_daemon_down_error_message() {
    let wasm_path = get_vault_wasm_path();
    let temp = tempdir().unwrap();
    let wallet_path = temp.path().join("wallet.json");
    let _wallet = WalletFile::generate_new(&wallet_path, true).unwrap();

    let deploy_args = DeployArgs {
        wasm: wasm_path,
        amount: 1_000,
        datum: "00".to_string(),
        wallet: wallet_path,
        fee: 100,
        node_url: "http://127.0.0.1:54321".to_string(), // dead port
        dry_run: false,
    };

    let res = deploy_contract(deploy_args);
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert!(
        err_str.contains("Failed to connect to Scytale Node HTTP Gateway"),
        "Error should contain friendly gateway connection guidance: {}",
        err_str
    );
    assert!(
        err_str.contains("scytale-node"),
        "Error should reference scytale-node daemon: {}",
        err_str
    );
}

#[test]
fn test_clean_hex_helper() {
    assert_eq!(clean_hex("0xdeadbeef"), "deadbeef");
    assert_eq!(clean_hex("0XCAFE"), "CAFE");
    assert_eq!(clean_hex("1234"), "1234");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_contract_call_mempool_rejection_error_parsing() {
    let wasm_path = get_vault_wasm_path();

    // 1. Start an ephemeral Node
    let temp = tempdir().unwrap();
    let config = NodeConfig {
        data_dir: temp.path().to_path_buf(),
        mining_enabled: false,
        miner_payout_script: vec![0x01, 0x02, 0x03],
        ..NodeConfig::default()
    };
    let mut node = Node::open(config).unwrap();
    node.start().unwrap();
    let node = Arc::new(node);

    // 2. Start HTTP Gateway on random port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let bind_addr = format!("127.0.0.1:{port}");
    let node_url = format!("http://{bind_addr}");
    let (shutdown_tx, shutdown_rx) = broadcast::channel(1);

    let bind_copy = bind_addr.clone();
    let node_copy = Arc::clone(&node);
    let gateway_handle = tokio::spawn(async move {
        let _ = scytale_node::run_http_gateway(&bind_copy, node_copy, shutdown_rx).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 3. Fund a wallet and mine block with contract UTXO
    let wasm_bytes = std::fs::read(&wasm_path).unwrap();
    let script_hash = *blake3::hash(&wasm_bytes).as_bytes();
    let datum = VaultDatum {
        owner_pubkey: [0u8; 32],
        unlock_time: 0,
        emergency_key: [0u8; 32],
        penalty_fee: 10,
    };
    let datum_bytes = bincode::serialize(&datum).unwrap();
    let datum_hex = hex::encode(&datum_bytes);

    let contract_lock = scytale_core::OutputLock::Script {
        script_hash,
        datum: datum_bytes,
    };
    let genesis_tip = node.canonical_tip();
    let reward = scytale_consensus::calculate_block_reward(1);
    let cb = Transaction::new_coinbase(
        1,
        vec![TxOut::new(reward, contract_lock.to_locking_condition())],
    );
    let mut staging = node.query_utxo_set();
    staging.insert(
        OutPoint::new(cb.txid(), 0),
        scytale_core::UtxoEntry::new(
            TxOut::new(reward, contract_lock.to_locking_condition()),
            1,
            true,
        ),
    );
    let utxo_root = staging.compute_utxo_root();
    let header = BlockHeader::new(1, genesis_tip, Hash256::ZERO, utxo_root, 100, 0x207fffff, 0);
    let block1 = Block::new(header, vec![cb.clone()]);
    assert!(node.submit_external_block(block1).unwrap());

    // 4. Try to call contract with fee = 10 quanta (way below 92KB minimum relay fee)
    let redeemer = VaultRedeemer::EmergencyRescue {
        penalty_accepted: true,
    };
    let redeemer_bytes = bincode::serialize(&redeemer).unwrap();
    let redeemer_hex = hex::encode(&redeemer_bytes);

    let underfunded_call = CallArgs {
        utxo: format!("{}:0", cb.txid()),
        wasm: wasm_path,
        redeemer: redeemer_hex,
        datum: datum_hex,
        to: "010203".to_string(),
        amount: reward.saturating_sub(10),
        fee: 10, // Fee too low for ~92KB transaction!
        signature: None,
        dry_run: false,
        skip_dry_run: false,
        input_amount: reward,
        node_url,
    };

    let res = call_contract(underfunded_call);
    assert!(res.is_err(), "Call with fee too low must be rejected by mempool");
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Mempool submission rejected by node (HTTP 400)"),
        "Should contain mempool rejection header: {err_msg}"
    );
    assert!(
        err_msg.contains("Reason: mempool error: fee rate"),
        "Should contain structured error reason: {err_msg}"
    );

    let _ = shutdown_tx.send(());
    let _ = gateway_handle.await;
}
