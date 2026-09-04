use clap::Parser;
use std::sync::Arc;
use tempfile::tempdir;

use scytale_bridge::{NodeRequest, NodeResponse};
use scytale_core::{Block, BlockHeader, Hash256, OutPoint, Transaction, TxOut};
use scytale_node::{IpcServer, Node, NodeConfig};

#[allow(dead_code)]
#[path = "../src/client.rs"]
mod client;
#[allow(dead_code)]
#[path = "../src/formatter.rs"]
mod formatter;
#[allow(dead_code)]
#[path = "../src/identity.rs"]
mod identity;
#[allow(dead_code)]
#[path = "../src/wallet.rs"]
mod wallet;

use identity::IdentityStore;

#[derive(Parser, Debug)]
#[command(name = "scytale-cli")]
struct TestCli {
    #[arg(long, global = true, default_value = "/tmp/scytale.sock")]
    socket: String,

    #[arg(long, global = true)]
    identity_file: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: TestCommands,
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum TestCommands {
    Status,
    Account(TestAccountArgs),
    Passbook {
        #[arg(short, long)]
        lock: Option<String>,
    },
    Balance {
        #[arg(short, long)]
        account: Option<String>,
    },
    Send {
        #[arg(short, long)]
        to: String,
        #[arg(short, long)]
        amount: u64,
        #[arg(short, long, default_value_t = 0)]
        fee: u64,
        #[arg(long)]
        from: Option<String>,
    },
    Mine(TestMineArgs),
    Provenance {
        #[arg(short, long)]
        txid: String,
        #[arg(short, long)]
        index: u32,
        #[arg(long)]
        max_depth: Option<usize>,
    },
    Peer(TestPeerArgs),
    Wallet(TestWalletArgs),
    #[command(name = "transfer-p2pkh")]
    TransferP2pkh {
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: u64,
        #[arg(long, default_value_t = 1_000)]
        fee: u64,
        #[arg(long)]
        wallet_file: Option<std::path::PathBuf>,
    },
    #[command(name = "embed-data")]
    EmbedData {
        #[arg(long)]
        data: String,
        #[arg(long, default_value_t = 1_000)]
        fee: u64,
        #[arg(long)]
        wallet_file: Option<std::path::PathBuf>,
    },
    Stop,
}

#[derive(clap::Args, Debug, PartialEq, Eq)]
struct TestWalletArgs {
    #[command(subcommand)]
    action: TestWalletSubcommands,
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum TestWalletSubcommands {
    New {
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
        #[arg(long)]
        force: bool,
    },
    Info {
        #[arg(short, long)]
        file: Option<std::path::PathBuf>,
    },
}

#[derive(clap::Args, Debug, PartialEq, Eq)]
struct TestPeerArgs {
    #[command(subcommand)]
    action: TestPeerSubcommands,
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum TestPeerSubcommands {
    Connect { addr: String },
}

#[derive(clap::Args, Debug, PartialEq, Eq)]
struct TestAccountArgs {
    #[command(subcommand)]
    action: Option<TestAccountSubcommands>,
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum TestAccountSubcommands {
    List,
    New { alias: String },
    Switch { alias: String },
    Show { alias: Option<String> },
}

#[derive(clap::Args, Debug, PartialEq, Eq)]
struct TestMineArgs {
    #[arg(long, conflicts_with = "stop")]
    start: bool,
    #[arg(long, conflicts_with = "start")]
    stop: bool,
}

#[test]
fn test_cli_argument_parsing() {
    let cli = TestCli::try_parse_from(["scytale-cli", "status"]).unwrap();
    assert_eq!(cli.command, TestCommands::Status);

    let cli = TestCli::try_parse_from(["scytale-cli", "passbook"]).unwrap();
    assert_eq!(cli.command, TestCommands::Passbook { lock: None });

    let cli = TestCli::try_parse_from(["scytale-cli", "passbook", "--lock", "010203"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Passbook {
            lock: Some("010203".into())
        }
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "balance"]).unwrap();
    assert_eq!(cli.command, TestCommands::Balance { account: None });

    let cli = TestCli::try_parse_from(["scytale-cli", "balance", "--account", "alice"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Balance {
            account: Some("alice".into())
        }
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "account", "list"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Account(TestAccountArgs {
            action: Some(TestAccountSubcommands::List)
        })
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "account", "new", "alice"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Account(TestAccountArgs {
            action: Some(TestAccountSubcommands::New {
                alias: "alice".into()
            })
        })
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "account", "switch", "alice"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Account(TestAccountArgs {
            action: Some(TestAccountSubcommands::Switch {
                alias: "alice".into()
            })
        })
    );

    let cli = TestCli::try_parse_from([
        "scytale-cli",
        "send",
        "--to",
        "bob",
        "--amount",
        "500000000",
        "--fee",
        "1000",
    ])
    .unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Send {
            to: "bob".into(),
            amount: 500_000_000,
            fee: 1000,
            from: None
        }
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "mine", "--start"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Mine(TestMineArgs {
            start: true,
            stop: false
        })
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "mine", "--stop"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Mine(TestMineArgs {
            start: false,
            stop: true
        })
    );

    let cli = TestCli::try_parse_from([
        "scytale-cli",
        "provenance",
        "--txid",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "--index",
        "0",
    ])
    .unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Provenance {
            txid: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            index: 0,
            max_depth: None
        }
    );

