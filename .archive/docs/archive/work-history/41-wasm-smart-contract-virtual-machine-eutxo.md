# Work Record: 41. ScyVM eUTXO WebAssembly Smart Contract Architecture

## Overview
Implementasi arsitektur smart contract modern berbasis model Extended UTXO (eUTXO) dan mesin eksekusi WebAssembly (`wasmi`) untuk jaringan Scytale Blockchain. Sistem ini memungkinkan validasi transaksi yang deterministik, terisolasi, bebas efek samping reentrancy, dan dilengkapi pembatasan konsumsi gas ketat (*instruction-level fuel metering*).

## Components Implemented
1. **`Cargo.toml` Workspace Expansion**:
   - Menambahkan workspace members `contracts/*`.
   - Mengintegrasikan dependensi `wasmi = "0.31"`, `bincode = { version = "1.3", default-features = false }`, dan `hex = { version = "0.4", default-features = false, features = ["alloc"] }`.
   - Mengonfigurasi profile release khusus target wasm (`opt-level = "z"`, `codegen-units = 1`).

2. **`crates/scytale-sdk` (`#![no_std]`)**:
   - Definisi struktur runtime context `TxContext` (hash transaksi, block time, input amount, fee burned).
   - Helper serialisasi dan deserialisasi biner payload `decode_payload` dan `encode_payload`.
   - Konstanta kode status evaluasi: `VALIDATION_SUCCESS = 1`, `VALIDATION_REJECT = 0`.

3. **`contracts/vault` (Autonomous Timelock Vault)**:
   - Target kompilasi: `wasm32-unknown-unknown` (`cdylib`, `rlib`).
   - Logika evaluasi eUTXO murni `validate(datum, redeemer, context) -> i32`.
   - Mendukung dua alur validasi:
     - `NormalWithdraw`: Memvalidasi batas waktu `unlock_time` dan tanda tangan pemilik `sig_valid`.
     - `EmergencyRescue`: Penarikan darurat sebelum timelock dengan verifikasi pembakaran denda (`fee_burned >= penalty_fee`).

4. **`crates/scytale-vm` (ScyVM Runtime)**:
   - Modul isolasi memori linear WebAssembly berbasis `wasmi::Engine` dan `wasmi::Store`.
   - Metering bahan bakar (*fuel / gas metering*) untuk mencegah serangan *infinite loop*.
   - Penyuntikan `datum`, `redeemer`, dan `TxContext` secara langsung ke memori linear Wasm pada offset deterministik.
   - Eksekusi fungsi `validate` dan pengukuran konsumsi gas riil.

5. **Dokumentasi & Verifikasi**:
   - Spesifikasi arsitektur komprehensif di [docs/SMART_CONTRACTS.md](file:///mnt/ssd/scytale-lab/scytale/docs/SMART_CONTRACTS.md).
   - Unit & integration test `tests::test_vault_eutxo_lifecycle` yang menguji kompilasi wasm otomatis, skenario penolakan sebelum unlock time, dan skenario kelulusan transaksi sah dengan perhitungan gas.

## Verification Results
- `cargo build --target wasm32-unknown-unknown --release -p scytale-contract-vault`: Berhasil menghasilkan biner WebAssembly `scytale_contract_vault.wasm` (69 KB).
- `cargo test -p scytale-vm -- --nocapture`: Lulus 100% (`1 passed; 0 failed`).
- `cargo check --workspace`: Seluruh crate workspace terkompilasi bersih tanpa error.
