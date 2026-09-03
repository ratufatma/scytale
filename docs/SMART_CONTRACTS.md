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