    let cli =
        TestCli::try_parse_from(["scytale-cli", "peer", "connect", "127.0.0.1:9002"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Peer(TestPeerArgs {
            action: TestPeerSubcommands::Connect {
                addr: "127.0.0.1:9002".into()
            }
        })
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "stop"]).unwrap();
    assert_eq!(cli.command, TestCommands::Stop);

    let cli = TestCli::try_parse_from(["scytale-cli", "wallet", "new"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Wallet(TestWalletArgs {
            action: TestWalletSubcommands::New {
                file: None,
                force: false,
            }
        })
    );

    let cli = TestCli::try_parse_from(["scytale-cli", "wallet", "info"]).unwrap();
    assert_eq!(
        cli.command,
        TestCommands::Wallet(TestWalletArgs {
            action: TestWalletSubcommands::Info { file: None }
        })
    );

    let cli = TestCli::try_parse_from([
        "scytale-cli",
        "transfer-p2pkh",
        "--to",
        "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00",
        "--amount",
        "500000000",
        "--fee",
        "2000",
    ])
    .unwrap();
    assert_eq!(
        cli.command,
        TestCommands::TransferP2pkh {
            to: "112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00".into(),
            amount: 500_000_000,
            fee: 2_000,
            wallet_file: None,
        }
    );

    let cli = TestCli::try_parse_from([
        "scytale-cli",
        "embed-data",
        "--data",
        "0x01020304",
        "--fee",
        "500",
    ])
    .unwrap();
    assert_eq!(
        cli.command,
        TestCommands::EmbedData {
            data: "0x01020304".into(),
            fee: 500,
            wallet_file: None,
        }
    );
}

#[tokio::test]
async fn test_cli_fails_gracefully_when_daemon_down() {
    let temp = tempdir().unwrap();
    let sock_path = temp.path().join("nonexistent.sock");

    let err = client::send_node_request(&sock_path, NodeRequest::GetStatus)
        .await
        .unwrap_err();

    assert!(matches!(err, client::CliClientError::DaemonNotRunning));
    let display = err.to_string();
    assert_eq!(
        display,
        "Error: Node daemon is not running. Start scytale-node first."
    );
}

#[test]
fn test_identity_store_file_persistence() {
    let temp = tempdir().unwrap();
    let store_path = temp.path().join("identities.json");

    // 1. Auto-create on first load
    let mut store = IdentityStore::load_or_create(&store_path).unwrap();
    assert_eq!(store.active_account, "default");
    assert!(store.accounts.contains_key("default"));
    assert!(store_path.exists());

    // 2. Create and switch to new accounts
    store.create_account("alice").unwrap();
    store.create_account("bob").unwrap();
    store.switch_account("alice").unwrap();
    store.save(&store_path).unwrap();

    // 3. Reload from disk and verify persistence
    let reloaded = IdentityStore::load_or_create(&store_path).unwrap();
    assert_eq!(reloaded.active_account, "alice");
    assert_eq!(reloaded.accounts.len(), 3);
    assert!(reloaded.accounts.contains_key("alice"));
    assert!(reloaded.accounts.contains_key("bob"));

    // 4. Test alias resolution
    let alice_script = reloaded.accounts["alice"].locking_script_hex.clone();
    assert_eq!(reloaded.resolve_locking_script(None).unwrap(), alice_script);
    assert_eq!(
        reloaded.resolve_locking_script(Some("default")).unwrap(),
        "010203"
    );
    assert_eq!(
        reloaded.resolve_locking_script(Some("deadbeef")).unwrap(),
        "deadbeef"
    );
}

#[tokio::test]
async fn test_ipc_request_response_roundtrip() {
    let temp = tempdir().unwrap();
    let sock_path = temp.path().join("test_scytale.sock");

    let config = NodeConfig {
        data_dir: ":memory:".into(),
        mining_enabled: false,
        miner_payout_script: vec![0x01, 0x02, 0x03],
        ..NodeConfig::default()
    };

    let mut node = Node::open(config).unwrap();
    node.start().unwrap();
    let node = Arc::new(node);

    let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel(1);
    let server = IpcServer::new(&sock_path, Arc::clone(&node), shutdown_tx);

    let server_handle = tokio::spawn(async move {
        let _ = server.run().await;
    });

    // Wait briefly for server socket to be bound
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // 1. Query Status
    let resp = client::send_node_request(&sock_path, NodeRequest::GetStatus)
        .await
        .unwrap();
    match resp {
        NodeResponse::Status {
            state,
            canonical_height,
            mining_active,
            ..
        } => {
            assert_eq!(state, "Running");
            assert_eq!(canonical_height, 0);
            assert!(!mining_active);
        }
        other => panic!("Unexpected response for GetStatus: {other:?}"),
    }

    // 2. Query Passbook (Founder Genesis Output)
    let founder_lock = scytale_core::genesis::GENESIS_FOUNDER_LOCK_HEX;
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::GetPassbook {
            locking_script_hex: founder_lock.into(),
        },
    )
    .await
    .unwrap();
    match resp {
        NodeResponse::Passbook(view) => {
            assert_eq!(view.account_lock_hex, founder_lock);
            assert_eq!(
                view.confirmed_balance_quanta,
                scytale_core::genesis::GENESIS_FOUNDER_QUANTA
            );
        }
        other => panic!("Unexpected response for GetPassbook: {other:?}"),
    }


