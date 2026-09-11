# Scytale Protocol — Technical Specification & Milestone Record: Tasks 32–38

```text
Project Scope  : Scytale Layer-1 Protocol
Milestone Span : Phase 3 Final Completion (State Authenticity, Fast Sync, Chaos Engineering, Autonomous DNS Seeder & Cold-Start Verification)
Current Status : 135 Workspace Tests PASS | Go Test Suites Race-Free | Docker Chaos PASS | Cold-Start Bootstrap PASS | Zero Float
```

---

## RINGKASAN CAPAIAN ARSITEKTUR

Penyelesaian Task 32 hingga Task 38 mentransformasi Scytale menjadi **Authenticated State Transition System** yang beroperasi secara desentralisasi penuh, otonom sejak inisialisasi awal (*cold-start*), dan tahan terhadap gangguan partisi jaringan:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        SCYTALE PROTOCOL STACK                          │
│                                                                        │
│  [ State Authenticity & Primitives (Task 32) ]                         │
│  • 120-Byte Canonical Header with `utxo_root` Post-State Commitment    │
│  • Lexicographical Balanced Binary Merkle Tree (redb UTXOS Table)      │
│  • Fail-Closed Consensus Rule on State Mismatch                        │
│                                                                        │
│  [ Fast Sync & Wire Streaming (Task 33) ]                              │
│  • Chunked Binary Wire Protocol: `getsnap` / `snapshot` (<= 2K txs/msg)│
│  • Out-of-Order Memory Reconstruction (`snapshot_assembler.go`)       │
│  • Anti-DoS Rate Limiting (30s interval on initial chunk request)      │
│                                                                        │
│  [ Distributed Chaos & System Verification (Task 34) ]                 │
│  • 4-Node Isolated Docker Cluster (`scytale-net` 172.28.0.0/16)        │
│  • Autonomous Mesh Peering & Fee Market Live Verification              │
│  • Network Partition Reorg with Exact Post-State `utxo_root` Match     │
│  • Ultra-Low Resource Footprint: 12–24 MiB RAM per container node      │
│                                                                        │
│  [ Autonomous Network Bootstrapping (Tasks 35, 36 & 37) ]              │
│  • Authoritative Dual-Stack DNS Seeder Daemon (UDP/TCP :53, miekg/dns) │
│  • Anti-Sybil /24 Subnet Limiter (Max 2 IPs per subnet)                │
│  • Cold-Start Async DNS Resolver with Periodic Auto-Dial Fallback      │
│  • Authoritative NS Delegation Architecture (Cloudflare + Linux)       │
│                                                                        │
│  [ Containerized Seeder & Cold-Start Mesh Verification (Task 38) ]     │
│  • Multi-service Docker integration: `seeder` + `node-coldstart`       │
│  • Zero-configuration peering: Node boots with 0 static peers          │
│  • State convergence verified via internal DNS resolution in 1s        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## TASK 32: COMPACT UTXO COMMITMENT (`utxo_root` IN BLOCKHEADER)

* **Tujuan:** Mengunci komitmen kriptografis status koin yang belum dibelanjakan (*unspent coins*) langsung pada setiap header blok tanpa memerlukan replay transaksi historis dari genesis.
* **Komponen:** `crates/scytale-core`, `crates/scytale-storage`, `crates/scytale-mining`, `crates/scytale-consensus`, `apps/scytale-node`.
* **Arsitektur & Invarian:**
  * **Header Kanonikal 120-Byte:**
    $$\text{Header} = \text{version (4B)} \,\Vert{}\, \text{prev\_hash (32B)} \,\Vert{}\, \text{tx\_merkle (32B)} \,\Vert{}\, \mathbf{utxo\_root\ (32B)} \,\Vert{}\, \text{timestamp (8B)} \,\Vert{}\, \text{target (4B)} \,\Vert{}\, \text{nonce (8B)}$$
  * **Preimage Daun Merkle Kanonikal:**
    $$\text{Leaf} = \text{BLAKE3}\Big(\text{"SCYTALE\_UTXO\_LEAF\_V1"} \,\Vert{}\, \text{txid (32B)} \,\Vert{}\, \text{index (4B LE)} \,\Vert{}\, \text{value\_quanta (8B LE)} \,\Vert{}\, \text{locking\_script}\Big)$$
  * **Pohon Merkle Seimbang Leksikografis:**
    * Daun diurutkan berdasarkan `OutPoint` kanonikal: `(txid ASC, index ASC)`.
    * Jika jumlah daun ganjil, daun terakhir diduplikasi untuk membentuk cabang simetris.
    * Himpunan UTXO kosong: $\text{utxo\_root} = \mathbf{0}_{32}$.
  * **Aturan Konsensus Post-State:**
    * Penambang mensimulasikan mutasi UTXO prospektif sebelum proof-of-work.
    * Validator mengeksekusi mutasi pada staging database, menghitung `compute_utxo_root()`, dan menolak blok (*fail-closed*) jika `block.header.utxo_root != calculated_root` dengan error `BlockError::InvalidUtxoRoot`.
  * **Snapshot Penyimpanan (`scytale-storage`):**
    * Operasi atomik `export_utxo_snapshot` dan `apply_utxo_snapshot` pada `StorageEngine`.

