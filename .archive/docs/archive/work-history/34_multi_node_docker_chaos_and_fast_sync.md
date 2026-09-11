# Dokumen Spesifikasi Teknis — Task 34

```text
Task ID       : 34
Task Name     : Multi-Node Docker Cluster Chaos & Fast Sync End-to-End Stress Test
Phase         : Phase 3 — Protocol Hardening & System Verification
Target Files  : docker-compose.yml, Dockerfile, scripts/chaos_stress_test.sh, docs/work/34_multi_node_docker_chaos_and_fast_sync.md
Status        : READY FOR EXECUTION
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
            ▼                                 ▼                                 ▼
┌───────────────────────┐         ┌───────────────────────┐         ┌───────────────────────┐
│     node-1 (Miner)    │         │     node-2 (Relay)    │         │   node-3 (Partition)  │
│  IP: 172.28.0.10      │◄───────►│  IP: 172.28.0.20      │◄───────►│  IP: 172.28.0.30      │
│  Port P2P: 9001       │ Wire    │  Port P2P: 9002       │ Wire    │  Port P2P: 9003       │
│  Port HTTP: 8332      │ P2P     │  Port HTTP: 8333      │ P2P     │  Port HTTP: 8334      │
│  • Block producer     │         │  • Dynamic discovery  │         │  • Chaos target       │
│  • Fee accumulator    │         │  • Mempool gossip     │         │  • Fork producer      │
│  • Snapshot server    │         │  • Relay validator    │         │  • Reorg victim       │
└───────────────────────┘         └───────────────────────┘         └───────────────────────┘
                                              ▲
                                              │ (Fast Sync Trigger: Block Tip >= 10)
                                              │
                                  ┌───────────┴───────────┐
                                  │   node-4 (Fast Sync)  │
                                  │  IP: 172.28.0.40      │
                                  │  Port P2P: 9004       │
                                  │  Port HTTP: 8335      │
                                  │  • Bergabung belakangan│
                                  │  • Flag: --fast-sync  │
                                  │  • Unduh snapshot     │
                                  │  • Skip block replay  │
                                  └───────────────────────┘
```
           ┌──────────────────────┬───────────┴───────────┬──────────────────────┐
           │                      │                       │                      │
     ┌─────▼──────┐         ┌─────▼──────┐          ┌─────▼──────┐         ┌─────▼──────┐
     │   node-1   │         │   node-2   │          │   node-3   │         │   node-4   │
     │ 172.28.0.10│◄───────►│ 172.28.0.20│◄─ ─ ─ ─ ─│ 172.28.0.30│         │ 172.28.0.40│
     │  (Miner)   │  P2P    │  (Relay)   │  Mesh    │  (Chaos)   │         │(Fast Sync) │
     │  9001/8332 │         │  9002/8333 │ Discovery│  9003/8334 │         │  9004/8335 │
     └────────────┘         └────────────┘          └────────────┘         └─────┬──────┘
                                                                                 │ Fast Sync
                                                                                 └───────────► (Snapshot from node-2)
```

---

## 2. Skenario Pengujian Chaos End-to-End

### Skenario A: Autonomous Mesh Discovery (`getaddr` / `addr`)
* `node-1` dan `node-2` berjalan terhubung.
* `node-3` dimulai hanya dengan konfigurasi awal peer ke `node-1:9001`.
* **Verifikasi:** Daemon P2P `node-3` bertukar `getaddr` dengan `node-1`, menerima daftar alamat `node-2:9002`, melakukan auto-dial otonom, dan mencapai topologi mesh (`peer_count >= 2`).

### Skenario B: Dynamic Fee Market & Mempool Propagation
* Ingest transaksi beragam tingkat biaya (fee density) ke `node-1`.
* **Verifikasi:** Transaksi dipropagasi melalui gossip dua-tahap (`inv` / `getdata` / `tx`) ke `node-2`, telemetri mempool dan `min_relay_fee` terdeteksi pada HTTP Gateway.

### Skenario C: Network Partition & Atomic Chain Reorganization
* Suntik partisi jaringan: putuskan `node-3` dari `scytale-net` via Docker network disconnect.
* `node-1` melanjutkan penambangan cabang mayoritas.
* `node-3` menambang cabang minoritas terisolasi.
* Pulihkan partisi: sambungkan kembali `node-3` ke `scytale-net`.
* **Verifikasi:** `node-3` mendeteksi rantai mayoritas yang lebih berbobot, membatalkan cabang minoritasnya, dan melakukan rollback serta reorganisasi atomik ke tip dan `utxo_root` kanonikal `node-1`.

### Skenario D: Fast Sync State Download & Merkle Verification
* Jalankan `node-4` dengan mode sinkronisasi kilat (`--fast-sync`).
* `node-4` meminta potongan snapshot UTXO dari `node-2` via `getsnapshot` / `snapshot`.
* **Verifikasi:** `node-4` merekonstruksi seluruh state UTXO secara chunked, memverifikasi komitmen Merkle root secara fail-closed, dan mencocokkan `utxo_root` kanonikal tanpa perlu mengeksekusi ulang seluruh histori blok sejak Genesis.

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
[INFO] Node-3 isolated minority branch: height = 133, tip: 0xf0e7146a562cba5bc757411fd0f05c42542e265f57750da182d985b72e07e27e.
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
