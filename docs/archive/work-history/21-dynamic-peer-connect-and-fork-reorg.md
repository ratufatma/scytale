# Scytale Protocol — Technical Specification & Architecture Record: Task 21
## Dynamic Peer Connect & Live Fork Reorganization Harness

```text
Document ID   : SPEC-TASK-21
Task ID       : 21
Task Name     : Dynamic Peer Connect & Live Fork Reorganization Harness
Phase         : Phase 2 — Chaos, Cluster & Observability
Target Crates : crates/scytale-storage, crates/scytale-consensus, apps/scytale-node, apps/scytale-cli
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Atomic Rollback/Rollforward, Lowest Common Ancestor (LCA), Zero Dirty Reads, Live Reorg
Quality Gates : 100% Rust Tests PASS | Live Reorg Shell Harness PASS (scripts/testnet_fork_reorg.sh)
```

---

## 1. Problem Statement

Jaringan blockchain harus tangguh terhadap partisi jaringan sementara, mampu mendeteksi cabang rantai yang memiliki bobot kerja kumulatif (*cumulative work*) lebih berat saat konektivitas pulih, dan membatalkan state blok usang secara atomik tanpa merusak integritas buku besar.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Runtime Dynamic Peer Connection
- Pesan IPC `NodeRequest::ConnectPeer { addr }` diimplementasikan pada `scytale-bridge`.
- Perintah CLI `scytale-cli peer connect <ADDR>`.
- Node memicu `P2pBridgeEvent::ConnectPeer`, menginstruksikan daemon Go untuk mendial peer jarak jauh secara runtime tanpa perlu mematikan atau me-restart daemon.

### 2.2 Atomic Rollback & Reorganization (`crates/scytale-storage` & `apps/scytale-node`)
- `ChainTree::extend_or_reorganize` menemukan Lowest Common Ancestor (LCA).
- Pada `StorageEngine::apply_reorganization`:
  - Blok cabang yang terputus (*disconnected blocks*) dihapus dari indeks kanonikal; UTXO yang sempat dibelanjakan dikembalikan ke tabel `UTXOS` di `redb`.
  - Blok cabang yang menang (*connected blocks*) dimasukkan ke indeks kanonikal; input dibelanjakan dihapus dan output baru dimasukkan.
  - Seluruh operasi dilakukan di dalam satu `redb::WriteTransaction` untuk menjamin konsistensi ACID.
- Penanganan Mempool saat Reorganisasi: Transaksi yang bentrok digusur, dan transaksi sah dari blok terputus dikembalikan ke status *pending* di mempool.
- Rekonsiliasi Passbook: Transaksi yang terputus berubah status dari `Confirmed` menjadi `Reorganized`, dan saldo terkonfirmasi diselaraskan secara akurat.

---

## 3. Hasil Pengujian & Verifikasi

Diverifikasi secara otomatis melalui `scripts/testnet_fork_reorg.sh`:
- Node 1 menambang 5 blok di Partisi A.
- Node 2 menambang 21 blok di Partisi B.
- Perintah `scytale-cli peer connect` dijalankan.
- Node 1 mengunduh 21 blok via IBD, membatalkan cabang 5 blok lama, mengadopsi rantai Node 2 di Height 21, dan menyelaraskan Passbook secara instan.