---

## TASK 33: FAST SYNC WIRE PROTOCOL (`getsnapshot` / `snapshot`)

* **Tujuan:** Menyediakan mekanisme transmisi status koin terotentikasi melintasi jaringan P2P daemon Go (`network/`) sehingga node validator baru dapat langsung sinkron dalam hitungan detik.
* **Komponen:** `network/internal/wire/`, `network/internal/peer/`, `network/cmd/scytale-p2p/`, `crates/scytale-bridge/`, `apps/scytale-node/`.
* **Arsitektur & Invarian:**
  * **Protokol Wire Binary:**
    * `CmdGetSnapshot = "getsnap"` (36 byte): `BlockHash [32B] | ChunkIndex (4B LE)`.
    * `CmdSnapshot = "snapshot"`:
      $$\text{Frame} = \text{BlockHash [32B]} \,\Vert{}\, \text{ChunkIndex (4B)} \,\Vert{}\, \text{TotalChunks (4B)} \,\Vert{}\, \text{EntryCount (4B)} \,\Vert{}\, \sum \text{UtxoEntries}$$
      Format biner per entri: `TxID [32B] | Index [4B LE] | Value [8B LE] | ScriptLen [4B LE] | LockingScript [N Bytes]`.
  * **Batasan Ukuran & Anti-DoS:**
    * `MaxSnapshotChunkEntries = 2000` ($\le 2\text{ MB}$ per pesan).
    * `MaxLockingScriptSize = 10000` byte.
    * Rate Limiting: Peer pembayar hanya melayani maksimal 1 permintaan awal (`chunkIndex == 0`) per 30 detik per peer. Chunk lanjutan mengalir berurutan tanpa jeda.
  * **Rekonstruksi Memori Dinamis (`snapshot_assembler.go`):**
    * Menerima chunk yang tiba acak (*out-of-order*).
    * Setelah chunk $0 \dots \text{TotalChunks}-1$ lengkap, payload diserahkan via socket IPC (`NodeRequest::ApplySnapshot`).
  * **Fail-Closed State Verification:**
    * Node Rust merekonstruksi tabel koin di memori, memverifikasi `compute_utxo_merkle_root() == header.utxo_root`, dan menolak snapshot jika terjadi deviasi 1-bit sebelum diterapkan ke `redb`.

---

## TASK 34: MULTI-NODE DOCKER CLUSTER CHAOS & FAST SYNC TEST

* **Tujuan:** Memvalidasi seluruh subsistem protokol di lingkungan jaringan kontainer nyata di bawah tekanan konkurensi, lelang biaya, partisi jaringan (*split-brain*), dan sinkronisasi kilat.
* **Komponen:** `docker-compose.yml`, `Dockerfile`, `scripts/chaos_stress_test.sh`, `apps/scytale-node/src/node.rs`, `network/cmd/scytale-p2p/main.go`.
* **Topologi Kluster (`scytale-net` Subnet `172.28.0.0/16`):**
  * `node-1` (Miner, `172.28.0.10`): Memproduksi blok, mengakumulasikan miner fees, melayani snapshot.
  * `node-2` (Relay, `172.28.0.20`): Menyebarkan transaksi mempool via wire gossip.
  * `node-3` (Partition Target, `172.28.0.30`): Sasaran isolasi jaringan dan reorganisasi rantai.
  * `node-4` (Fast Sync Target, `172.28.0.40`): Bergabung belakangan menggunakan flag `--fast-sync`.
