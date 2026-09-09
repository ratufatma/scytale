# Work Record: 43. `scytale-cli contract` Smart Contract Developer Tooling

## Overview
Implementasi sub-perintah `contract` pada `scytale-cli` sebagai developer tooling untuk pengembang smart contract berbasis WebAssembly (ScyVM). Tooling ini menyediakan antarmuka CLI yang user-friendly untuk seluruh siklus pengembangan kontrak eUTXO: inspeksi biner, kompilasi, pembuatan locking transaction (deploy), dan eksekusi transaksi pembelanjaan (call) dengan simulasi ScyVM sandbox otomatis.

## Komponen yang Diimplementasikan

### `apps/scytale-cli/src/contract.rs` [NEW]

Modul baru yang mengimplementasikan 4 subcommand:

#### 1. `contract inspect --wasm <file.wasm>`
- Memvalidasi magic number Wasm (`\0asm`, versi 1)
- Menghitung dan menampilkan **BLAKE3 `script_hash`** (32 bytes / 64 hex chars)
- Menampilkan ukuran biner (bytes dan KiB)
- Output siap untuk digunakan pada perintah `deploy`

```bash
$ scytale-cli contract inspect --wasm contracts/vault.wasm

╔══════════════════════════════════════════════════════════════╗
║             SCYTALE CONTRACT INSPECTOR                      ║
╚══════════════════════════════════════════════════════════════╝
  File       : contracts/vault.wasm
  Size       : 69432 bytes (67.80 KiB)
  ScriptHash : a3b4c5d6e7f8...
  Wasm Magic  : ✓ valid (\0asm)
  Wasm Version: 1
```

#### 2. `contract build [--path <dir>] [--package <name>]`
- Menjalankan `cargo build --release --target wasm32-unknown-unknown` di direktori crate
- Mendukung flag `-p <crate_name>` untuk workspace multi-crate
- Output berupa informasi lokasi artefak dan langkah selanjutnya

#### 3. `contract deploy --wasm <file> --amount <quanta> --datum <hex>`
- Membaca biner Wasm dan menghitung `script_hash` via BLAKE3
- Membangun `OutputLock::Script { script_hash, datum }` dan mengonversinya ke `TxOut`
- Menampilkan preview struktur transaksi penguncian sebelum broadcast
- Menampilkan ukuran `locking_condition` bytes yang akan tertulis on-chain
- Saat ini dalam mode **preview** (broadcast via IPC node akan diintegrasikan di work berikutnya)

#### 4. `contract call --utxo <txhash:idx> --wasm <file> --redeemer <hex> --datum <hex> --to <hex>`
- Mem-parse referensi UTXO dalam format `<tx_hash_hex>:<output_index>`
- Membangun transaksi pembelanjaan lengkap menggunakan `TxInput` (eUTXO witness)
- **Dry-Run otomatis via ScyVM sandbox** sebelum broadcast:
  - Memanggil `ScyVM::execute_validator(wasm, datum, redeemer, ctx, MAX_TX_GAS)`
  - Menggunakan Unix timestamp saat ini sebagai `block_time`
  - Menampilkan gas fuel yang dikonsumsi
  - Menghentikan eksekusi jika kontrak mengembalikan `VALIDATION_REJECT`
- Dapat dilewati dengan `--skip-dry-run` (untuk kasus darurat/debug)

### `apps/scytale-cli/Cargo.toml`
Tambahan dependensi:
- `scytale-sdk = { path = "../../crates/scytale-sdk", features = ["std"] }`
- `scytale-vm = { path = "../../crates/scytale-vm" }`
- `bincode = { workspace = true }`
- `hex = { workspace = true }`

### `apps/scytale-cli/src/main.rs`
- Deklarasi `pub mod contract` dan `use contract::{ContractArgs, handle_contract}`
- Tambahan variant `Commands::Contract(ContractArgs)` di enum `Commands`
- Dispatch arm `Commands::Contract(args) => handle_contract(args)?` di fungsi `execute()`

## Suite Pengujian Unit (7 tests)

| Test | Deskripsi |
|------|-----------|
| `test_inspect_minimal_wasm` | `inspect` berhasil pada file wasm valid |
| `test_inspect_missing_file` | `inspect` gagal dengan `CliClientError::User` jika file tidak ada |
| `test_inspect_blake3_hash_is_deterministic` | Hash BLAKE3 deterministik (64 hex chars) |
| `test_inspect_hash_changes_with_content` | Hash berbeda untuk konten berbeda |
| `test_call_utxo_parse_valid` | Parse UTXO format `<txhash>:<idx>` valid |
| `test_call_utxo_parse_invalid` | Format UTXO tanpa `:` terdeteksi salah |
| `test_output_lock_script_round_trip` | `OutputLock::Script` → locking_condition → kembali ke `OutputLock::Script` (lossless) |
| `test_deploy_computes_correct_script_hash` | Hash Wasm di deploy konsisten dengan blake3::hash |

## Hasil Verifikasi

### Unit & Integration Tests (30 total):
```
scytale-cli unit tests  : 13 passed / 0 failed
cli_tests               : 10 passed / 0 failed
wallet_p2pkh_tests      :  7 passed / 0 failed
─────────────────────────────────────────────
Total                   : 30 passed / 0 failed
```

### Workspace Check:
```
cargo check --workspace → Finished (0 errors, 0 warnings)
```

## Contoh Workflow Lengkap

```bash
# 1. Kompilasi kontrak
scytale-cli contract build --path /mnt/ssd/scytale-lab/scytale/contracts/vault

# 2. Inspeksi dan dapatkan script_hash
scytale-cli contract inspect \
  --wasm target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm

# 3. Buat datum (gunakan bincode encoding dari VaultDatum, hasil di hex)
DATUM_HEX="0000000000000000000000000000000000000000000000000000000000000000\
00f28d0600000000000000000000000000000000000000000000000000000000\
0000000000000000e803000000000000"

# 4. Deploy: lock 5 SCY ke kontrak
scytale-cli contract deploy \
  --wasm target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm \
  --amount 500000000 \
  --datum "$DATUM_HEX"

# 5. Call: belanjakan UTXO setelah timelock expired
REDEEMER_HEX="0001"   # VaultRedeemer::NormalWithdraw { sig_valid: true }

scytale-cli contract call \
  --utxo "abc123...def456:0" \
  --wasm target/wasm32-unknown-unknown/release/scytale_contract_vault.wasm \
  --redeemer "$REDEEMER_HEX" \
  --datum "$DATUM_HEX" \
  --to "deadbeef..." \
  --amount 499000000 \
  --fee 1000
# Output: [✓] DRY-RUN PASSED! Gas consumed: 22092 fuel
```
