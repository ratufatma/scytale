//! Terminal output formatters with strict integer-only arithmetic.

use scytale_bridge::{
    EntryStatusDto, EntryTypeDto, PassbookViewDto, ProvenanceCategoryDto, ProvenanceTraceDto,
};

/// Formats atomic integer quanta into an exact 8-decimal SCY string.
///
/// Guaranteed zero floating-point arithmetic.
pub fn format_quanta_to_scy(quanta: u64) -> String {
    let scy = quanta / 100_000_000;
    let rem = quanta % 100_000_000;
    format!("{}.{:08}", scy, rem)
}

/// Formats a signed integer quanta delta into an exact signed 8-decimal SCY string.
pub fn format_quanta_signed_to_scy(delta: i64) -> String {
    let sign = if delta < 0 { "-" } else { "+" };
    let abs_val = delta.unsigned_abs();
    let scy = abs_val / 100_000_000;
    let rem = abs_val % 100_000_000;
    format!("{}{}.{:08}", sign, scy, rem)
}

/// Formats an integer with thousands separator commas.
pub fn format_integer_commas(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + (s.len() / 3));
    let len = s.len();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

/// Prints a formatted node status summary.
pub fn print_status(
    state: &str,
    canonical_height: u64,
    canonical_tip_hash: &str,
    mempool_count: usize,
    mining_active: bool,
) {
    println!("============================================================");
    println!("                 SCYTALE NODE STATUS");
    println!("============================================================");
    println!("Runtime State     : {}", state);
    println!("Canonical Height  : {}", canonical_height);
    println!(
        "Canonical Tip     : 0x{}",
        canonical_tip_hash
            .strip_prefix("0x")
            .unwrap_or(canonical_tip_hash)
    );
    println!("Mempool Txs       : {}", mempool_count);
    println!(
        "Mining Active     : {}",
        if mining_active { "ACTIVE" } else { "STOPPED" }
    );
    println!("============================================================");
}

/// Prints a canonical bank passbook view.
pub fn print_passbook(view: &PassbookViewDto) {
    let lock_display = if view.account_lock_hex.starts_with("0x") {
        view.account_lock_hex.clone()
    } else {
        format!("0x{}", view.account_lock_hex)
    };

    println!("============================================================");
    println!("                SCYTALE CANONICAL PASSBOOK");
    println!("============================================================");
    println!("Account Lock      : {}", lock_display);
    println!(
        "Confirmed Balance : {} SCY ({} quanta)",
        format_quanta_to_scy(view.confirmed_balance_quanta),
        format_integer_commas(view.confirmed_balance_quanta)
    );
    println!(
        "Pending Delta     : {} SCY",
        format_quanta_signed_to_scy(view.pending_balance_quanta)
    );
    println!("Total Entries     : {}", view.total_entries);
    println!("------------------------------------------------------------");
    println!(
        "{:<8} {:<16} {:<17} {:<10} {:<6}",
        "#ENTRY", "TYPE", "AMOUNT (SCY)", "STATUS", "CONF"
    );
    println!("------------------------------------------------------------");

    if view.entries.is_empty() {
        println!("(No passbook transaction history for this account lock)");
    } else {
        for entry in &view.entries {
            let entry_num_str = format!("#{:06}", entry.entry_number);
            let type_str = match entry.entry_type {
                EntryTypeDto::Received => "Received",
                EntryTypeDto::Sent => "Sent",
                EntryTypeDto::MiningReward => "MiningReward",
                EntryTypeDto::Change => "Change",
            };
            let amount_str = format!("{} SCY", format_quanta_to_scy(entry.amount_quanta));
            let (status_str, conf_str) = match entry.status {
                EntryStatusDto::Confirmed { confirmations } => {
                    ("Confirmed", confirmations.to_string())
                }
                EntryStatusDto::Pending => ("Pending", "-".to_string()),
                EntryStatusDto::Reorganized => ("Reorg", "-".to_string()),
            };

            println!(
                "{:<8} {:<16} {:<17} {:<10} {:<6}",
                entry_num_str, type_str, amount_str, status_str, conf_str
            );
        }
    }
    println!("============================================================");
}

/// Prints a value-provenance trace summary.
pub fn print_provenance(trace: &ProvenanceTraceDto) {
    println!("============================================================");
    println!("                 VALUE PROVENANCE TRACE");
    println!("============================================================");
    println!("Target OutPoint   : {}", trace.target_outpoint);
    println!("Total Hops        : {}", trace.steps.len());
    println!("------------------------------------------------------------");
    println!(
        "{:<5} {:<7} {:<12} {:<17} {:<20}",
        "HOP", "HEIGHT", "CATEGORY", "VALUE (SCY)", "TXID"
    );
    println!("------------------------------------------------------------");

    if trace.steps.is_empty() {
        println!("(No lineage steps found for outpoint)");
    } else {
        for (i, step) in trace.steps.iter().enumerate() {
            let cat_str = match step.category {
                ProvenanceCategoryDto::Coinbase => "Coinbase",
                ProvenanceCategoryDto::Genesis => "Genesis",
                ProvenanceCategoryDto::Transfer => "Transfer",
            };
            let val_str = format!("{} SCY", format_quanta_to_scy(step.value_quanta));
            let txid_short = if step.txid_hex.len() > 16 {
                format!("{}...", &step.txid_hex[..16])
            } else {
                step.txid_hex.clone()
            };

            println!(
                "{:<5} {:<7} {:<12} {:<17} {:<20}",
                i + 1,
                step.block_height,
                cat_str,
                val_str,
                txid_short
            );
        }
    }
    println!("============================================================");
}