* **Hasil 4 Skenario Pengujian Chaos:**
  1. **Autonomous Mesh Discovery:** `node-3` (hanya dikonfigurasi ke `node-1`) secara mandiri menemukan `node-2` via `getaddr`/`addr` (`peer_count >= 2`).
  2. **Fee Market Saturation:** Transaksi dengan densitas biaya tertinggi diprioritaskan masuk ke blok, dan miner fee terakumulasi ke output Coinbase.
  3. **Network Partition & Atomic Reorg:**
     * `node-3` diputus dari subnet Docker (`docker network disconnect`) dan menambang rantai minoritas.
     * Saat partisi disembuhkan (`docker network connect`), `node-3` melakukan rollback atomik dan menyinkronkan rantai mayoritas dengan `utxo_root` identik.
  4. **Fast Sync Live Verification:**
     * `node-4` menyerap snapshot state UTXO dari `node-2` via `getsnap`.
     * State divalidasi dan diaplikasikan tanpa replay blok dari Genesis. `utxo_root` diverifikasi identik 100% dengan `node-1`.
* **Ketahanan & Efisiensi Sistem:**
  * Mengeliminasi *Lock Order Inversion* pada `submit_transaction` di `node.rs`.
  * Efisiensi memori terverifikasi: seluruh node hanya mengonsumsi **12.49 MiB – 23.81 MiB RAM** (batas aman < 512 MiB).

---

## TASK 35: AUTONOMOUS DNS SEEDER DAEMON

* **Tujuan:** Menyediakan layanan nama domain DNS otoritatif yang mengembalikan subset alamat IP node sehat secara dinamis untuk mengatasi masalah *cold-start bootstrap*.
* **Komponen:** `network/cmd/scytale-seeder/`, `network/internal/seeder/`.
* **Arsitektur & Invarian:**
  * **Dual-Stack DNS Server (`server.go`):**
    * Menjalankan listener paralel pada port 53 UDP dan TCP berbasis `github.com/miekg/dns`.
    * Mengembalikan record `A` (IPv4) dan `AAAA` (IPv6) dengan TTL 60 detik serta status `Authoritative = true`.
    * Pengacakan IP kandidat menggunakan Fisher-Yates shuffle berbasis `crypto/rand` (maksimal 16 record).
  * **Evaluator Reputasi & Anti-Sybil (`store.go`):**
    * Freshness: `LastSuccess` dalam kurun waktu $\le 2$ jam terakhir.
    * Reliability: Rasio keberhasilan $\ge 70\%$ jika `TotalAttempts >= 3`.
    * Consensus Lag: `BestHeight` tertinggal $\le 288$ blok dari median jaringan.
    * Anti-Sybil Subnet Limiter: Maksimal **2 alamat IP per subnet `/24`** (IPv4) atau subnet `/48` (IPv6).
  * **Crawler & Prober (`crawler.go`):**
    * Worker pool konkuren (16 worker) yang mendial target P2P port 9001 dengan timeout 3 detik.
    * Melakukan handshake `wire.MsgVersion`/`wire.MsgVerack`, mencatat tinggi rantai, dan meminta alamat baru via `wire.MsgGetAddr`.
    * Penjadwalan ulang dengan *exponential backoff* hingga maksimal 6 jam.

---

## TASK 36: DYNAMIC DNS SEEDER CLIENT & COLD-START BOOTSTRAPPING

* **Tujuan:** Mengintegrasikan resolusi DNS seeder otomatis ke dalam daemon klien `scytale-p2p` saat node pertama kali dijalankan tanpa riwayat peer.
* **Komponen:** `network/internal/peer/dns.go`, `network/cmd/scytale-p2p/main.go`.
* **Arsitektur & Invarian:**
  * **Klien Resolver (`dns.go`):**
    * `ResolveDNSSeeds`: Mendukung domain standar, domain berport custom, dan literal IP.
    * Menyaring alamat unroutable (loopback, multicast, 0.0.0.0) dan melakukan deduplikasi IP.
  * **Integrasi Daemon P2P (`main.go`):**
    * Flag CLI baru: `--dns-seed` (default: `"seed.scytale.org"`) dan `--no-dns-seeds`.
    * Cold-Start Auto-Trigger: Jika `addrBook.Size() == 0`, daemon menjalankan resolusi DNS asinkron (`queryDNSSeedsAsync`), memasukkan hasil ke `addrBook`, dan langsung memicu `triggerDial`.
    * Periodic Fallback: Jika koneksi keluar belum memenuhi target dan `addrBook` kosong, kueri DNS dijadwalkan ulang tiap 3 menit.

---

## TASK 37: CLOUDFLARE DNS & NS DELEGATION OPERATIONAL GUIDE

