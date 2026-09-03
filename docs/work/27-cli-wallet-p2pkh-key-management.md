# Scytale Protocol — Technical Specification & Architecture Record: Task 27
## CLI Wallet & P2PKH Key Management in `apps/scytale-cli`

```text
Document ID   : SPEC-TASK-27
Task ID       : 27
Task Name     : CLI Wallet & P2PKH Key Management in apps/scytale-cli
Phase         : Phase 3 — User-Facing Protocol & Client Tooling
Target Crates : apps/scytale-cli, crates/scytale-bridge
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Non-Custodial Key Storage, Client-Side Signing, Greedy Coin Selection, Data Embedding
Quality Gates : 100% Rust Tests PASS | End-to-End P2PKH Transfer & Data Embed PASS
```

---

## 1. Problem Statement

Pengguna akhir dan pengembang memerlukan dompet mandiri non-kustodial (*self-sovereign client wallet*) untuk menghasilkan pasangan kunci Ed25519, menurunkan alamat Pay-to-Public-Key-Hash (P2PKH), menandatangani transaksi secara offline di sisi klien, dan menyematkan data arbitrer ke dalam rantai blok.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Modul Dompet Non-Kustodial (`apps/scytale-cli/src/wallet.rs`)
- Berkas dompet `~/.scytale/wallet.json` dilindungi hak akses POSIX `0600`.
- Memuat seed rahasia Ed25519 (32 byte hex), kunci publik (32 byte hex), dan alamat P2PKH (`BLAKE3(PublicKey)` 32 byte hex).

### 2.2 Format Skrip P2PKH Standar
- **Skrip Penguncian (ScriptPubKey):**
  `OP_DUP OP_BLAKE3 <32-byte Address> OP_EQUALVERIFY OP_CHECKSIG`
- **Skrip Pembukaan (ScriptSig):**
  `<64-byte Ed25519 Signature> <32-byte Public Key>`

### 2.3 Protokol IPC Node Tambahan (`crates/scytale-bridge`)
- `NodeRequest::GetUtxosByLock { locking_script }` $\rightarrow$ Mengembalikan daftar UTXO yang dapat dibelanjakan.
- `NodeRequest::SubmitRawTransaction { tx }` $\rightarrow$ Mengirimkan transaksi yang telah ditandatangani ke node.

### 2.4 Perintah CLI Dompet
- `scytale-cli wallet new`: Menghasilkan dompet baru.
- `scytale-cli wallet info`: Menampilkan alamat dan kueri saldo aktif.
- `scytale-cli transfer-p2pkh --to <addr> --amount <quanta> [--fee <quanta>]`: Algoritma coin selection tamak (*greedy coin selection*), penandatanganan sighash lokal, dan penyiaran transaksi.
- `scytale-cli embed-data --data <hex_or_string> [--fee <quanta>]`: Menyematkan data hingga 80 byte ke rantai blok menggunakan output `OP_RETURN` bernilai 0.