/// Prints a list of local accounts from the identity store.
pub fn print_accounts(store: &crate::identity::IdentityStore) {
    println!("============================================================");
    println!("                 SCYTALE LOCAL ACCOUNTS");
    println!("============================================================");
    println!(
        "{:<3} {:<16} {:<38}",
        "ACT", "ALIAS", "LOCKING SCRIPT (HEX)"
    );
    println!("------------------------------------------------------------");

    for (alias, record) in &store.accounts {
        let is_active = alias == &store.active_account;
        let act_marker = if is_active { "*" } else { " " };
        let lock_display = if record.locking_script_hex.len() > 36 {
            format!("{}...", &record.locking_script_hex[..34])
        } else {
            record.locking_script_hex.clone()
        };

        println!(
            "{:<3} {:<16} 0x{:<36}",
            act_marker,
            alias,
            lock_display.strip_prefix("0x").unwrap_or(&lock_display)
        );
    }
    println!("============================================================");
    println!("* = active account profile");
}

/// Prints detailed information for a single account.
pub fn print_account_detail(record: &crate::identity::AccountRecord, is_active: bool) {
    println!("============================================================");
    println!("                 SCYTALE ACCOUNT DETAILS");
    println!("============================================================");
    println!("Alias             : {}", record.alias);
    println!(
        "Active Status     : {}",
        if is_active { "ACTIVE (*)" } else { "INACTIVE" }
    );
    println!(
        "Locking Script    : 0x{}",
        record
            .locking_script_hex
            .strip_prefix("0x")
            .unwrap_or(&record.locking_script_hex)
    );
    println!(
        "Secret Key (Hex)  : 0x{}",
        record
            .secret_key_hex
            .strip_prefix("0x")
            .unwrap_or(&record.secret_key_hex)
    );
    println!("Created (Epoch)   : {}", record.created_at_epoch);
    println!("============================================================");
}

/// Prints a formatted summary when a new wallet is generated.
pub fn print_wallet_created(path: &std::path::Path, pubkey: &str, address: &str) {
    println!("============================================================");
    println!("                 SCYTALE WALLET CREATED");
    println!("============================================================");
    println!("Wallet File       : {}", path.display());
    println!(
        "Public Key (Hex)  : 0x{}",
        pubkey.strip_prefix("0x").unwrap_or(pubkey)
    );
    if address.starts_with("scy1") {
        println!("Address (Bech32)  : {}", address);
    } else {
        println!(
            "Address (Legacy)  : 0x{}",
            address.strip_prefix("0x").unwrap_or(address)
        );
    }
    println!("Permissions       : 0600 (POSIX strict user read/write)");
    println!("============================================================");
}

/// Prints detailed wallet information including on-chain confirmed balance.
pub fn print_wallet_info(
    path: &std::path::Path,
    pubkey: &str,
    address: &str,
    utxo_count: usize,
    confirmed_quanta: u64,
) {
    println!("============================================================");
    println!("                  SCYTALE WALLET INFO");
    println!("============================================================");
    println!("Wallet File       : {}", path.display());
    println!(
        "Public Key (Hex)  : 0x{}",
        pubkey.strip_prefix("0x").unwrap_or(pubkey)
    );
    if address.starts_with("scy1") {
        println!("Address (Bech32)  : {}", address);
    } else {
        println!(
            "Address (Legacy)  : 0x{}",
            address.strip_prefix("0x").unwrap_or(address)
        );
    }
    println!("UTXOs Available   : {}", utxo_count);
    println!(
        "Confirmed Balance : {} SCY ({} quanta)",
        format_quanta_to_scy(confirmed_quanta),
        format_integer_commas(confirmed_quanta)
    );
    println!("============================================================");
}

/// Prints formatted confirmation for a P2PKH transfer.
pub fn print_p2pkh_transfer_success(
    txid: &str,
    to_addr: &str,
    amount_quanta: u64,
    fee_quanta: u64,
) {
    println!("============================================================");
    println!("             P2PKH TRANSACTION SUBMITTED");
    println!("============================================================");
    println!(
        "Transaction ID    : 0x{}",
        txid.strip_prefix("0x").unwrap_or(txid)
    );
    if to_addr.starts_with("scy1") {
        println!("Recipient Address : {}", to_addr);
    } else {
        println!(
            "Recipient Address : 0x{}",
            to_addr.strip_prefix("0x").unwrap_or(to_addr)
        );
    }
    println!(
        "Amount Sent       : {} SCY ({} quanta)",
        format_quanta_to_scy(amount_quanta),
        format_integer_commas(amount_quanta)
    );
    println!(
        "Miner Fee         : {} SCY ({} quanta)",
        format_quanta_to_scy(fee_quanta),
        format_integer_commas(fee_quanta)
    );
    println!("Status            : Admitted to local mempool & broadcasted to P2P network");
    println!("============================================================");
}

/// Prints formatted confirmation for an embedded data transaction.
pub fn print_embed_data_success(
    txid: &str,
    payload_hex: &str,
    payload_len: usize,
    fee_quanta: u64,
) {
    println!("============================================================");
    println!("            DATA COMMITMENT TRANSACTION SUBMITTED");
    println!("============================================================");
    println!(
        "Transaction ID    : 0x{}",
        txid.strip_prefix("0x").unwrap_or(txid)
    );
    println!("Payload Size      : {} bytes (OP_RETURN)", payload_len);
    println!(
        "Payload (Hex)     : 0x{}",
        payload_hex.strip_prefix("0x").unwrap_or(payload_hex)
    );
    println!(
        "Miner Fee         : {} SCY ({} quanta)",
        format_quanta_to_scy(fee_quanta),
        format_integer_commas(fee_quanta)
    );
    println!("Status            : Admitted to mempool (data carrier will commit on-chain)");
    println!("============================================================");
}
