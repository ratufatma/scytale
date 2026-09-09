# Scytale Protocol — Technical Specification & Architecture Record: Task 38
## Docker Cluster Seeder Integration & Cold-Start Mesh Bootstrap Verification

```text
Task ID       : 38
Task Name     : Docker Cluster Seeder Integration & Cold-Start Mesh Bootstrap Verification
Phase         : Phase 4 — Network Bootstrap & Production Tooling
Target Files  : Dockerfile, docker-compose.yml, scripts/test_seeder_coldstart.sh, docs/work/38-docker-seeder-coldstart-bootstrap.md
Reference     : network/cmd/scytale-seeder, network/cmd/scytale-p2p, docker-compose.yml
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Autonomous Peer Discovery via DNS, Zero Static CLI Peer on Bootstrap Node, Subnet Convergence, Strict Merkle Root Consistency
```

---

## 1. Arsitektur Uji Cold-Start Multi-Node Docker

Task 38 menguji secara *end-to-end* kemampuan bootstrap otonom node baru Scytale dalam topologi Docker Compose terisolasi (`scytale-net: 172.28.0.0/16`):

```text
┌─────────────────────────────────────────────────────────────────────────────────┐
│                        DOCKER CLUSTER (scytale-net)                             │
│                                                                                 │
│   [ Seeder Container ] (172.28.0.5:53 UDP/TCP)                                  │
│   • Menjalankan scytale-seeder --domain=seed.scytale.org                        │
│   • Crawl & probe aktif ke node-1 (172.28.0.10:9001) & node-2 (172.28.0.20)     │
│   • Mencatat node-1 dan node-2 sebagai "Good Nodes" di ledger memori            │
│                                                                                 │
│   [ Established Mesh Nodes ]                                                    │
│   • node-1 (172.28.0.10): Mining aktif (blok bertambah)                         │
│   • node-2 (172.28.0.20): Sinkronisasi dengan node-1                            │
│                                                                                 │
│   [ Cold-Start Node ] (node-coldstart: 172.28.0.50)                             │
│   • Start dengan ZERO static peer (--peer KOSONG)                               │
│   • Konfigurasi DNS mengarah ke container seeder: dns: [172.28.0.5]             │
│   • P2P daemon mendeteksi AddrBook kosong ──► Kueri A seed.scytale.org          │
│   • Menerima respons IP node-1 / node-2 dari seeder                             │
│   • Melakukan auto-dial ──► Handshake ──► Sinkronisasi rantai blok              │
│   • Hasil: Ketinggian blok dan utxo_root node-coldstart identik dengan node-1!  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Rincian Modifikasi Komponen

### A. Update `Dockerfile`
- Menambahkan kompilasi `scytale-seeder` pada tahap Go builder:
  ```dockerfile
  RUN CGO_ENABLED=0 go build -ldflags="-s -w" -o /app/bin/scytale-seeder ./cmd/scytale-seeder
  ```
- Menyalin binary `scytale-seeder` ke `/usr/local/bin/scytale-seeder` pada image runtime minimal.

### B. Update `docker-compose.yml`
- Menambahkan service `seeder`:
  - IP: `172.28.0.5`
  - Port 53 UDP/TCP terikat ke bridge internal.
  - Parameter: `--domain=seed.scytale.org --listen=:53 --seeds=172.28.0.10:9001,172.28.0.20:9002 --workers=4 --probe-interval=10s`.
- Menambahkan service `node-coldstart` (di bawah profil `coldstart`):
  - IP: `172.28.0.50`
  - Konfigurasi DNS container: `dns: [172.28.0.5]`.
  - Parameter: `scytale-node start --p2p-bind 0.0.0.0:9005 --http-bind 0.0.0.0:8336` (TANPA bendera `--peer`).

### C. Skrip Uji Verifikasi Otomatis (`scripts/test_seeder_coldstart.sh`)
- Menjalankan kluster: `seeder`, `node-1`, `node-2`.
- Menunggu node-1 menambang minimal 10 blok lalu membekukan penambangan untuk verifikasi konvergensi.
- Memverifikasi seeder mendeteksi node-1 via kueri `dig @127.0.0.1 -p 1053 seed.scytale.org A`.
- Menyalakan `node-coldstart` tanpa flag `--peer`.
- Memverifikasi `node-coldstart` menemukan peer via DNS seeder dan tersinkronisasi hingga ketinggian blok yang sama dengan `node-1`.

---

## 3. Quality Gates & Verification Plan

1. Binary `scytale-seeder` berhasil dibangun dan terpasang di dalam container `scytale:latest`.
2. Kueri DNS internal mengembalikan IP node yang valid dari seeder.
3. Node `node-coldstart` berhasil terhubung ke jaringan tanpa konfigurasi IP statis apa pun.
4. Nilai `utxo_root` dan tinggi kanonikal pada `node-coldstart` terbukti identik (*state convergence*).

---

## 4. Hasil Verifikasi & Eksekusi

```text
============================================================
   SCYTALE DOCKER SEEDER & COLD-START BOOTSTRAP TEST       
============================================================
[INFO] [1/4] Compiling binaries and building container image...
[INFO] [2/4] Starting Seeder, node-1 (Miner), and node-2 (Relay)...
[INFO] Waiting for node-1 (HTTP port 8332) to be ready...
[✓] node-1 is responsive on port 8332.
[INFO] Waiting for node-2 (HTTP port 8333) to be ready...
[✓] node-2 is responsive on port 8333.
[INFO] Waiting for node-1 to mine at least 10 blocks...
[✓] node-1 mined 43 blocks.
[INFO] Pausing mining on node-1 to freeze state for cold-start sync verification...
[INFO] [3/4] Verifying DNS Seeder resolves healthy nodes...
[INFO] DNS Seeder response for seed.scytale.org: 172.28.0.10
172.28.0.20
[INFO] [4/4] Starting node-coldstart with ZERO static peers...
[INFO] Waiting for node-coldstart (HTTP port 8336) to be ready...
[✓] node-coldstart is responsive on port 8336.
[INFO] Waiting for node-coldstart to discover peers via DNS and sync blocks...
[INFO] Progress [1s]: node-coldstart height = 51, node-1 height = 51
[✓] node-coldstart successfully synced to height 51!
[✓] UTXO commitment verified: utxo_root matches perfectly (0xa69a0f50f70a033e19d7e65cb86500f92b0740cdf29be1e45543ccad8cab6f7d).
============================================================
   AUTONOMOUS DNS SEEDER & COLD-START BOOTSTRAP PASSED!     
============================================================
```