    // 3. Toggle Mining On and Off
    let resp = client::send_node_request(&sock_path, NodeRequest::SetMining { enabled: true })
        .await
        .unwrap();
    assert!(matches!(resp, NodeResponse::MiningToggled { active: true }));

    let resp = client::send_node_request(&sock_path, NodeRequest::SetMining { enabled: false })
        .await
        .unwrap();
    assert!(matches!(
        resp,
        NodeResponse::MiningToggled { active: false }
    ));

    // Connect Block 1 to fund 010203 so SendTransaction has confirmed inputs
    let genesis_tip = node.canonical_tip();
    let subsidy = scytale_consensus::calculate_block_reward(1);
    let cb1 = Transaction::new_coinbase(1, vec![TxOut::new(subsidy, vec![0x01, 0x02, 0x03])]);
    let mut staging = node.query_utxo_set();
    staging.insert(
        OutPoint::new(cb1.txid(), 0),
        scytale_core::UtxoEntry::new(TxOut::new(subsidy, vec![0x01, 0x02, 0x03]), 1, true),
    );
    let utxo_root = staging.compute_utxo_root();
    let header = BlockHeader::new(1, genesis_tip, Hash256::ZERO, utxo_root, 100, 0x207fffff, 0);
    let block1 = Block::new(header, vec![cb1]);
    assert!(node.submit_external_block(block1).unwrap());
    assert_eq!(node.canonical_height(), 1);

    // 4. Send Transaction via IPC
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::SendTransaction {
            recipient_script_hex: "040506".into(),
            amount_quanta: 200_000_000,
            fee_quanta: 1_000,
            sender_script_hex: Some("010203".into()),
        },
    )
    .await
    .unwrap();
    assert!(matches!(resp, NodeResponse::TransactionSubmitted { .. }));
    assert_eq!(node.mempool_len(), 1);

    // 5. Connect Peer via IPC
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::ConnectPeer {
            addr: "127.0.0.1:9002".into(),
        },
    )
    .await
    .unwrap();
    assert!(matches!(resp, NodeResponse::Success { .. }));

    // 6. Trace Provenance on Genesis coinbase (txid from block 0)
    let chain = node.query_canonical_chain().unwrap();
    let genesis_txid = chain[0].0.transactions[0].txid();
    let resp = client::send_node_request(
        &sock_path,
        NodeRequest::TraceProvenance {
            txid_hex: genesis_txid.to_string(),
            index: 0,
            max_depth: None,
        },
    )
    .await
    .unwrap();
    match resp {
        NodeResponse::Provenance(trace) => {
            assert_eq!(trace.steps.len(), 1);
            assert_eq!(
                trace.steps[0].category,
                scytale_bridge::ProvenanceCategoryDto::Genesis
            );
        }
        other => panic!("Unexpected response for TraceProvenance: {other:?}"),
    }


    // 7. Stop Node
    let resp = client::send_node_request(&sock_path, NodeRequest::StopNode)
        .await
        .unwrap();
    assert!(matches!(resp, NodeResponse::Success { .. }));

    let _ = server_handle.await;
}

#[test]
fn test_formatter_pure_integer_math() {
    assert_eq!(formatter::format_quanta_to_scy(0), "0.00000000");
    assert_eq!(formatter::format_quanta_to_scy(100_000_000), "1.00000000");
    assert_eq!(
        formatter::format_quanta_to_scy(1_000_000_000),
        "10.00000000"
    );
    assert_eq!(formatter::format_quanta_to_scy(50_000_000), "0.50000000");
    assert_eq!(formatter::format_quanta_to_scy(1), "0.00000001");

    assert_eq!(formatter::format_quanta_signed_to_scy(0), "+0.00000000");
    assert_eq!(
        formatter::format_quanta_signed_to_scy(100_000_000),
        "+1.00000000"
    );
    assert_eq!(
        formatter::format_quanta_signed_to_scy(-100_000_000),
        "-1.00000000"
    );

    assert_eq!(formatter::format_integer_commas(100), "100");
    assert_eq!(formatter::format_integer_commas(1000), "1,000");
    assert_eq!(
        formatter::format_integer_commas(1000000000),
        "1,000,000,000"
    );
}
