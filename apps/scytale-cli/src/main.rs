//! Scytale CLI: Operator command line tooling for node inspection, control, and wallet identity.

mod client;
pub mod contract;
pub mod formatter;
pub mod identity;
pub mod wallet;

use clap::{Args, Parser, Subcommand};
use client::{send_node_request, CliClientError};
use contract::{ContractArgs, handle_contract};
use ed25519_dalek::Signer;
use identity::IdentityStore;
use scytale_bridge::{NodeRequest, NodeResponse};
use scytale_core::{Hash256, OutPoint, Transaction, TxIn, TxOut, TRANSACTION_VERSION_1};
use scytale_primitives::from_hex;
use std::path::PathBuf;
use std::process::ExitCode;
use std::str::FromStr;
use wallet::WalletFile;

const DEFAULT_SOCKET_PATH: &str = "/tmp/scytale.sock";

#[derive(Parser, Debug)]
#[command(
    name = "scytale-cli",
    author,
    version,
    about = "Scytale Blockchain Interactive Operator CLI"
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        default_value = DEFAULT_SOCKET_PATH,
        help = "Path to node IPC socket"
    )]
    pub socket: String,

    #[arg(
        long,
        global = true,
        help = "Path to custom identity JSON registry (defaults to ~/.scytale/identities.json)"
    )]
    pub identity_file: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        default_value = "http://127.0.0.1:8332",
        help = "HTTP Gateway URL of the node"
    )]
    pub node_url: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum Commands {
    /// Query node runtime state, tip, height, mempool size, and mining status
    Status,

    /// Manage local wallet identities and account profiles
    Account(AccountArgs),

    /// Query and display the canonical passbook ledger or cryptographic statement
    Passbook(PassbookArgs),

    /// Shortcut to display the confirmed and pending balance of an account
    Balance {
        /// Optional account alias or locking script hex (defaults to active account)
        #[arg(short, long)]
        account: Option<String>,
    },

    /// Create and submit a value transfer transaction to the node mempool
    Send {
        /// Recipient alias or locking condition hex script (e.g. 'bob' or 010203)
        #[arg(short, long)]
        to: String,

        /// Transfer amount in integer quanta (1 SCY = 100,000,000 quanta)
        #[arg(short, long)]
        amount: u64,

        /// Miner fee in integer quanta
        #[arg(short, long, default_value_t = 0)]
        fee: u64,

        /// Optional sender alias or locking script hex (defaults to active account)
        #[arg(long)]
        from: Option<String>,
    },

    /// Control the autonomous Proof-of-Work mining worker
    Mine(MineArgs),

    /// Trace the value provenance lineage of an unspent or spent transaction output
    Provenance {
        /// Transaction ID in 32-byte hexadecimal format
        #[arg(short, long)]
        txid: String,

        /// Output index (0-indexed)
        #[arg(short, long)]
        index: u32,

        /// Maximum hop depth to trace
        #[arg(long)]
        max_depth: Option<usize>,
    },

    /// Manage network peer connections
    Peer(PeerArgs),

    /// Manage non-custodial Ed25519 local wallet keys and P2PKH addresses
    Wallet(WalletArgs),

    /// Create, sign, and submit an Ed25519 P2PKH transfer transaction
    #[command(name = "transfer-p2pkh")]
    TransferP2pkh {
        /// Recipient 32-byte BLAKE3 address in hexadecimal
        #[arg(long)]
        to: String,

        /// Transfer amount in integer quanta (1 SCY = 100,000,000 quanta)
        #[arg(long)]
        amount: u64,

        /// Miner fee in integer quanta (default: 1,000 quanta)
        #[arg(long, default_value_t = 1_000)]
        fee: u64,

        /// Path to wallet JSON file (defaults to ~/.scytale/wallet.json)
        #[arg(long)]
        wallet_file: Option<PathBuf>,
    },

    /// Create, sign, and submit a transaction with an OP_RETURN data carrier output
    #[command(name = "embed-data")]
    EmbedData {
        /// Metadata to commit on-chain (hex string with 0x prefix or UTF-8 text, max 80 bytes)
        #[arg(long)]
        data: String,

        /// Miner fee in integer quanta (default: 1,000 quanta)
        #[arg(long, default_value_t = 1_000)]
        fee: u64,

        /// Path to wallet JSON file (defaults to ~/.scytale/wallet.json)
        #[arg(long)]
        wallet_file: Option<PathBuf>,
    },

    /// Request graceful shutdown of the node daemon
    Stop,

    /// Developer tooling for eUTXO WebAssembly smart contracts (inspect, build, deploy, call)
    Contract(ContractArgs),
}


