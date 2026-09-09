# Scytale Protocol — Technical Specification & Architecture Record: Task 20
## Ergonomic Wallet & Identity Management in `scytale-cli`

```text
Document ID   : SPEC-TASK-20
Task ID       : 20
Task Name     : Ergonomic Wallet & Identity Management in scytale-cli
Phase         : Phase 2 — Chaos, Cluster & Observability
Target Crates : apps/scytale-cli
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Non-Custodial Storage, POSIX 0600 Permissions, Atomic Active Switch, Zero-Friction Inference
Quality Gates : 100% Rust Tests PASS | 0 Warnings Clippy
```

---

## 1. Problem Statement

Sebelum Task 20, transaksi, kueri saldo, dan perintah penambangan membutuhkan entri manual kondisi penguncian heksadesimal 32-byte mentah (`--lock 010203...`). Hal ini menimbulkan friksi berlebihan dan memperbesar resiko kesalahan operator.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Modul Registri Identitas (`apps/scytale-cli/src/identity.rs`)
- Registri lokal non-kustodial tersimpan di `~/.scytale/identities.json` dengan izin berkas POSIX ketat `0600`.
- Auto-bootstrapping: Membuat akun `default` pada eksekusi pertama jika profil belum ada.
- Model Akun:
  ```rust
  pub struct AccountRecord {
      pub alias: String,
      pub secret_key_hex: String,
      pub locking_script_hex: String,
      pub created_at_epoch: u64,
  }
  ```

### 2.2 Subperintah CLI Ergonomis
- `scytale-cli account list`: Menampilkan seluruh akun terdaftar, menandai akun aktif dengan tanda `*`.
- `scytale-cli account new <ALIAS>`: Menghasilkan pasangan kunci kriptografi baru.
- `scytale-cli account switch <ALIAS>`: Mengganti akun aktif secara atomik.
- `scytale-cli account show [<ALIAS>]`: Menampilkan metadata detail akun.

### 2.3 Inferensi Perintah Tanpa Friksi (Zero-Friction Command Inferences)
- `scytale-cli balance`, `scytale-cli passbook`, `scytale-cli send`, dan `scytale-cli mine` secara otomatis menginferensikan identitas akun aktif ketika flag `--lock`, `--account`, atau `--from` tidak dicantumkan secara eksplisit.
