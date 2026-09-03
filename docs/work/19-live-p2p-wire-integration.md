# Scytale Protocol — Technical Specification & Architecture Record: Task 19
## Live P2P Wire Integration & Go Daemon Process Supervisor

```text
Document ID   : SPEC-TASK-19
Task ID       : 19
Task Name     : Live P2P Wire Integration & Go Daemon Process Supervisor
Phase         : Phase 2 — Chaos, Cluster & Observability
Target Crates : apps/scytale-node, crates/scytale-bridge, network/cmd/scytale-p2p
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Subprocess Supervision, Length-Prefixed Framing, Zero-Loss Gossip, Fail-Closed IPC
Quality Gates : 100% Rust Tests PASS | 100% Go Race Tests PASS | Live 2-Node P2P PASS
```

---

## 1. Problem Statement

Sebelum Task 19, mesin konsensus (`scytale-node` dalam bahasa Rust) dan daemon protokol *wire* (`scytale-p2p` dalam bahasa Go) berjalan secara terisolasi. Operator node diharuskan menjalankan dua proses independen secara manual, mengoordinasikan jalur soket Unix, dan menangani pemulihan *crash* secara manual.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Child Process Supervisor (`apps/scytale-node/src/node.rs`)
- `scytale-node` diperluas untuk mengawasi siklus hidup daemon Go via pemanggilan proses standar (`std::process::Command`).
- Deteksi otomatis binary terkompilasi di `target/debug`, `target/release`, atau direktori `network/cmd/scytale-p2p`.
- Propagasi sinyal: Shutdown anggun (`SIGTERM`) dan penangkapan (*reaping*) child process saat node berhenti.

### 2.2 Bi-Directional IPC Socket Bridge (`crates/scytale-bridge`)
- Unix Domain Socket (`/tmp/scytale_p2p_bridge.sock`) menggunakan framing codec dengan panjang prefix 4-byte big-endian.
- Rust Core bertindak sebagai server IPC; daemon Go terhubung saat inisialisasi.

### 2.3 Wire Event Multiplexing
- `P2pBridgeEvent::BroadcastTransaction`: Menyebarkan transaksi mempool yang baru diterima ke daemon Go untuk *wire gossip*.
- `P2pBridgeEvent::BroadcastBlock`: Mentransmisikan blok kanonikal baru yang berhasil ditambang atau diterima.
- `P2pBridgeEvent::IngressTransaction` & `P2pBridgeEvent::IngressBlock`: Menerima entitas yang diumumkan oleh peer, memvalidasi header/skrip, dan memasukkannya ke mempool atau chain tree.
- Alur Gossip Dua Tahap: `INV` $\rightarrow$ `GETDATA` $\rightarrow$ `BLOCK` / `TX`.

### 2.4 Ekstensi Argumen Baris Perintah (CLI)
- `--p2p-bind <ADDR>`: Pengikatan TCP lokal untuk koneksi peer (contoh: `0.0.0.0:9001`).
- `--p2p-peer <ADDR>`: Alamat peer bootstrap awal.
- `--no-p2p`: Bypass penuh untuk pengujian unit lokal satu node terisolasi.

---

## 3. Hasil Pengujian & Verifikasi

Diverifikasi melalui skrip otomatis `scripts/testnet_2node.sh`:
- Node 1 menambang blok pada port 9001.
- Node 2 terhubung pada port 9002.
- Seluruh blok tersinkronisasi 100% secara instan melalui sambungan wire P2P live.