#[derive(Args, Debug, PartialEq, Eq)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub action: WalletSubcommands,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum WalletSubcommands {
    /// Generate a new Ed25519 keypair and save POSIX 0600 wallet file
    New {
        /// Path to save wallet JSON file (defaults to ~/.scytale/wallet.json)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Overwrite existing file if it already exists
        #[arg(long)]
        force: bool,

        /// Generate wallet with a BIP-39 mnemonic seed phrase
        #[arg(long)]
        mnemonic: bool,

        /// Number of words for BIP-39 mnemonic (12 or 24, default: 12)
        #[arg(long, default_value_t = 12)]
        words: usize,
    },

    /// Restore an existing wallet from a BIP-39 mnemonic phrase
    Restore {
        /// BIP-39 mnemonic phrase (12 or 24 words separated by spaces)
        #[arg(long)]
        phrase: String,

        /// Path to save wallet JSON file (defaults to ~/.scytale/wallet.json)
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Overwrite existing file if it already exists
        #[arg(long)]
        force: bool,
    },

    /// Display wallet details and confirmed balance from node
    Info {
        /// Path to wallet JSON file (defaults to ~/.scytale/wallet.json)
        #[arg(short, long)]
        file: Option<PathBuf>,
    },
}

#[derive(Args, Debug, PartialEq, Eq)]
pub struct PeerArgs {
    #[command(subcommand)]
    pub action: PeerSubcommands,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum PeerSubcommands {
    /// Dynamically connect to a remote network peer address (e.g. 127.0.0.1:9002)
    Connect {
        /// Peer TCP address host:port
        addr: String,
    },
}

#[derive(Args, Debug, PartialEq, Eq)]
pub struct PassbookArgs {
    /// Optional account alias or locking condition hex script (e.g. 010203 or 'default')
    #[arg(short, long)]
    pub lock: Option<String>,

