# Work Record: 48. Passbook Enhancement — Multi-Asset eUTXO, Cryptographic Merkle Statements & VPS Production Tooling

## Overview
Catatan kerja ini mendokumentasikan evolusi arsitektural modul **Passbook** Scytale dari antarmuka visualisasi saldo dasar menjadi sistem pencatatan keuangan (*financial ledger*) terverifikasi kriptografis tingkat produksi. Peningkatan ini mencakup pengindeksan sekunder alamat transaksi atomik, pelacakan multi-aset eUTXO (koin Native & token Scy20), pembuktian inklusi Merkle UTXO set offline, eksposur REST HTTP Gateway, visualisasi CLI TUI bergaya buku tabungan bank (*pure-integer ASCII passbook table*), serta kesiapan deployment VPS dengan template unit service systemd dan skrip otomatisasi lingkungan.

---

## Arsitektur & Komponen Utama

```text
  ┌─────────────────────────────────────────────────────────────────────────────┐
  │                      SCYTALE CORE & STORAGE ARCHITECTURE                    │
  │                                                                             │
  │  [redb Storage Engine]                                                      │
  │     ├── BLOCKS & HEADERS                                                    │
  │     ├── UTXO_SET                                                            │
  │     └── ADDRESS_TX_INDEX (New: [u8; 40] -> Vec<AddressTxRecord>)            │
  │           (Key: address_bytes[32B] || height_be[8B])                        │
  │                                                                             │
  │  [Passbook Statement Generator] (crates/scytale-core/src/utxo.rs)           │
  │     ├── generate_utxo_merkle_proof()                                        │
  │     ├── verify_utxo_inclusion()                                             │
  │     ├── generate_passbook_statement()                                       │
  │     └── verify_statement()                                                  │
  │                                                                             │
  │  [Passbook Query Engine] (apps/scytale-node/src/passbook.rs)                │
  │     ├── Multi-Asset Support (PassbookAsset::Native & PassbookAsset::Scy20)  │
  │     ├── Financial Semantics (PassbookAction, Datum Hash, Pending Balance)   │
  │     └── Fast Range Queries via ADDRESS_TX_INDEX (O(K) vs O(N))              │
  │                                                                             │
  │  [HTTP Gateway API] (apps/scytale-node/src/http_gateway.rs)                 │
  │     ├── GET /api/v1/passbook?address=<bech32>&limit=50                      │
  │     └── GET /api/v1/passbook/statement?address=<bech32>                     │
  │                                                                             │
  │  [CLI Presentation] (apps/scytale-cli/src/passbook_cmd.rs)                  │
  │     ├── scytale-cli passbook show <address> (ASCII Bank Table)              │
  │     └── scytale-cli passbook statement <address> [--verify]                 │
  │                                                                             │
  │  [Production VPS Systemd Tooling] (scripts/systemd/ & scripts/)             │
  │     ├── scytale-node.service (HTTP 8332, P2P 9001, Ingest Webhook)          │
  │     ├── scytale-explorer.service (Node.js Express microservice)             │
  │     └── setup_vps_env.sh (UFW firewall, non-root user, 0700 storage permissions)
  └─────────────────────────────────────────────────────────────────────────────┘
```

---

## Rincian Implementasi Per Tahap

### Tahap 1: Secondary Address Indexing (`crates/scytale-storage`)
* **Definisi Tabel Baru (`tables.rs`):**
  `ADDRESS_TX_INDEX: TableDefinition<&[u8; 40], &[u8]>`
  - Kunci komposit 40-byte: `address_hash[0..32] || block_height[32..40]` (Big-Endian).
  - Nilai: Serialisasi bincode kanonikal dari `Vec<AddressTxRecord>`.
* **Atomic Pipeline Integration (`storage.rs`):**
  - Pada `commit_block()`, seluruh input (pengeluaran) dan output (penerimaan) diekstrak dan dicatat dalam satu transaksi tulis atomik redb bersama data blok dan pembaruan UTXO set.
  - Pada `reorganize()` dan `unwind_block()`, rekaman indeks sekunder pada blok-blok yang di-orphan dihapus secara deterministik untuk menjamin integritas rantai kanonikal.
* **API Pembacaan:**
  - `get_address_txs(address, from_height, to_height, limit)` melakukan range query berarah maju/mundur dengan kompleksitas $O(K)$ rekaman relevan, menghindari full-scan terhadap seluruh histori rantai.

### Tahap 2: Multi-Asset Tracking & Rich Passbook View (`apps/scytale-node`)
* **Tipe Data & Enum Keuangan (`passbook.rs`):**
  - `PassbookAsset`: Mendukung `Native` (koin SCY dasar) dan `Scy20 { token_id: Hash256 }` (token kustom eUTXO).
  - `PassbookAction`: Mengklasifikasikan transaksi secara semantik: `Received`, `Sent`, `MiningReward`, `Change`, `Scy20Mint`, `Scy20Transfer`, `Scy20Burn`, `ContractInteraction { datum_hash }`, `VaultDeposit { timelock_until }`, dan `VaultWithdrawal`.
