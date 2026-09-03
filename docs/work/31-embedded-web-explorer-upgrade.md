# Scytale Protocol — Technical Specification & Architecture Record: Task 31
## Embedded Web Explorer Upgrade (Bech32 Formatting & Mempool Priority Inspector)

```text
Document ID   : SPEC-TASK-31
Task ID       : 31 (Embedded Web Explorer Upgrade)
Phase         : Phase 3 — User-Facing Protocol & Client Tooling
Target Files  : apps/scytale-node/src/http_gateway.rs, web/explorer/index.html
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Zero-Dependency Runtime, Embedded Binary HTML (include_str!), Dual Bech32/Hex Resolution, Real-Time Fee Market Telemetry
```

---

## 1. Ruang Lingkup & Desain Perubahan

Rencana pembaruan antarmuka **Web Explorer Tersemat** dirancang untuk memanfaatkan kapabilitas yang telah dibangun pada Task 28 (Mempool Telemetry) dan Task 30 (Bech32 Encoding), menjaga arsitektur *zero-dependency* tanpa *external runtime assets*.

```text
┌────────────────────────────────────────────────────────────────────────┐
│               scytale-node (Embedded HTTP Gateway :8332)               │
│                                                                        │
│   GET /api/v1/blocks   ──► TxOutputDto diperkaya dengan `address`      │
│                            (otomatis di-encode ke scy1... jika P2PKH)  │
│   GET /api/v1/mempool  ──► Menyajikan metrik kapasitas & antrean       │
│   GET /                ──► web/explorer/index.html                     │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        web/explorer/index.html                         │
│                                                                        │
│  [ Panel 1: Header & Global Search ]                                   │
│  • Input pencarian mendukung: Block Height, Block Hash, TxID, dan       │
│    Alamat Bech32 (scy1...) ──► Otomatis memanggil /api/v1/passbook/:lock│
│                                                                        │
│  [ Panel 2: Live Mempool & Fee Market Inspector ] (BARU)               │
│  • Gauge Bar Kapasitas: Count (0/5.000 txs) & Size (0/5 MB)           │
│  • Indikator Pasar Biaya: Min Relay Floor (1 q/B), Median Fee Rate     │
│  • Tabel Antrean Prioritas: TxID, Size (B), Fee, Fee-Rate (milli-q/B), │
│    serta penanda visual status aman vs risiko penggusuran (eviction)   │
│                                                                        │
│  [ Panel 3: Recent Blocks & Transactions ]                             │
│  • Output P2PKH ditampilkan sebagai pill scy1... (copy-to-clipboard)  │
│  • Output OP_RETURN ditampilkan sebagai metadata badge abu-abu gelap   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Spesifikasi Teknis Backend (`apps/scytale-node/src/http_gateway.rs`)

Agar antarmuka web tetap ringan (*zero JS external library* untuk Bech32), backend HTTP Gateway bertanggung jawab mengekstrak skrip penguncian menjadi alamat ramah pengguna:

### A. Pengayaan DTO Transaksi Output (`TxOutputDto`)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutputDto {
    pub value_quanta: u64,
    pub locking_script: String,
    pub script_type: String,               // "p2pkh", "op_return", atau "custom"
    pub address: Option<String>,           // "scy1..." jika script berupa P2PKH standar
    pub op_return_payload: Option<String>, // Teks UTF-8 atau hex jika OP_RETURN
}
```

### B. Pendeteksi Skrip (`analyze_locking_script`)
* **Pendeteksian P2PKH Standar:**
  Pola kanonikal ScytaleScript: `OP_DUP (0x73) OP_BLAKE3 (0xa0) OP_PUSHBYTES_32 (0x20) [32-byte hash] OP_EQUALVERIFY (0x88) OP_CHECKSIG (0xac)` (total 37 byte).
  * Ekstrak hash 32-byte pada indeks `[3..35]`.
  * Konversi via `Address::from_pubkey_hash(hash).to_bech32()`.
  * Kembalikan `("p2pkh".to_string(), Some(bech32_str), None)`.
