use crate::schema::MIGRATIONS;
use rusqlite::{params, Connection};
use std::path::Path;

/// High-performance relational indexer store backed by SQLite.
pub struct IndexerStore {
    conn: Connection,
}

impl IndexerStore {
    /// Opens (or creates) an indexer database at the specified filesystem path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        let mut store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    /// Creates an ephemeral in-memory indexer database (ideal for tests and dev).
    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let mut store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    /// Runs all pending relational table migrations inside an atomic transaction.
    fn run_migrations(&mut self) -> Result<(), rusqlite::Error> {
        let tx = self.conn.transaction()?;
        for migration in MIGRATIONS {
            tx.execute_batch(migration)?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Mengindeks blok baru secara atomik:
    /// - Menyimpan header blok
    /// - Menyimpan transaksi
    /// - Memperbarui status UTXO yang dibelanjakan (is_spent = 1)
    /// - Mendaftarkan UTXO baru yang belum dibelanjakan (is_spent = 0)
    pub fn index_block(
        &mut self,
        height: u64,
        hash: &str,
        parent_hash: &str,
        timestamp: u64,
        merkle_root: &str,
        utxo_root: &str,
        txs: &[IndexerTxPayload],
    ) -> Result<(), rusqlite::Error> {
        let tx = self.conn.transaction()?;

        // 1. Simpan Header Blok
        tx.execute(
            "INSERT OR REPLACE INTO index_blocks (height, hash, parent_hash, timestamp, tx_count, merkle_root, utxo_root) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![height, hash, parent_hash, timestamp, txs.len() as u64, merkle_root, utxo_root],
        )?;

        // 2. Simpan Transaksi & Mutasi UTXO
        for t in txs {
            tx.execute(
                "INSERT OR REPLACE INTO index_transactions (txid, block_height, block_hash, fee, input_count, output_count) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![t.txid, height, hash, t.fee, t.inputs.len() as u64, t.outputs.len() as u64],
            )?;

            // Tandai UTXO yang dibelanjakan (Spent)
            for input in &t.inputs {
                tx.execute(
                    "UPDATE index_address_utxos SET is_spent = 1 WHERE txid = ?1 AND vout = ?2",
                    params![input.prev_txid, input.prev_vout],
                )?;
            }

            // Daftarkan UTXO baru (Unspent)
            for output in &t.outputs {
                tx.execute(
                    "INSERT OR REPLACE INTO index_address_utxos (address, txid, vout, value_quanta, token_id, is_spent) VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                    params![output.address, t.txid, output.vout, output.value, output.token_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    /// Kueri saldo instan O(1) berdasarkan alamat (hanya menjumlahkan UTXO dengan is_spent = 0).
    pub fn get_address_balance(&self, address: &str) -> Result<u64, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(SUM(value_quanta), 0) FROM index_address_utxos WHERE address = ?1 AND is_spent = 0"
        )?;
        let balance: i64 = stmt.query_row(params![address], |row| row.get(0))?;
        Ok(balance as u64)
    }

    /// Kueri daftar UTXO aktif (unspent) milik alamat tertentu.
    pub fn get_address_utxos(&self, address: &str) -> Result<Vec<AddressUtxoEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT txid, vout, value_quanta, token_id FROM index_address_utxos WHERE address = ?1 AND is_spent = 0 ORDER BY vout ASC"
        )?;
        let rows = stmt.query_map(params![address], |row| {
            Ok(AddressUtxoEntry {
                txid: row.get(0)?,
                vout: row.get(1)?,
                value: row.get(2)?,
                token_id: row.get(3)?,
            })
        })?;
        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }
        Ok(entries)
    }

    /// Mendapatkan tinggi blok terakhir yang sudah terindeks di SQLite
    pub fn get_latest_indexed_height(&self) -> Result<Option<u64>, rusqlite::Error> {
        let mut stmt = self.conn.prepare("SELECT MAX(height) FROM index_blocks")?;
        let height: Option<u64> = stmt.query_row([], |row| row.get(0)).unwrap_or(None);
        Ok(height)
    }

    /// Memeriksa apakah indexer tertinggal dari tinggi target penyimpanan utama
    pub fn requires_catchup(&self, target_height: u64) -> Result<bool, rusqlite::Error> {
        match self.get_latest_indexed_height()? {
            Some(indexed_height) => Ok(indexed_height < target_height),
            None => Ok(target_height > 0),
        }
    }
}

/// Representasi entri UTXO aktif milik alamat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressUtxoEntry {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub token_id: Option<String>,
}

