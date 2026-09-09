# Scytale Protocol — Technical Specification & Architecture Record: Task 22
## Multi-Node Dockerized Cluster with docker-compose

```text
Document ID   : SPEC-TASK-22
Task ID       : 22
Task Name     : Multi-Node Dockerized Cluster with docker-compose
Phase         : Phase 2 — Chaos, Cluster & Observability
Target Files  : Dockerfile, docker-compose.yml, network/cmd/scytale-p2p
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Multi-Stage Minimal Runtime, DNS Peer Resolution, Port Isolation, Zero Build-Tool Leakage
Quality Gates : Docker Build Success | 3-Node Virtual Cluster Verification
```

---

## 1. Problem Statement

Penyebaran dan orkestrasi kluster multi-node lokal untuk simulasi pengujian membutuhkan instalasi manual Rust, Go, pustaka C, dan ketergantungan sistem lainnya, yang memperbesar resiko deviasi konfigurasi antar-lingkungan (*environmental drift*).

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Multi-Stage Production Dockerfile (`Dockerfile`)
- **Stage 1 (Rust Builder):** Menggunakan `rust:1.80-slim-bullseye` untuk mengompilasi `scytale-node` dan `scytale-cli`.
- **Stage 2 (Go Builder):** Menggunakan `golang:1.22-bullseye` untuk mengompilasi `scytale-p2p`.
- **Stage 3 (Runtime Minimal):** Menggunakan `debian:bullseye-slim` yang hanya memuat binary terkompilasi, sertifikat TLS CA, dan pustaka runtime bersama (`libssl`, `libc`), menghasilkan image produksi yang ramping dan aman.

### 2.2 DNS & Hostname Resolution
- Daemon P2P Go dan IPC bridge Rust ditingkatkan untuk mendukung resolusi nama domain / hostname Docker (contoh: `node1:9001`) pada jaringan *bridge* container.

### 2.3 Topologi 3-Node Cluster (`docker-compose.yml`)
- `node1` (Bootstrap Node): Mengekspos P2P 9001 dan HTTP 8332.
- `node2` (Miner Node): Terhubung ke `node1:9001`, mengeksekusi penambangan PoW di latar belakang.
- `node3` (Follower/Explorer Node): Terhubung ke `node1:9001`, menyajikan antarmuka baca HTTP API pada port 8334.