* **Struktur PassbookView:**
  - `confirmed_native_balance_quanta: u64`
  - `token_balances: BTreeMap<Hash256, u64>`
  - `pending_native_balance_quanta: i64` (perhitungan delta dari mempool real-time)
  - `entries: Vec<PassbookEntry>` dengan pagination dan filtering rentang ketinggian blok.

### Tahap 3: Cryptographic Passbook Statement & Merkle Proof (`crates/scytale-core`)
* **Struktur Bukti Merkle UTXO Set (`utxo.rs`):**
  ```rust
  pub struct UtxoMerkleProof {
      pub outpoint: OutPoint,
      pub value_quanta: u64,
      pub leaf_hash: Hash256,
      pub audit_path: Vec<(Hash256, bool)>,
      pub leaf_index: usize,
  }
  ```
* **Verifikasi Inklusi Kriptografis:**
  - `generate_utxo_merkle_proof(utxos, target_outpoint)` menghasilkan jalur audit biner deterministik.
  - `verify_utxo_inclusion(root, proof)` merekonstruksi hash root secara independen.
* **Passbook Statement Mandiri:**
  - `generate_passbook_statement()` mengagregasi seluruh UTXO aktif milik alamat target, total saldo kanonikal, commit hash blok/state, dan bundel bukti Merkle untuk setiap outpoint.
  - `verify_statement(root, statement)` memvalidasi bahwa seluruh leaf dalam statement secara matematis terikat pada `utxo_root` rantai tanpa memerlukan akses ke full-node.

### Tahap 4: HTTP Gateway Endpoints & CLI TUI (`apps/scytale-node` & `apps/scytale-cli`)
* **REST HTTP Gateway (`http_gateway.rs`):**
  - `GET /api/v1/passbook`: Mengembalikan serialized JSON dari `PassbookView` berdasarkan query parameter `address`, `from_height`, `to_height`, dan `limit`.
  - `GET /api/v1/passbook/statement`: Menghasilkan `PassbookStatement` berbobot bukti kriptografis untuk alamat target.
* **Penyajian CLI Passbook TUI (`passbook_cmd.rs`):**
  - Sub-perintah `scytale-cli passbook show <address>` merender tabel buku tabungan ASCII murni:
    - Kolom terstruktur: `DATE/TIME`, `BLOCK`, `TXID`, `ACTION`, `ASSET`, `DEBIT`, `CREDIT`, dan `RUNNING BALANCE`.
    - Format moneter menggunakan kalkulasi integer murni (`quanta / 10^8` dan format 8-digit desimal), menghindari pembulatan floating point IEEE-754.
  - Sub-perintah `scytale-cli passbook statement <address> [--verify]` menampilkan ringkasan cryptographic proof dan melakukan verifikasi lokal offline.

### Tahap 5: Kesiapan Deployment VPS & Systemd Tooling (`scripts/`)
* **Template Unit Systemd:**
  - `scripts/systemd/scytale-node.service`: Mengatur daemon Rust dengan binding internal HTTP 127.0.0.1:8332, listener P2P 0.0.0.0:9001, webhook indexer ke port 3000, `LimitNOFILE=65535`, sandboxing Linux (`NoNewPrivileges`, `ProtectSystem=full`), dan auto-restart `on-failure`.
  - `scripts/systemd/scytale-explorer.service`: Mengelola microservice web explorer berbasis Node.js dengan port 3000, binding lokal, auto-restart `always`, dan integrasi database SQLite WAL.
* **Skrip Otomatisasi Lingkungan VPS (`scripts/setup_vps_env.sh`):**
  - Mengonfigurasi user/group non-root `scytale` tanpa shell interaktif.
  - Menerapkan hak akses ketat `0700` pada direktori basis data `/var/lib/scytale/data`.
  - Membuat token otentikasi ingest bersama yang aman di `/etc/scytale/node.env` dan `/etc/scytale/explorer.env`.
  - Mengatur konfigurasi firewall UFW: membuka port 22 (SSH) dan 9001 (P2P Wire), sambil memproteksi port 8332 dan 3000 agar hanya dapat diakses melalui jaringan lokal atau reverse proxy.

---

## Verifikasi & Hasil Pengujian

1. **Unit & Integration Test Workspace:**
   - Seluruh test suite pada `scytale-storage`, `scytale-core`, `scytale-node`, `scytale-cli`, `scytale-mempool`, dan `scytale-vm` lulus 100%.
   - Verifikasi mempool eviction: Mekanisme fail-closed teruji saat mempool penuh, transaksi dengan fee-rate lebih rendah ditolak dengan `MempoolError::MempoolFull`.
   - Verifikasi ScyVM memory & fuel: Pagu memori 64 halaman (4 MiB) dan kehabisan bahan bakar mentrap secara terkontrol tanpa menimbulkan panic.
2. **Kompilasi Binari Rilis:**
   - Binari `target/release/scytale-node` dan `target/release/scytale-cli` terkompilasi bersih tanpa peringatan.
3. **P2P Daemon Test Suite:**
   - Seluruh test suite Go pada `network/` lulus tanpa regresi.
