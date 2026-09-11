# Dokumen Spesifikasi Teknis — Task 34

```text
Task ID       : 34
Task Name     : Multi-Node Docker Cluster Chaos & Fast Sync End-to-End Stress Test
Phase         : Phase 3 — Protocol Hardening & System Verification
Target Files  : docker-compose.yml, Dockerfile, scripts/chaos_stress_test.sh, docs/work/34_multi_node_docker_chaos_and_fast_sync
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Zero Panic, Zero Data Race, State Convergence, Strict Merkle Root Match, Memory Bounded (< 512 MB/node)
```

---

## 1. Arsitektur & Topologi Uji Chaos Multi-Node

Task 34 memvalidasi seluruh subsistem protokol Scytale yang telah dibangun pada Task 28 hingga Task 33 dalam sebuah lingkungan kluster terisolasi menggunakan Docker Compose. Pengujian ini menggabungkan penemuan jaringan dinamis, persaingan pasar biaya, ketahanan terhadap partisi jaringan (*network split-brain*), dan pembuktian sinkronisasi kilat (*Fast Sync*) secara *end-to-end*.

```text
                                  ┌────────────────────────┐
                                  │      scytale-net       │
                                  │   (Docker Bridge Subnet)│
                                  │       172.28.0.0/16    │
                                  └───────────┬────────────┘
                                              │
            ┌─────────────────────────────────┼─────────────────────────────────┐
            │                                 │                                 │
     ┌──────▼──────┐                   ┌──────▼──────┐                   ┌──────▼──────┐
     │   node-1    │                   │   node-2    │                   │   node-3    │
     │  172.28.0.10│◄─────────────────►│  172.28.0.20│◄ - - - - - - - - -│  172.28.0.30│
     │ (Miner PoW) │   Wire P2P Mesh   │(Relay/Mempool│   Dynamic Mesh   │ (Chaos Test │
     │  9001/8332  │                   │  9002/8333  │    Discovery     │  Partition) │
     └─────────────┘                   └──────┬──────┘                   └─────────────┘
                                              │ Fast Sync Snapshot
                                              │ (getsnapshot/snapshot)
                                              ▼
                                       ┌─────────────┐
                                       │   node-4    │
                                       │ 172.28.0.40 │
                                       │ (Fast Sync) │
                                       │  9004/8335  │
                                       └─────────────┘
```

---

## 2. Empat Skenario Uji Chaos (Execution Scenarios)

### Skenario 1: Autonomous Mesh Discovery & Bech32 Handshake
* `node-1` (Miner) dan `node-2` (Relay) dimulai dalam jaringan `scytale-net`.
* `node-3` hanya diberikan opsi peer `--peer 172.28.0.10:9001` (hanya terhubung ke `node-1`).
* **Verifikasi:** Melalui pertukaran pesan `getaddr` dan `addr` pada lapisan wire P2P, `node-3` secara otonom menemukan alamat `node-2` (`172.28.0.20:9002`) dan membentuk koneksi langsung ke `node-2`.

### Skenario 2: Fee Market Saturation & Cascade Eviction
* Skrip membanjiri mempool `node-2` dengan transaksi P2PKH berbiaya rendah (1.000 milli-quanta/byte).
* Skrip menyuntikkan transaksi dengan fee lebih tinggi (5.000 – 10.000 milli-quanta/byte) serta transaksi data `OP_RETURN`.
* **Verifikasi:** Transaksi berprioritas biaya tinggi diprioritaskan masuk ke dalam kandidat blok oleh `node-1`, biaya terakumulasi ke dalam output *Coinbase*, dan transaksi dengan fee terendah digusur secara atomik jika batas kuota tercapai.

### Skenario 3: Network Partition & Consensus Reorg
* Memutus komunikasi `node-3` dari jaringan bridge Docker (`docker network disconnect`).
* `node-1` menambang rantai blok mayoritas sementara `node-3` menambang rantai minoritas secara terisolasi.
* Partisi jaringan dipulihkan (`docker network connect`).
* **Verifikasi:** Node dengan rantai lebih pendek (`node-3`) mendeteksi rantai kanonikal lebih berat, melakukan rollback atomik, memulihkan UTXO, memvalidasi `utxo_root` pasca-reorg, dan menyelaraskan tip rantai secara sempurna dengan `node-1`.

### Skenario 4: Fast Sync Live Verification (Node-4)
* Setelah rantai mencapai ketinggian $\ge 10$ blok kanonikal, kontainer `node-4` dinyalakan dengan flag `--fast-sync`.
* `node-4` meminta snapshot UTXO via pesan wire `getsnap` ke peer relay.
* Peer menyajikan stream chunk snapshot UTXO kanonikal.
* `node-4` merekonstruksi state via `SnapshotAssembler`, memvalidasi kecocokan `compute_utxo_merkle_root == header.utxo_root`, dan langsung melompat ke tip tanpa mengeksekusi ulang seluruh transaksi historis sejak Genesis.

---

