# Scytale Protocol — Technical Specification & Milestone Record: Tasks 32–34

```text
Project Scope  : Scytale Layer-1 Protocol
Milestone Span : Phase 3 Completion (Authenticated State, Fast Sync & Chaos Engineering)
Current Status : 135 Workspace Tests PASS | 28 Go Test Suites Race-Free | 4/4 Docker Chaos Scenarios PASS | Zero Float
```

---

## RINGKASAN CAPAIAN ARSITEKTUR

Dengan penyelesaian Task 32 hingga 34, Scytale bertransformasi dari blockchain berbasis pemutaran riwayat (*history replay*) menjadi **Authenticated State Transition System** yang mendukung sinkronisasi keadaan instan (*instant state sync*) dan tahan terhadap anomali partisi jaringan terdistribusi:

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
│  • Ultra-Low Resource Usage: 12–24 MiB RAM per container node          │
└────────────────────────────────────────────────────────────────────────┘
```

---

## TASK 32: COMPACT UTXO COMMITMENT (`utxo_root` IN BLOCKHEADER)

* **Tujuan:** Mengeliminasi ketergantungan pada validasi transaksi historis dari Genesis untuk membuktikan status saldo koin aktif, dengan mengunci komitmen kriptografis status koin yang belum dibelanjakan (*unspent coins*) langsung pada setiap header blok.
* **Komponen:** `crates/scytale-core`, `crates/scytale-storage`, `crates/scytale-mining`, `crates/scytale-consensus`, `apps/scytale-node`.
* **Arsitektur & Invarian:**
  * **Ekspansi Header Kanonikal 120-Byte:**
    Ukuran serialisasi biner header diperluas dari 88 byte menjadi 120 byte:

    $$\text{Header} = \text{version (4B)} \,\Vert{}\, \text{prev\_hash (32B)} \,\Vert{}\, \text{tx\_merkle (32B)} \,\Vert{}\, \mathbf{utxo\_root\ (32B)} \,\Vert{}\, \text{timestamp (8B)} \,\Vert{}\, \text{target (4B)} \,\Vert{}\, \text{nonce (8B)}$$

  * **Preimage Daun Merkle Kanonikal:**
    Setiap entri koin yang tersimpan di tabel `UTXOS` database dipetakan ke daun hash BLAKE3 dengan domain separation:

    $$\text{Leaf} = \text{BLAKE3}\Big(\text{"SCYTALE\_UTXO\_LEAF\_V1"} \,\Vert{}\, \text{txid (32B)} \,\Vert{}\, \text{index (4B LE)} \,\Vert{}\, \text{value\_quanta (8B LE)} \,\Vert{}\, \text{locking\_script}\Big)$$

  * **Pohon Merkle Seimbang Leksikografis:**
    * Daun diurutkan berdasarkan `OutPoint` kanonikal: `(txid ASC, index ASC)`.
    * Jika jumlah daun ganjil, daun terakhir diduplikasi untuk membentuk cabang simetris.
    * Jika himpunan koin kosong: $\text{utxo\_root} = \mathbf{0}_{32}$.

  * **Aturan Konsensus Post-State:**
    * Penambang mensimulasikan mutasi UTXO prospektif sebelum memulai iterasi proof-of-work.
    * Node validator mengeksekusi mutasi pada basis data sementara (*staging*), menghitung `compute_utxo_root()`, dan menolak blok mentah-mentah (*fail-closed*) jika `block.header.utxo_root != calculated_root` dengan error `BlockError::InvalidUtxoRoot`.

  * **Otentikasi Snapshot Penyimpanan (`scytale-storage`):**
    * Disediakan method atomik `export_utxo_snapshot` dan `apply_utxo_snapshot` pada `StorageEngine`.

---

## TASK 33: FAST SYNC WIRE PROTOCOL (`getsnapshot` / `snapshot`)

* **Tujuan:** Menyediakan mekanisme transmisi status koin terotentikasi melintasi jaringan P2P daemon Go (`network/`) sehingga node validator baru dapat langsung sinkron tanpa memproses ulang ribuan blok historis.
* **Komponen:** `network/internal/wire/`, `network/internal/peer/`, `network/cmd/scytale-p2p/`, `crates/scytale-bridge/`, `apps/scytale-node/`.
* **Arsitektur & Invarian:**
  * **Protokol Wire Binary Baru:**
    * `CmdGetSnapshot = "getsnap"` (36 byte): `BlockHash [32B] | ChunkIndex (4B LE)`.
    * `CmdSnapshot = "snapshot"`:

      $$\text{Frame} = \text{BlockHash [32B]} \,\Vert{}\, \text{ChunkIndex (4B)} \,\Vert{}\, \text{TotalChunks (4B)} \,\Vert{}\, \text{EntryCount (4B)} \,\Vert{}\, \sum \text{UtxoEntries}$$

      Setiap entri biner memuat: `TxID [32B] | Index [4B LE] | Value [8B LE] | ScriptLen [4B LE] | LockingScript [N Bytes]`.

  * **Batasan Ukuran & Anti-DoS:**
    * `MaxSnapshotChunkEntries = 2000` ($\le 2\text{ MB}$ per pesan socket).
    * `MaxLockingScriptSize = 10000` byte.
    * Rate Limiting: Peer pembayar hanya melayani maksimal 1 permintaan awal (`chunkIndex == 0`) per 30 detik per koneksi peer. Chunk lanjutan diperbolehkan mengalir secara berurutan tanpa penundaan.

  * **Rekonstruksi Memori Dinamis (`snapshot_assembler.go`):**
    * Mengelola penerimaan chunk yang mungkin tiba secara acak (*out-of-order*).
    * Setelah seluruh chunk $0 \dots \text{TotalChunks}-1$ lengkap, payload diserahkan ke node Rust melalui socket IPC (`NodeRequest::ApplySnapshot`).

  * **Fail-Closed State Verification:**
    * Node Rust merekonstruksi tabel koin di memori, menghitung `compute_utxo_merkle_root()`, mencocokkannya dengan `header.utxo_root`, dan menolak snapshot jika terdapat deviasi 1-bit sebelum diterapkan ke `redb`.

---

## TASK 34: MULTI-NODE DOCKER CLUSTER CHAOS & FAST SYNC TEST

* **Tujuan:** Memvalidasi seluruh subsistem protokol Scytale di lingkungan jaringan kontainer multi-node nyata di bawah tekanan konkurensi, lelang biaya, partisi jaringan (*split-brain*), dan sinkronisasi kilat.
* **Komponen:** `docker-compose.yml`, `Dockerfile`, `scripts/chaos_stress_test.sh`, `apps/scytale-node/src/node.rs`, `network/cmd/scytale-p2p/main.go`.
* **Topologi Kluster (`scytale-net` Subnet `172.28.0.0/16`):**
  * `node-1` (Miner, `172.28.0.10`): Memproduksi blok, mengakumulasikan biaya transaksi ke Coinbase, menyajikan snapshot.
  * `node-2` (Relay, `172.28.0.20`): Peer perantara, menyebarkan transaksi mempool via wire gossip.
  * `node-3` (Partition Target, `172.28.0.30`): Sasaran injeksi isolasi jaringan dan reorganisasi rantai.
  * `node-4` (Fast Sync Target, `172.28.0.40`): Bergabung belakangan menggunakan flag `--fast-sync`.

* **Hasil 4 Skenario Pengujian Chaos:**
  1. **Autonomous Mesh Discovery:** `node-3` (hanya dikonfigurasi ke `node-1`) secara otonom menemukan `node-2` via pesan `getaddr`/`addr` (`peer_count >= 2`).
  2. **Fee Market Saturation:** Transaksi bersaing berdasarkan rasio biaya; transaksi dengan densitas biaya tertinggi diprioritaskan masuk ke template blok, dan miner fee terakumulasi ke Coinbase.
  3. **Network Partition & Atomic Reorg:**
     * `node-3` diputus dari subnet Docker (`docker network disconnect`) dan menambang rantai minoritas (height 133).
     * `node-1` menambang rantai mayoritas (height 146).
     * Saat partisi disembuhkan (`docker network connect`), `node-3` secara atomik melakukan rollback rantai minoritas dan menyinkronkan rantai mayoritas dengan `utxo_root` identik:
       `0x7ed9efad965238cca8c6a5c42dd23843f8b35cd0012acc921228e726926ab7ac`.
  4. **Fast Sync Live Verification:**
     * `node-4` menyerap snapshot state UTXO melalui pesan `getsnap` dari `node-2`.
     * State divalidasi dan diaplikasikan tanpa mengeksekusi ulang blok dari Genesis. `utxo_root` diverifikasi identik 100% dengan `node-1`.

* **Perbaikan Deadlock & Konkurensi Sistem:**
  * Mengeliminasi *Lock Order Inversion* pada `submit_transaction` di `node.rs` (memanggil `canonical_height()` sebelum mengunci mutex `utxo_set`).
  * Efisiensi memori terverifikasi: seluruh node hanya mengonsumsi **12.49 MiB – 23.81 MiB RAM** (jauh di bawah batas keamanan 512 MiB).

---

## MATRIKS VERIFIKASI AKHIR PROTOKOL (FASE 3 SELESAI)

| Komponen / Pipeline | Status Verifikasi | Catatan Mutu & Invarian |
| --- | --- | --- |
| **Rust Unit & Integration Tests** | **135 LULUS** | `cargo test --workspace --all-targets` |
| **Go P2P Network Suite** | **28 SUITE LULUS** | `go test -v -race ./...` (0 race condition, 0 deadlock) |
| **Linter & Formatting** | **LULUS** | `cargo fmt --check` & `cargo clippy -D warnings` |
| **Aritmatika Integer Konsensus** | **TERPENUHI** | Nol operasi float di seluruh lapisan konsensus & fee |
| **2-Node Sync Testnet** | **LULUS** | `./scripts/testnet_2node.sh` |
| **Fork Reorg Testnet** | **LULUS** | `./scripts/testnet_fork_reorg.sh` |
| **Docker Chaos & Fast Sync** | **LULUS 100%** | `./scripts/chaos_stress_test.sh` (4/4 skenario PASS) |