/// Payload transaksi terindeks untuk satu transaksi dalam blok.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerTxPayload {
    pub txid: String,
    pub fee: u64,
    pub inputs: Vec<IndexerInput>,
    pub outputs: Vec<IndexerOutput>,
}

/// Rujukan input yang dibelanjakan (previous outpoint).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerInput {
    pub prev_txid: String,
    pub prev_vout: u32,
}

/// Rujukan output baru yang dibuat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexerOutput {
    pub address: String,
    pub vout: u32,
    pub value: u64,
    pub token_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_store_lifecycle_and_balance() {
        let mut store = IndexerStore::open_in_memory().unwrap();

        // 1. Block 0: Coinbase paying 50 SCY to Alice
        let alice = "scy1alice00000000000000000000000000000000";
        let bob = "scy1bob0000000000000000000000000000000000";

        let cb0 = IndexerTxPayload {
            txid: "tx0000000000000000000000000000000000000000000000000000000000000001".into(),
            fee: 0,
            inputs: vec![],
            outputs: vec![IndexerOutput {
                address: alice.into(),
                vout: 0,
                value: 5_000_000_000,
                token_id: None,
            }],
        };

        store
            .index_block(0, "hash0", "parent0", 1700000000, "merkle0", "utxo0", &[cb0])
            .unwrap();

        assert_eq!(store.get_address_balance(alice).unwrap(), 5_000_000_000);
        assert_eq!(store.get_address_balance(bob).unwrap(), 0);

        let alice_utxos = store.get_address_utxos(alice).unwrap();
        assert_eq!(alice_utxos.len(), 1);
        assert_eq!(alice_utxos[0].value, 5_000_000_000);

        // 2. Block 1: Alice spends 20 SCY to Bob, 30 SCY change to Alice
        let tx1 = IndexerTxPayload {
            txid: "tx0000000000000000000000000000000000000000000000000000000000000002".into(),
            fee: 0,
            inputs: vec![IndexerInput {
                prev_txid: "tx0000000000000000000000000000000000000000000000000000000000000001".into(),
                prev_vout: 0,
            }],
            outputs: vec![
                IndexerOutput {
                    address: bob.into(),
                    vout: 0,
                    value: 2_000_000_000,
                    token_id: None,
                },
                IndexerOutput {
                    address: alice.into(),
                    vout: 1,
                    value: 3_000_000_000,
                    token_id: None,
                },
            ],
        };

        store
            .index_block(1, "hash1", "hash0", 1700000060, "merkle1", "utxo1", &[tx1])
            .unwrap();

        // Check updated balances
        assert_eq!(store.get_address_balance(alice).unwrap(), 3_000_000_000);
        assert_eq!(store.get_address_balance(bob).unwrap(), 2_000_000_000);

        // Alice now only has 1 active UTXO (the change output)
        let alice_utxos_updated = store.get_address_utxos(alice).unwrap();
        assert_eq!(alice_utxos_updated.len(), 1);
        assert_eq!(alice_utxos_updated[0].vout, 1);
        assert_eq!(alice_utxos_updated[0].value, 3_000_000_000);

        // Bob has 1 active UTXO
        let bob_utxos = store.get_address_utxos(bob).unwrap();
        assert_eq!(bob_utxos.len(), 1);
        assert_eq!(bob_utxos[0].value, 2_000_000_000);

        // Verify catchup helpers
        assert_eq!(store.get_latest_indexed_height().unwrap(), Some(1));
        assert!(!store.requires_catchup(1).unwrap());
        assert!(store.requires_catchup(2).unwrap());
    }

    #[test]
    fn test_empty_store_catchup() {
        let store = IndexerStore::open_in_memory().unwrap();
        assert_eq!(store.get_latest_indexed_height().unwrap(), None);
        assert!(!store.requires_catchup(0).unwrap());
        assert!(store.requires_catchup(1).unwrap());
    }
}