## 3. Spesifikasi Lingkungan Docker

### A. `Dockerfile` Multi-Stage:
* **Stage 1 (Rust Builder):** Menggunakan toolchain Rust stabil untuk mengompilasi `scytale-node` dan `scytale-cli` rilis.
* **Stage 2 (Go Builder):** Menggunakan Go untuk mengompilasi daemon P2P `scytale-p2p`.
* **Stage 3 (Runtime Minimal):** Berbasis `ubuntu:24.04` (glibc 2.39 kompatibel) dilengkapi `curl`, `jq`, `iptables`, `iproute2`, dan `tini`.

### B. `docker-compose.yml`:
* Subnet fixed: `172.28.0.0/16`.
* Layanan `node-1` (`172.28.0.10`), `node-2` (`172.28.0.20`), `node-3` (`172.28.0.30`), dan `node-4` (`172.28.0.40`).
* Isolasi volume dan pemetaan port unik (`9001-9004` untuk P2P dan `8332-8335` untuk HTTP Gateway).

---

## 4. Skrip Automasi Pengujian (`scripts/chaos_stress_test.sh`)

Skrip automasi menjalankan tahapan:
1. *Pre-flight & build verification* (Docker & Compose).
2. *Scenario A:* Penemuan peer otonom via `getaddr`/`addr`.
3. *Scenario B:* Saturasi mempool dan pasar biaya dinamis.
4. *Scenario C:* Partisi jaringan kontainer dan reorganisasi rantai atomik.
5. *Scenario D:* Fast sync state download dan verifikasi `utxo_root`.
6. *Teardown:* Pembersihan kontainer dan volume secara anggun (*graceful teardown*).

---

## 5. Quality Gates

1. Seluruh unit dan integrasi tes Rust lulus (`cargo test --workspace --all-targets`).
2. Seluruh unit dan race detection Go lulus (`(cd network && go test -v -race ./...)`).
3. Skrip `scripts/chaos_stress_test.sh` dieksekusi dengan hasil sukses 100% (4 dari 4 skenario lulus).
4. Tidak ada memory leak, *panic*, atau *data race* selama uji coba berlangsung.

---

## 6. Hasil Verifikasi & Eksekusi

```text
============================================================
   SCYTALE MULTI-NODE DOCKER CLUSTER CHAOS & FAST SYNC     
============================================================
[INFO] [1/5] Pre-flight checks & building container images...
Image scytale:latest Built
[INFO] [2/5] Starting Scenario A: Autonomous Mesh Discovery...
[✓] node-1 (Miner) is responsive on port 8332.
[✓] node-2 (Relay) is responsive on port 8333.
[✓] node-3 (Partition Target) is responsive on port 8334.
[✓] Scenario A PASSED: node-3 discovered mesh peer node-2 autonomously (peer_count = 2 >= 2).
[INFO] [3/5] Starting Scenario B: Fee Market Saturation & Mempool Telemetry...
[INFO] node-1 reached height 59.
[✓] Scenario B PASSED: Fee market transactions propagated and inspected across relay node.
[INFO] [4/5] Starting Scenario C: Network Partition & Atomic Chain Reorganization...
[INFO] Injecting network partition: disconnecting node-3 from scytale-net...
[INFO] Node-1 majority chain height: 146, tip: 0x468dd81eface13559eb14bcf096dcdc90ec0dee6dfa2ef0aad1e0d319537a73a.
[INFO] Node-3 isolated minority branch: height = 133, tip = 0xf0e7146a562cba5bc757411fd0f05c42542e265f57750da182d985b72e07e27e.
[INFO] Healing network partition: reconnecting node-3 to scytale-net...
[✓] node-3 (Reconnected) is responsive on port 8334.
[✓] Scenario C PASSED: node-3 successfully reorganized to majority chain! Tip: 0x468dd81eface13559eb14bcf096dcdc90ec0dee6dfa2ef0aad1e0d319537a73a, UTXO Root: 0x7ed9efad965238cca8c6a5c42dd23843f8b35cd0012acc921228e726926ab7ac.
[INFO] [5/5] Starting Scenario D: Fast Sync Verification (node-4)...
[✓] node-4 (Fast Sync) is responsive on port 8335.
[✓] Scenario D PASSED: node-4 Fast Sync verified! Authenticated utxo_root matches: 0x7ed9efad965238cca8c6a5c42dd23843f8b35cd0012acc921228e726926ab7ac.
[INFO] Checking container metrics and inspecting logs for panics...
NAME                  MEM USAGE / LIMIT     CPU %
scytale-node4         23.81MiB / 15.57GiB   67.29%
scytale-node3         12.66MiB / 15.57GiB   88.33%
scytale-node2         14.99MiB / 15.57GiB   67.61%
scytale-node1         12.49MiB / 15.57GiB   9.73%
============================================================
   ALL 4 CHAOS & FAST SYNC SCENARIOS PASSED 100%!           
============================================================
[INFO] Cleanup complete.
```