* **Tujuan:** Menyusun panduan operasional delegasi nameserver otoritatif dari penyedia DNS (Cloudflare) ke daemon `scytale-seeder`.
* **Komponen:** `docs/DNS-SEEDER-DEPLOYMENT-GUIDE.md`, `docs/work/37-cloudflare-dns-seeder-delegation-guide.md`.
* **Rancangan Operasional:**
  * **Glue Record (Record A):**
    * `ns1.seed.scytale.org` $\rightarrow$ IP Publik Server Seeder.
    * Proxy Status: **DNS Only (Grey Cloud)** — *wajib non-proxied agar port 53 UDP/TCP tidak terblokir*.
  * **Delegasi Subdomain (Record NS):**
    * `seed.scytale.org` $\rightarrow$ `ns1.seed.scytale.org`.
  * **Systemd Service Linux:**
    * Konfigurasi unit `/etc/systemd/system/scytale-seeder.service` dengan *graceful restart* dan optimasi `LimitNOFILE=65535`.
    * Nonaktifkan stub listener `systemd-resolved` di Ubuntu agar port 53 bebas digunakan oleh seeder.

---

## TASK 38: DOCKER CLUSTER SEEDER & COLD-START MESH VERIFICATION

* **Tujuan:** Memvalidasi integrasi end-to-end antara server DNS Seeder dan node klien dalam kluster Docker multi-container terisolasi tanpa konfigurasi manual peer statis.
* **Komponen:** `Dockerfile`, `docker-compose.yml`, `apps/scytale-node/src/main.rs`, `apps/scytale-node/src/p2p_supervisor.rs`, `scripts/test_seeder_coldstart.sh`.
* **Topologi & Konfigurasi Pengujian:**
  * Container `seeder` (`172.28.0.5`): Melayani kueri DNS internal port 53 UDP/TCP dan melakukan probing berkala ke `node-1` (`172.28.0.10`) dan `node-2` (`172.28.0.20`).
  * Container `node-coldstart` (`172.28.0.50`): Menggunakan nameserver seeder (`dns: [172.28.0.5]`), **dijalankan dengan 0 static peer** (tanpa `--peer`).
  * Meneruskan parameter CLI `--dns-seed` dan `--no-dns-seeds` dari supervisor node Rust ke subprocess P2P Go.
* **Hasil Pengujian Otomatis (`scripts/test_seeder_coldstart.sh`):**
  * `seeder` mendeteksi node aktif dan mengembalikan IP `172.28.0.10` dan `172.28.0.20` saat menerima kueri `seed.scytale.org`.
  * `node-coldstart` melakukan cold-start, menyelesaikan domain seed, melakukan handshake peering secara mandiri, dan menyinkronkan rantai blok hingga height 51 dalam tempo 1 detik.
  * **State Convergence:** `utxo_root` pada `node-coldstart` cocok 100% dengan `node-1` (`0xa69a0f50f70a033e19d7e65cb86500f92b0740cdf29be1e45543ccad8cab6f7d`).

---

## MATRIKS VERIFIKASI AKHIR PROTOKOL (FASE 3 SELESAI PENUH)

| Komponen / Pipeline | Status Verifikasi | Catatan Mutu & Invarian |
| --- | --- | --- |
| **Rust Unit & Integration Tests** | **135 LULUS** | `cargo test --workspace --all-targets` |
| **Go P2P Network Test Suite** | **LULUS PENUH** | `go test -v -race ./...` (0 race condition, 0 deadlock) |
| **DNS Seeder Test Suite** | **LULUS PENUH** | `go test -v -race ./internal/seeder/...` |
| **DNS Client Resolver Test Suite** | **LULUS PENUH** | `go test -v -race ./internal/peer/...` |
| **Linter & Formatting** | **LULUS** | `cargo fmt --check` & `cargo clippy -D warnings` |
| **Aritmatika Konsensus** | **TERPENUHI** | Nol operasi float di seluruh lapisan konsensus & fee |
| **2-Node Sync Testnet** | **LULUS** | `./scripts/testnet_2node.sh` |
| **Fork Reorg Testnet** | **LULUS** | `./scripts/testnet_fork_reorg.sh` |
| **Docker Chaos & Fast Sync** | **LULUS 100%** | `./scripts/chaos_stress_test.sh` (4/4 skenario PASS) |
| **Docker Seeder Cold-Start** | **LULUS 100%** | `./scripts/test_seeder_coldstart.sh` (Zero static peers PASS) |
