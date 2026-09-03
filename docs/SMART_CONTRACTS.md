# Scytale Smart Contract & Virtual Machine Architecture (ScyVM)

Dokumentasi spesifikasi sistem smart contract terdesentralisasi Scytale berbasis **eUTXO (Extended Unspent Transaction Output)** dan WebAssembly Sandbox.

---

## 1. Konsep Arsitektur: eUTXO vs Account Model

Berbeda dari model EVM (Ethereum) yang menyimpan state global bersama (*shared global mutable state*) yang rentan terhadap serangan Reentrancy, Scytale menerapkan model matematis murni:

$$\text{Validator}(Datum, Redeemer, TxContext) \to \text{Valid} \mid \text{Invalid}$$

```text
  [Input UTXO]                                           [Output UTXO]
  ┌─────────────────────────┐                            ┌─────────────────────────┐
  │ Nilai: 50.00 SCY        │   +------------------+     │ Nilai: 49.95 SCY        │
  │ Datum: {Unlock, Key}    │──►│  ScyVM (Wasmi)   │────►│ Penerima: Alamat Baru   │
  │ Redeemer: {Signature}   │   │  Eksekusi Wasm   │     │ Datum: None / Hash Baru │
  └─────────────────────────┘   +------------------+     └─────────────────────────┘
                                         │
                             (Biaya Gas Terukur / Opcode)
```

1. **Datum:** State terenkapsulasi yang terkunci di dalam UTXO.
2. **Redeemer:** Argumen/input yang disediakan oleh pembelanja untuk memenuhi syarat validasi.
3. **TxContext:** Metadata transaksi dan blok (block height, block time, fee, hash) yang disuntikkan oleh node saat validasi.

---

## 2. Struktur Workspace Crate

* `crates/scytale-sdk`: Definisi struktur dasar `TxContext`, helper serialisasi biner, dan kode status evaluasi. Pustaka ini murni `#![no_std]` untuk mendukung target Wasm ringkas.
* `crates/scytale-vm`: Runtime berbasis `wasmi` yang mengisolasi eksekusi bytecode `.wasm`, menghitung konsumsi gas, dan menyuntikkan memori linear secara deterministik.
* `contracts/*`: Direktori khusus logika smart contract mandiri. Kontrak dikompilasi menjadi library `.wasm` dinamis tanpa dependensi sistem operasi.

---

## 3. Menulis dan Mengompilasi Kontrak Baru

Setiap smart contract adalah pustaka dinamis Rust no-std yang mengekspor fungsi tunggal `validate`:

```bash
# Kompilasi kontrak ke WebAssembly
cargo build --target wasm32-unknown-unknown --release -p <nama-crate-kontrak>
```

Hasil biner berekstensi `.wasm` yang sangat kecil (< 30 KB) dapat disimpan hash-nya di dalam UTXO pada ledger Scytale.

---

## 4. Pengujian & Verifikasi VM

Untuk menjalankan suite pengujian simulasi smart contract:

```bash
cargo test -p scytale-vm -- --nocapture
```

---

## 5. Developer CLI Tooling (`scytale-cli contract`)

Mulai dari Work #43, `scytale-cli` menyediakan sub-perintah `contract` yang terintegrasi dengan ScyVM untuk memudahkan siklus pengembangan kontrak.

### 5.1 Inspect — Tampilkan Script Hash & Metadata Biner

```bash
scytale-cli contract inspect --wasm <path/ke/contract.wasm>
```

Output mencakup:
- BLAKE3 `script_hash` (32 bytes, format hex) — digunakan sebagai identifier kontrak dalam `OutputLock::Script`
- Ukuran biner (bytes dan KiB)
- Validasi magic number Wasm dan versi

### 5.2 Build — Kompilasi Kontrak

```bash
# Kompilasi crate kontrak saat ini
scytale-cli contract build

# Kompilasi package spesifik dalam workspace
scytale-cli contract build --package scytale-contract-vault
```

### 5.3 Deploy — Kunci Dana ke Script UTXO

```bash
# DATUM_HEX: bincode hex dari struct Datum kontrak (misal VaultDatum)
scytale-cli contract deploy \
  --wasm target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm \
  --amount 500000000 \
  --datum "$DATUM_HEX"
```

Perintah ini membangun `OutputLock::Script { script_hash, datum }` dan menampilkan preview transaksi sebelum broadcast.

### 5.4 Call — Belanjakan UTXO dengan Dry-Run Otomatis

```bash
# REDEEMER_HEX: bincode hex dari struct Redeemer (misal VaultRedeemer)
scytale-cli contract call \
  --utxo "<txid_hex>:<output_index>" \
  --wasm target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm \
  --redeemer "$REDEEMER_HEX" \
  --datum "$DATUM_HEX" \
  --to "<recipient_locking_script_hex>" \
  --amount 499000000 \
  --fee 1000
```

**Dry-Run berjalan otomatis sebelum broadcast:**
- Memanggil `ScyVM::execute_validator` dalam sandbox lokal
- Menggunakan Unix timestamp saat ini sebagai `block_time`
- Transaksi dibatalkan jika kontrak mengembalikan `VALIDATION_REJECT`
- Menampilkan gas fuel yang dikonsumsi

Untuk melewati dry-run (tidak disarankan): tambahkan `--skip-dry-run`.

### 5.5 Alur Kerja Lengkap

```text
┌─────────────┐   ┌─────────────┐   ┌───────────────┐   ┌──────────────┐
│   contract  │   │   contract  │   │   contract    │   │   contract   │
│    build    │──►│   inspect   │──►│    deploy     │──►│    call      │
│  (kompilasi)│   │ (hash+size) │   │ (lock UTXO)  │   │ (dry-run+tx) │
└─────────────┘   └─────────────┘   └───────────────┘   └──────────────┘
```