    #[command(subcommand)]
    pub subcommand: Option<PassbookSubcommand>,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum PassbookSubcommand {
    /// Show financial passbook table for an address
    Show {
        /// Bech32 account address (e.g. scy1...)
        address: String,
        /// Starting block height (optional)
        #[arg(long)]
        from_height: Option<u64>,
        /// Ending block height (optional)
        #[arg(long)]
        to_height: Option<u64>,
        /// Maximum entries to display
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// Export or verify a cryptographic Merkle passbook statement for an address
    Statement {
        /// Bech32 account address (e.g. scy1...)
        address: String,
        /// Optional path to save JSON statement output
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Cryptographically verify Merkle inclusion proofs offline against the utxo_root
        #[arg(long)]
        verify: bool,
    },
}

#[derive(Args, Debug, PartialEq, Eq)]
pub struct AccountArgs {
    #[command(subcommand)]
    pub action: Option<AccountSubcommands>,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
pub enum AccountSubcommands {
    /// List all stored account identities
    List,

    /// Create a new account identity with an alias
    New {
        /// Unique human-readable alias
        alias: String,
    },

    /// Switch the active account identity
    Switch {
        /// Account alias to switch to
        alias: String,
    },

    /// Show detailed credentials for an account (defaults to active account)
    Show {
        /// Account alias to inspect
        alias: Option<String>,
    },
}

#[derive(Args, Debug, PartialEq, Eq)]
pub struct MineArgs {
    /// Start the background mining worker
    #[arg(long, conflicts_with = "stop")]
    pub start: bool,

    /// Stop the background mining worker
    #[arg(long, conflicts_with = "start")]
    pub stop: bool,

    /// Optional positional action ("start" or "stop")
    #[arg(index = 1)]
    pub action: Option<String>,

    /// Optional miner payout locking script hex
    #[arg(long)]
    pub payout: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Err(e) = execute(cli).await {
        eprintln!("{e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn execute(cli: Cli) -> Result<(), CliClientError> {
    let identity_path = cli
        .identity_file
        .unwrap_or_else(IdentityStore::default_path);
    let mut store = IdentityStore::load_or_create(&identity_path)
        .map_err(|e| CliClientError::User(format!("Failed to load identity store: {e}")))?;

    match cli.command {
        Commands::Status => {
            let resp = send_node_request(&cli.socket, NodeRequest::GetStatus).await?;
            match resp {
                NodeResponse::Status {
                    state,
                    canonical_height,
                    canonical_tip_hash,
                    mempool_count,
                    mining_active,
                } => {
                    formatter::print_status(
                        &state,
                        canonical_height,
                        &canonical_tip_hash,
                        mempool_count,
                        mining_active,
                    );
                }
                NodeResponse::Error { message } => {
                    eprintln!("Error from node: {message}");
                }
                other => eprintln!("Unexpected response: {other:?}"),
            }
        }

        Commands::Account(args) => match args.action.unwrap_or(AccountSubcommands::List) {
            AccountSubcommands::List => {
                formatter::print_accounts(&store);
            }
            AccountSubcommands::New { alias } => match store.create_account(&alias) {
                Ok(rec) => {
                    store.save(&identity_path).map_err(|e| {
                        CliClientError::User(format!("Failed to save identity store: {e}"))
                    })?;
                    println!("Created new account '{}' successfully.", rec.alias);
                    formatter::print_account_detail(&rec, false);
                }
                Err(e) => {
                    eprintln!("Error creating account: {e}");
                }
            },
            AccountSubcommands::Switch { alias } => match store.switch_account(&alias) {
                Ok(()) => {
                    store.save(&identity_path).map_err(|e| {
                        CliClientError::User(format!("Failed to save identity store: {e}"))
                    })?;
                    println!("Switched active account to '{}'.", store.active_account);
                }
                Err(e) => {
                    eprintln!("Error switching account: {e}");
                }
            },
            AccountSubcommands::Show { alias } => {
                let target_alias = alias.unwrap_or_else(|| store.active_account.clone());
                if let Some(record) = store.accounts.get(&target_alias) {
                    let is_active = target_alias == store.active_account;
                    formatter::print_account_detail(record, is_active);
                } else {
                    eprintln!("Error: Account '{target_alias}' not found in identity store.");
                }
            }
        },

        Commands::Passbook(args) => match args.subcommand {
            Some(PassbookSubcommand::Show {
                address,
                from_height,
                to_height,
                limit,
            }) => {
                let mut url = format!(
                    "{}/api/v1/passbook?address={}",
                    cli.node_url.trim_end_matches('/'),
                    address
                );
                if let Some(fh) = from_height {
                    url.push_str(&format!("&from_height={fh}"));
                }
                if let Some(th) = to_height {
                    url.push_str(&format!("&to_height={th}"));
                }
                url.push_str(&format!("&limit={limit}"));

                let resp = ureq::get(&url)
                    .call()
                    .map_err(|e| CliClientError::User(format!("HTTP request failed to {url}: {e}")))?;

                let view: scytale_node::passbook::PassbookView = resp.into_json().map_err(|e| {
                    CliClientError::User(format!("Failed to parse PassbookView JSON response: {e}"))
                })?;

                formatter::print_passbook_view_table(&address, &view);
            }
            Some(PassbookSubcommand::Statement {
                address,
                output,
                verify,
            }) => {
                let url = format!(
                    "{}/api/v1/passbook/statement?address={}",
                    cli.node_url.trim_end_matches('/'),
                    address
                );

                let resp = ureq::get(&url)
                    .call()
                    .map_err(|e| CliClientError::User(format!("HTTP request failed to {url}: {e}")))?;

                let statement: scytale_node::passbook::PassbookStatement = resp.into_json().map_err(|e| {
                    CliClientError::User(format!("Failed to parse PassbookStatement JSON response: {e}"))
                })?;

                if let Some(output_path) = output {
                    let json_str = serde_json::to_string_pretty(&statement).map_err(|e| {
                        CliClientError::User(format!("Failed to serialize statement JSON: {e}"))
                    })?;
                    std::fs::write(&output_path, json_str).map_err(|e| {
                        CliClientError::User(format!(
                            "Failed to write statement to {}: {e}",
                            output_path.display()
                        ))
                    })?;
                    println!("Passbook statement saved to {}", output_path.display());
                }

                if verify {
                    let is_valid = statement.verify_integrity();
                    formatter::print_statement_verification(&statement, is_valid);
                    if !is_valid {
                        return Err(CliClientError::User(
                            "Cryptographic statement verification failed!".to_string(),
                        ));
                    }
                } else {
                    let is_valid = statement.verify_integrity();
                    formatter::print_statement_verification(&statement, is_valid);
                }
            }
            None => {
                let lock_hex = store.resolve_locking_script(args.lock.as_deref()).map_err(|e| {
                    CliClientError::User(format!("Could not resolve account lock: {e}"))
                })?;

                let resp = send_node_request(
                    &cli.socket,
                    NodeRequest::GetPassbook {
                        locking_script_hex: lock_hex,
                    },
                )
                .await?;
                match resp {
                    NodeResponse::Passbook(view) => {
                        formatter::print_passbook(&view);
                    }
                    NodeResponse::Error { message } => {
                        eprintln!("Error from node: {message}");
                    }
                    other => eprintln!("Unexpected response: {other:?}"),
                }
            }
        },

        Commands::Balance { account } => {
            let lock_hex = store
                .resolve_locking_script(account.as_deref())
                .map_err(|e| CliClientError::User(format!("Could not resolve account: {e}")))?;

            let resp = send_node_request(
                &cli.socket,
                NodeRequest::GetPassbook {
                    locking_script_hex: lock_hex.clone(),
                },
            )
            .await?;
            match resp {
                NodeResponse::Passbook(view) => {
                    println!("============================================================");
                    println!("                 SCYTALE ACCOUNT BALANCE");
                    println!("============================================================");
                    println!(
                        "Locking Script    : 0x{}",
                        lock_hex.strip_prefix("0x").unwrap_or(&lock_hex)
                    );
                    println!(
                        "Confirmed Balance : {} SCY ({} quanta)",
                        formatter::format_quanta_to_scy(view.confirmed_balance_quanta),
                        formatter::format_integer_commas(view.confirmed_balance_quanta)
                    );
                    println!(
                        "Pending Delta     : {} SCY",
                        formatter::format_quanta_signed_to_scy(view.pending_balance_quanta)
                    );
                    println!("============================================================");
                }
                NodeResponse::Error { message } => {
                    eprintln!("Error from node: {message}");
                }
                other => eprintln!("Unexpected response: {other:?}"),
            }
        }

        Commands::Send {
            to,
            amount,
            fee,
            from,
        } => {
            let recipient_hex = store.resolve_locking_script(Some(&to)).map_err(|e| {
                CliClientError::User(format!("Could not resolve recipient '{to}': {e}"))
            })?;
            let sender_hex = store.resolve_locking_script(from.as_deref()).map_err(|e| {
                CliClientError::User(format!("Could not resolve sender script: {e}"))
            })?;

            let resp = send_node_request(
                &cli.socket,
                NodeRequest::SendTransaction {
                    recipient_script_hex: recipient_hex,
                    amount_quanta: amount,
                    fee_quanta: fee,
                    sender_script_hex: Some(sender_hex),
                },
            )
            .await?;
            match resp {
                NodeResponse::TransactionSubmitted { txid } => {
                    println!("Transaction admitted to mempool successfully.");
                    println!("TxID: 0x{}", txid.strip_prefix("0x").unwrap_or(&txid));
                }
                NodeResponse::Error { message } => {
                    eprintln!("Transaction rejected: {message}");
                }
                other => eprintln!("Unexpected response: {other:?}"),
            }
        }

        Commands::Mine(args) => {
            let is_start = args.start || args.action.as_deref() == Some("start");
            let is_stop = args.stop || args.action.as_deref() == Some("stop");
            if !is_start && !is_stop {
                eprintln!("Specify either `start` or `stop` (or `--start` / `--stop`) to control mining.");
                return Ok(());
            }

            let enabled = is_start;
            let resp = send_node_request(&cli.socket, NodeRequest::SetMining { enabled }).await?;
            match resp {
                NodeResponse::MiningToggled { active } => {
                    if active {
                        println!("Proof-of-Work mining worker is now ACTIVE.");
                    } else {
                        println!("Proof-of-Work mining worker is now STOPPED.");
                    }
                }
                NodeResponse::Error { message } => {
                    eprintln!("Error toggling mining: {message}");
                }
                other => eprintln!("Unexpected response: {other:?}"),
            }
        }

        Commands::Provenance {
            txid,
            index,
            max_depth,
        } => {
            let resp = send_node_request(
                &cli.socket,
                NodeRequest::TraceProvenance {
                    txid_hex: txid,
                    index,
                    max_depth,
                },
            )
            .await?;
            match resp {
                NodeResponse::Provenance(trace) => {
                    formatter::print_provenance(&trace);
                }
                NodeResponse::Error { message } => {
                    eprintln!("Provenance trace error: {message}");
                }
                other => eprintln!("Unexpected response: {other:?}"),
            }
        }

        Commands::Peer(args) => match args.action {
            PeerSubcommands::Connect { addr } => {
                let resp =
                    send_node_request(&cli.socket, NodeRequest::ConnectPeer { addr }).await?;
                match resp {
                    NodeResponse::Success { message } => {
                        println!("{message}");
                    }
                    NodeResponse::Error { message } => {
                        eprintln!("Peer connect error: {message}");
                    }
                    other => eprintln!("Unexpected response: {other:?}"),
                }
            }
        },

        Commands::Stop => {
            let resp = send_node_request(&cli.socket, NodeRequest::StopNode).await?;
            match resp {
                NodeResponse::Success { message } => {
                    println!("{message}");
                }
                NodeResponse::Error { message } => {
                    eprintln!("Stop error: {message}");
                }
                other => eprintln!("Unexpected response: {other:?}"),
            }
        }

        Commands::Wallet(args) => match args.action {
            WalletSubcommands::New {
                file,
                force,
                mnemonic,
                words,
            } => {
                let path = file.unwrap_or_else(WalletFile::default_path);
                if mnemonic {
                    let (wallet, phrase) =
                        WalletFile::generate_with_mnemonic(&path, force, words)
                            .map_err(CliClientError::Wallet)?;
                    formatter::print_wallet_mnemonic_created(
                        &path,
                        &wallet.public_key,
                        &wallet.address,
                        &phrase,
                    );
                } else {
                    let wallet =
                        WalletFile::generate_new(&path, force).map_err(CliClientError::Wallet)?;
                    formatter::print_wallet_created(&path, &wallet.public_key, &wallet.address);
                }
            }
            WalletSubcommands::Restore {
                phrase,
                file,
                force,
            } => {
                let path = file.unwrap_or_else(WalletFile::default_path);
                let wallet = WalletFile::restore_from_mnemonic(&path, &phrase, force)
                    .map_err(CliClientError::Wallet)?;
                formatter::print_wallet_restored(&path, &wallet.public_key, &wallet.address);
            }
            WalletSubcommands::Info { file } => {
                let path = file.unwrap_or_else(WalletFile::default_path);
                let wallet = WalletFile::load_from(&path).map_err(CliClientError::Wallet)?;
                let lock_script = wallet
                    .p2pkh_locking_script()
                    .map_err(CliClientError::Wallet)?;

                match send_node_request(
                    &cli.socket,
                    NodeRequest::GetUtxosByLock {
                        locking_script: lock_script,
                    },
                )
                .await
                {
                    Ok(NodeResponse::Utxos(utxos)) => {
                        let count = utxos.len();
                        let confirmed: u64 = utxos.iter().map(|u| u.value_quanta).sum();
                        formatter::print_wallet_info(
                            &path,
                            &wallet.public_key,
                            &wallet.address,
                            count,
                            confirmed,
                        );
                    }
                    Ok(NodeResponse::Error { message }) => {
                        eprintln!("Node error: {message}");
                    }
                    Err(CliClientError::DaemonNotRunning) => {
                        eprintln!("Warning: Node daemon offline. Displaying local wallet details without on-chain balance.");
                        formatter::print_wallet_info(
                            &path,
                            &wallet.public_key,
                            &wallet.address,
                            0,
                            0,
                        );
                    }
                    Err(e) => return Err(e),
                    other => eprintln!("Unexpected node response: {other:?}"),
                }
            }
        },

        Commands::TransferP2pkh {
            to,
            amount,
            fee,
            wallet_file,
        } => {
            let path = wallet_file.unwrap_or_else(WalletFile::default_path);
            let wallet = WalletFile::load_from(&path).map_err(CliClientError::Wallet)?;

            let recipient_addr = scytale_core::Address::parse(&to).map_err(|e| {
                CliClientError::User(format!("Invalid recipient address '{to}': {e}"))
            })?;
            let recipient_lock = wallet::build_p2pkh_locking_script(recipient_addr.hash());
            let sender_lock = wallet
                .p2pkh_locking_script()
                .map_err(CliClientError::Wallet)?;

            let resp = send_node_request(
                &cli.socket,
                NodeRequest::GetUtxosByLock {
                    locking_script: sender_lock.clone(),
                },
            )
            .await?;

            let utxos = match resp {
                NodeResponse::Utxos(u) => u,
                NodeResponse::Error { message } => {
                    return Err(CliClientError::User(format!("Node error: {message}")))
                }
                other => {
                    return Err(CliClientError::User(format!(
                        "Unexpected response: {other:?}"
                    )))
                }
            };

            let total_needed = amount
                .checked_add(fee)
                .ok_or_else(|| CliClientError::User("Amount plus fee overflow".into()))?;

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
                    wallet::WalletError::InsufficientFunds {
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

            let mut outputs = vec![TxOut::new(amount, recipient_lock)];
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
                    wallet::build_p2pkh_unlocking_script(&sig.to_bytes(), &pubkey_bytes);
            }

            let submit_resp = send_node_request(
                &cli.socket,
                NodeRequest::SubmitRawTransaction { tx: Box::new(tx) },
            )
            .await?;

            match submit_resp {
                NodeResponse::TransactionSubmitted { txid } => {
                    formatter::print_p2pkh_transfer_success(&txid, &to, amount, fee);
                }
                NodeResponse::Error { message } => {
                    return Err(CliClientError::User(format!(
                        "Transaction rejected by node: {message}"
                    )));
                }
                other => {
                    return Err(CliClientError::User(format!(
                        "Unexpected response: {other:?}"
                    )))
                }
            }
        }

        Commands::EmbedData {
            data,
            fee,
            wallet_file,
        } => {
            let path = wallet_file.unwrap_or_else(WalletFile::default_path);
            let wallet = WalletFile::load_from(&path).map_err(CliClientError::Wallet)?;

            let payload_bytes = if let Some(hex_str) = data.strip_prefix("0x") {
                from_hex(hex_str)
                    .map_err(|e| CliClientError::User(format!("Invalid hex string: {e}")))?
            } else {
                data.as_bytes().to_vec()
            };

            if payload_bytes.len() > 80 {
                return Err(CliClientError::Wallet(
                    wallet::WalletError::DataPayloadTooLarge {
                        size: payload_bytes.len(),
                        max: 80,
                    },
                ));
            }

            let op_return_lock = wallet::build_op_return_script(&payload_bytes);
            let sender_lock = wallet
                .p2pkh_locking_script()
                .map_err(CliClientError::Wallet)?;

            let resp = send_node_request(
                &cli.socket,
                NodeRequest::GetUtxosByLock {
                    locking_script: sender_lock.clone(),
                },
            )
            .await?;

            let utxos = match resp {
                NodeResponse::Utxos(u) => u,
                NodeResponse::Error { message } => {
                    return Err(CliClientError::User(format!("Node error: {message}")))
                }
                other => {
                    return Err(CliClientError::User(format!(
                        "Unexpected response: {other:?}"
                    )))
                }
            };

            let mut selected_utxos = Vec::new();
            let mut accumulated: u64 = 0;

            let mut sorted_utxos = utxos;
            sorted_utxos.sort_by_key(|b| std::cmp::Reverse(b.value_quanta));

            for u in sorted_utxos {
                accumulated = accumulated.saturating_add(u.value_quanta);
                selected_utxos.push(u);
                if accumulated >= fee && !selected_utxos.is_empty() {
                    break;
                }
            }

            if accumulated < fee || selected_utxos.is_empty() {
                return Err(CliClientError::Wallet(
                    wallet::WalletError::InsufficientFunds {
                        required: fee.max(1),
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

            let mut outputs = vec![TxOut::new(0, op_return_lock)];
            if accumulated > fee {
                let change = accumulated - fee;
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
                    wallet::build_p2pkh_unlocking_script(&sig.to_bytes(), &pubkey_bytes);
            }

            let submit_resp = send_node_request(
                &cli.socket,
                NodeRequest::SubmitRawTransaction { tx: Box::new(tx) },
            )
            .await?;

            match submit_resp {
                NodeResponse::TransactionSubmitted { txid } => {
                    formatter::print_embed_data_success(
                        &txid,
                        &scytale_primitives::to_hex(&payload_bytes),
                        payload_bytes.len(),
                        fee,
                    );
                }
                NodeResponse::Error { message } => {
                    return Err(CliClientError::User(format!(
                        "Transaction rejected by node: {message}"
                    )));
                }
                other => {
                    return Err(CliClientError::User(format!(
                        "Unexpected response: {other:?}"
                    )))
                }
            }
        }

        Commands::Contract(args) => {
            handle_contract(args)?;
        }
    }
    Ok(())
}
