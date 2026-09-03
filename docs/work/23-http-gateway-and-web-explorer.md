# Scytale Protocol — Technical Specification & Architecture Record: Task 23
## Lightweight HTTP / JSON-RPC Read-Only Gateway & Embedded Web Explorer

```text
Document ID   : SPEC-TASK-23
Task ID       : 23
Task Name     : Lightweight HTTP / JSON-RPC Read-Only Gateway & Embedded Web Explorer
Phase         : Phase 2 — Chaos, Cluster & Observability
Target Crates : apps/scytale-node, web/explorer
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Zero-Dependency Embedded Single Binary, Read-Only Safety, CORS Enabled, Axum Async
Quality Gates : 100% HTTP Gateway Tests PASS | Embedded Web Interface Serves 200 OK
```

---

## 1. Problem Statement

Aplikasi eksternal, penjelajah blok (*block explorer*), dan sistem pemantauan membutuhkan akses inspeksi ke state blockchain secara mudah melalui protokol web standar (HTTP/JSON) tanpa harus mengakses soket Unix IPC lokal.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Async HTTP Gateway (`apps/scytale-node/src/http_gateway.rs`)
- Dibangun menggunakan `axum` 0.7 dan middleware `tower-http` dengan dukungan Cross-Origin Resource Sharing (CORS).
- Konfigurasi fleksibel via argumen baris perintah `--http-bind <IP:PORT>` (default: `0.0.0.0:8332`) dan flag `--no-http` untuk menonaktifkan server.

### 2.2 Endpoint REST API
- `GET /api/v1/status`: Metadata status node, hash tip rantai kanonikal, ketinggian blok, dan ukuran mempool.
- `GET /api/v1/blocks/tip`: Detail header blok kanonikal tertinggi.
- `GET /api/v1/blocks/:hash_or_height`: Data detail blok beserta seluruh muatan transaksi.
- `GET /api/v1/tx/:txid`: Pencarian transaksi beserta dekomposisi input/output.
- `GET /api/v1/passbook/:lock_hex`: Buku tabungan kanonikal (*Passbook ledger*) untuk skrip penguncian yang ditentukan.
- `GET /api/v1/provenance/:txid/:index`: Penelusuran silsilah DAG koin dari titik genesis atau penambangan awal.
- `GET /health`: Pemeriksaan liveness & readiness (HTTP 200 OK).

### 2.3 Embedded Web Explorer (`web/explorer/index.html`)
- Single Page Application (SPA) modern tanpa dependensi eksternal (*zero-dependency*), di-embed langsung ke segmen biner `scytale-node` saat waktu kompilasi melalui makro `include_str!`.
- Disajikan pada rute `GET /` dan `GET /index.html` dengan header `Content-Type: text/html; charset=utf-8`.
- Dilengkapi pencarian universal, visualisasi transaksi, dan penjelajah rantai waktu nyata.
