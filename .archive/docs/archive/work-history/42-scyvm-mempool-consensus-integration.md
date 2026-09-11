# Work Record: 42. ScyVM Mempool & Consensus Integration

## Overview
Integrasi mesin eksekusi WebAssembly ScyVM ke dalam dua titik validasi kritis jaringan Scytale:
1. **Gerbang penerimaan Mempool** (`submit_transaction`) — validasi eUTXO sebelum transaksi diantrikan untuk penambangan.
2. **Pipeline validasi Blok** (`submit_external_block`) — validasi eUTXO secara deterministik menggunakan `block.header.timestamp` sebagai konteks waktu, dengan akumulasi gas per blok.

## Components Modified / Added

### `crates/scytale-core/Cargo.toml`
Tambahan dependensi:
- `bincode = { workspace = true }`
- `hex = { workspace = true }`
- `scytale-sdk = { path = "../scytale-sdk", features = ["std"] }`
- `scytale-vm = { path = "../scytale-vm" }`

### `crates/scytale-core/src/transaction.rs`
Penambahan tipe data eUTXO:
- `OutputLock` — enum spending condition: `PublicKey([u8; 32])` vs `Script { script_hash, datum }`.
- `TxOutput` — high-level output wrapper dengan `OutputLock` dan konversi ke/dari `TxOut`.
- `TxInput` — high-level input wrapper dengan `signature`, `redeemer`, `script_source`. Mendukung konversi ke/dari `TxIn` canonical melalui `to_tx_in()` / `from_tx_in()`.
- `EutxoWitness` — witness struct yang diserialisasi ke dalam field `authorization` pada `TxIn`.
- `Transaction::compute_hash()` — mengekspor raw 32-byte BLAKE3 hash.
- `Transaction::from_eutxo()` — constructor helper dari eUTXO models.

Semua `OutputLock` dan `EutxoWitness` menggunakan **magic prefix** (4 byte) untuk identifikasi deterministik:
- `OutputLock::MAGIC_PREFIX = [0x53, 0x43, 0x59, 0x01]` ("SCY\x01")
- `TxInput::MAGIC_PREFIX = [0x53, 0x43, 0x59, 0x02]` ("SCY\x02")

### `crates/scytale-core/src/vm_adapter.rs` [NEW]
Modul adapter yang menghubungkan `scytale-vm` dengan model transaksi `scytale-core`:
- `create_tx_context(tx, block_time, total_in, total_out) -> TxContext` — membangun konteks evaluasi deterministik.
- `verify_transaction_eutxo(tx, block_time, utxos, gas_limit) -> Result<u64, EutxoValidationError>` — mengevaluasi semua input `OutputLock::Script` dalam satu transaksi:
  1. Verifikasi BLAKE3 hash bytecode Wasm vs `script_hash` pada UTXO.
  2. Eksekusi `ScyVM::execute_validator(wasm, datum, redeemer, &ctx, gas)`.
  3. Tolak jika `!is_valid`, execution trap, atau gas melebihi `MAX_TX_GAS`.
- `EutxoValidationError` — error taxonomy lengkap: `MissingUtxo`, `MissingScriptSource`, `MissingRedeemer`, `ScriptHashMismatch`, `VmExecutionFailed`, `ValidationRejected`, `GasLimitExceeded`, `BlockGasLimitExceeded`.
- Konstanta: `MAX_TX_GAS = 5_000_000` fuel, `MAX_BLOCK_GAS = 50_000_000` fuel.

### `crates/scytale-core/src/lib.rs`
Re-export `vm_adapter` module dan semua tipe eUTXO baru.

### `apps/scytale-node/src/node.rs`
**Mempool Gate (`submit_transaction`)**:
```rust
verify_transaction_eutxo(&tx, now, &utxos, MAX_TX_GAS)
    .map_err(NodeError::EutxoValidation)?;
```
Dipanggil sebelum `mempool.admit_transaction()` — menggunakan Unix timestamp saat ini sebagai proxy `block_time`.

**Block Validation (`submit_external_block`)**:
```rust
let tx_gas = verify_transaction_eutxo(tx, block_time, &staging_utxos, MAX_TX_GAS)
    .map_err(NodeError::EutxoValidation)?;
block_gas_consumed = block_gas_consumed.saturating_add(tx_gas);
if block_gas_consumed > MAX_BLOCK_GAS { return Err(...); }
```
Dipanggil untuk setiap transaksi non-coinbase yang memperluas canonical tip, menggunakan `block.header.timestamp` sebagai `block_time` yang deterministik.

### `apps/scytale-node/src/error.rs`
Tambahan variant `EutxoValidation(#[from] scytale_core::EutxoValidationError)` pada `NodeError`.

### `apps/scytale-node/Cargo.toml`
Tambahan dependensi: `scytale-sdk`, `scytale-vm`, `bincode`.

### `apps/scytale-node/tests/eutxo_validation_tests.rs` [NEW]
Suite pengujian integrasi end-to-end dengan 5 test case:

| Test | Skenario | Hasil |
|------|---------|-------|
| `test_vault_rejected_before_unlock_time` | `block_time = 1.7B < unlock_time = 1.8B` | `ValidationRejected` ✓ |
| `test_vault_accepted_after_unlock_time` | `block_time = 1.85B > unlock_time = 1.8B` | `Ok(22092 fuel)` ✓ |
| `test_vault_gas_limit_exceeded` | gas_limit = 1 | `VmExecutionFailed(ExecutionTrapped)` ✓ |
| `test_vault_script_hash_mismatch` | hash UTXO salah | `ScriptHashMismatch` ✓ |
| `test_standard_pkh_transaction_unaffected` | Standard P2PK input | `Ok(0 gas)` ✓ |

## Verification Results

### Integration Tests (5 tests):
```
running 5 tests
[OK] Standard P2PK transaction passes eUTXO gate with 0 gas.
test test_standard_pkh_transaction_unaffected ... ok
[OK] Script hash mismatch correctly detected.
test test_vault_script_hash_mismatch ... ok
[OK] Gas limit enforcement passed: VmExecutionFailed(ExecutionTrapped)
test test_vault_gas_limit_exceeded ... ok
test test_vault_rejected_before_unlock_time ... ok
[OK] Vault accepted. Gas consumed: 22092 fuel
test test_vault_accepted_after_unlock_time ... ok

test result: ok. 5 passed; 0 failed; finished in 0.08s
```

### Full Workspace:
`cargo check --workspace` → bersih, tanpa error/warning regresi.
