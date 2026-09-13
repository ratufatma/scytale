//! Relational SQLite schema and migrations for Scytale indexer.

pub const MIGRATIONS: &[&str] = &[
    // 1. Tabel Blok Terindeks
    r#"
    CREATE TABLE IF NOT EXISTS index_blocks (
        height INTEGER PRIMARY KEY,
        hash TEXT NOT NULL UNIQUE,
        parent_hash TEXT NOT NULL,
        timestamp INTEGER NOT NULL,
        tx_count INTEGER NOT NULL,
        merkle_root TEXT NOT NULL,
        utxo_root TEXT NOT NULL
    );
    "#,

    // 2. Tabel Transaksi Terindeks
    r#"
    CREATE TABLE IF NOT EXISTS index_transactions (
        txid TEXT PRIMARY KEY,
        block_height INTEGER NOT NULL,
        block_hash TEXT NOT NULL,
        fee INTEGER NOT NULL,
        input_count INTEGER NOT NULL,
        output_count INTEGER NOT NULL,
        FOREIGN KEY(block_height) REFERENCES index_blocks(height)
    );
    "#,

    // 3. Tabel Alamat & UTXO Aktif (Read-Optimized O(1) Saldo Dompet)
    r#"
    CREATE TABLE IF NOT EXISTS index_address_utxos (
        address TEXT NOT NULL,
        txid TEXT NOT NULL,
        vout INTEGER NOT NULL,
        value_quanta INTEGER NOT NULL,
        token_id TEXT,
        is_spent INTEGER NOT NULL DEFAULT 0, -- 0 = Unspent, 1 = Spent
        PRIMARY KEY (txid, vout)
    );
    CREATE INDEX IF NOT EXISTS idx_address_unspent ON index_address_utxos(address, is_spent);
    "#,

    // 4. Tabel Statistik Jaringan
    r#"
    CREATE TABLE IF NOT EXISTS index_network_stats (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );
    "#
];