* **Pendeteksian `OP_RETURN` (0x6a):**
  * Ekstrak payload byte data carrier (`script[2..]`).
  * Jika valid UTF-8 string, tampilkan teks; jika tidak, tampilkan format hex (`0x...`).
  * Kembalikan `("op_return".to_string(), None, Some(payload))`.
* **Skrip Lainnya:**
  * Kembalikan `("custom".to_string(), None, None)`.

### C. Penyempurnaan Endpoint Mempool (`GET /api/v1/mempool`)
Tambahkan konstanta kapasitas dan ambang batas pasar biaya pada response `MempoolResponse`:
```json
{
  "count": 3,
  "max_count": 5000,
  "total_bytes": 680,
  "max_bytes": 5000000,
  "total_fees_quanta": 12500,
  "min_relay_fee_milli": 1000,
  "transactions": [
    {
      "txid": "0x...",
      "fee_quanta": 5000,
      "size_bytes": 220,
      "fee_rate_milli": 22727,
      "added_time": 1756850000
    }
  ]
}
```

---

## 3. Spesifikasi Tampilan Frontend (`web/explorer/index.html`)

### A. Komponen Mempool & Fee Inspector
1. **Dua Progress Bar Kapasitas Real-Time:**
   * **Slot Transaksi:** `count / 5.000 txs`.
   * **Memori Payload:** `total_bytes / 5.000.000 bytes`.
   * Indikator warna dinamis:
     * Hijau (`emerald`): $< 70\%$
     * Kuning (`amber`): $70\% - 90\%$
     * Merah (`rose`): $> 90\%$
2. **Kartu Ringkasan Biaya:**
   * *Total Pending Fees* (diformat dalam desimal SCY, e.g. `0.00050000 SCY`).
   * *Minimum Relay Fee* (`1.00 q/B`).
3. **Tabel Antrean Prioritas:**
   Daftar transaksi terurut prioritas:
   * Rank (#1, #2, dst).
   * TxID (format pemotongan tengah `0x1a2b...c3d4` dengan copy-to-clipboard).
   * Size (Bytes).
   * Fee (quanta).
   * Fee Density (`(fee_rate_milli / 1000).toFixed(2)` q/B).
   * Status: Badge "High Priority" (hijau) atau "Eviction Risk" (oranye/merah jika mempool $> 90\%$).

### B. Penyajian Alamat Bech32 & OP_RETURN Metadata
1. Alamat pada rincian blok/transaksi ditampilkan sebagai pill Bech32: `scy1qpzr...5ptk` dengan fitur klik untuk menyalin.
2. Label badge `P2PKH` di samping alamat penerima.
3. Transaksi metadata `OP_RETURN` ditampilkan dengan badge ungu `DATA / OP_RETURN` beserta preview teks/hex payload.
4. Klik pada alamat langsung memicu pencarian passbook rekening bersangkutan.

### C. Pencarian Global Universal
1. Input pencarian mendukung:
   * Angka (misal `0`, `42`) $\rightarrow$ Cari Block Height.
   * Hex 64-karakter $\rightarrow$ Cari Block Hash atau TxID.
   * String berawalan `scy1...` atau hex locking script $\rightarrow$ Cari Passbook (`/api/v1/passbook/:lock`).
2. Menampilkan modal rincian saldo terkonfirmasi, selisih tertunda, serta riwayat buku tabungan (*passbook entries*).

---

## 4. Rencana Verifikasi & Uji Mutu

1. **Unit & Integration Tests:**
   * `cargo test -p scytale-node --test http_gateway_tests`
   * Memvalidasi field baru pada `TxOutputDto` (`script_type`, `address`, `op_return_payload`).
   * Memvalidasi metadata kapasitas pada respons `/api/v1/mempool`.
2. **Quality Gates Otomatis:**
   * `cargo fmt --all -- --check`
   * `cargo clippy --workspace --all-targets -- -D warnings`
   * `cargo test --workspace --all-targets`
   * `./scripts/testnet_2node.sh`
   * `./scripts/testnet_fork_reorg.sh`
3. **Uji Fungsional Browser Tersemat:**
   * Melakukan binding lokal `scytale-node --http-bind 127.0.0.1:8332`.
   * Memeriksa endpoint `/` mengembalikan HTML tersemat yang valid dan merender panel inspector dengan benar.
