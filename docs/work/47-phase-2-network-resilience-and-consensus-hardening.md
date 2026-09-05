# Work Record: 47. Phase 2 — Network Resilience & Consensus Hardening

## Overview
Implementasi komprehensif paket pembaruan **Fase 2 (Ketahanan Jaringan & Konsensus)** pada ekosistem Layer-1 Scytale. Pembaruan ini memperkuat tiga pilar kritis terhadap ancaman partisi jaringan, eksploitasi reorganisasi rantai dalam (*deep chain reorganization / 51% reorg*), dan serangan *out-of-memory* (OOM) berbasis alokasi memori kontrak pintar:
1. **Hardcoded Fallback Seed Peers pada Go P2P Daemon (`network/`):** Menyediakan daftar IP seed bawaan protokol (`45.147.46.122:9001`) yang secara otomatis diaktifkan saat resolusi DNS seed gagal atau mengembalikan nol alamat, mencegah isolasi node (*cold-start eclipse prevention*).
2. **Max Reorg Depth Protection pada Konsensus & Node Orchestrator (`scytale-consensus` & `scytale-node`):** Menerapkan aturan konsensus deterministik batas kedalaman reorganisasi (`MAX_REORG_DEPTH = 100` blok). Cabang alternatif yang berusaha mencabut lebih dari 100 blok kanonikal ditolak secara *fail-closed* dengan `ChainError::ReorgDepthExceeded`.
3. **Wasm Linear Memory Upper Bound pada ScyVM (`scytale-vm`):** Membatasi pagu alokasi memori linier WebAssembly maksimum 64 halaman (*pages* atau 4 MiB) per eksekusi validator smart contract eUTXO, dilengkapi `StoreLimits` wasmi dengan *trap-on-grow-failure*.

---

## Arsitektur & Karakteristik Desain

### 1. Topologi Fallback Seed Peer & Mitigasi DNS Outage
```text
[ Node Startup / Discovery Heartbeat ]
                  │
                  ▼
         Query DNS Seed Domains
                  │
      ┌───────────┴───────────┐
      │                       │
 [ Resolusi Berhasil ]   [ DNS Gagal / 0 Alamat / Filter ISP ]
      │                       │
      ▼                       ▼
Da柺arkan IP Terurai     Daftarkan DefaultFallbackSeeds
ke Address Book         ("45.147.46.122:9001" official seeder)
      │                       │
      └───────────┬───────────┘
                  ▼
         Trigger Auto-Dialer
```
* **Anti-Eclipse:** Node yang baru dinyalakan di lingkungan jaringan dengan DNS poisoning atau pemblokiran ISP lokal tidak lagi terjebak dalam keadaan terisolasi (*stalled cold-start*).
* **Konfigurasibilitas:** Mendukung flag CLI `--fallback-seed <IP:port>` yang dapat disetel berulang kali untuk private devnet atau custom cluster deployment.

---

### 2. Pertahanan Finalitas Konsensus & Max Reorg Depth
```text
Blok Masuk -> Candidate Cumulative Work > Active Tip Work?
     │
     ├── TIDAK: Simpan di DAG sebagai side branch, selesai.
     │
     └── YA: Hitung Jalur ke Lowest Common Ancestor (LCA)
                 │
                 ▼
         disconnected_blocks = [old_tip .. common_ancestor)
         reorg_depth = disconnected_blocks.len()
                 │
                 ▼
         Apakah reorg_depth > max_reorg_depth (default 100)?
                 ├── YA  : Tolak promosi tip kanonikal!
                 │         Simpan blok di DAG (side branch)
                 │         Kembalikan ChainError::ReorgDepthExceeded
                 │
                 └── TIDAK: Lanjutkan validasi state UTXO & reorg aman
```
* **Probabilistic Finality:** Mencegah serangan *history rewriting* yang mencabut transaksi masa lalu di luar jendela 100 blok.
* **Integrity DAG:** Node tetap menyimpan blok kompetitor dalam struktur `ChainTree` sebagai referensi graf tanpa merusak state database atau tip kanonikal.
* **Node Telemetry:** Parameter dikonfigurasi melalui `NodeConfig.max_reorg_depth` dan flag CLI `--max-reorg-depth <N>`.

---

### 3. Batas Atas Memori Linier ScyVM (4 MiB Upper Bound)
* **Standard Page Size:** 1 Wasm linear memory page = $65.536\text{ bytes}$ (64 KiB).
* **Batas Maksimum:** 64 pages = $4.194.304\text{ bytes}$ (4 MiB).
* **Dual Layer Defense:**
  1. **Engine Level:** `StoreLimitsBuilder::new().memory_size(MAX_WASM_MEMORY_BYTES).trap_on_grow_failure(true).build()` disuntikkan ke `Store`. Upaya kontrak memanggil instruksi `memory.grow` melebihi pagu langsung memicu trap eksekusi tanpa alokasi memori OS.
  2. **Boundary Validation:** Pemeriksaan ukuran halaman memori (`memory.current_pages(&store)`) sebelum passing argumen dan sesudah eksekusi fungsi `validate`. Modul dengan deklarasi awal $> 64$ pages langsung ditolak dengan `VmError::MemoryLimitExceeded`.

---

## Komponen & Perubahan Kode

1. **`network/internal/peer/dns.go`**:
   * Menambahkan slice `DefaultFallbackSeeds` dengan alamat IP seeder resmi devnet (`45.147.46.122:9001`).
2. **`network/cmd/scytale-p2p/main.go`**:
   * Menambahkan flag CLI `--fallback-seed`.
   * Menambahkan field `fallbackSeeds: []string` pada struct `Daemon`.
   * Memperbarui `queryDNSSeedsAsync()` untuk mendaftarkan fallback seeds saat resolusi DNS menghasilkan 0 peer.
   * Memperbarui `Run()` dan `autoDialerLoop()` untuk memastikan node kosong segera di-bootstrap dengan fallback seeds.
3. **`network/internal/peer/dns_test.go`**:
   * Menambahkan pengujian `TestDefaultFallbackSeeds_FormatAndValidity` yang memvalidasi format IP, port, dan routability.
4. **`crates/scytale-consensus/src/error.rs`**:
   * Menambahkan varian error `ChainError::ReorgDepthExceeded { depth: u64, max: u64 }`.
5. **`crates/scytale-consensus/src/lib.rs` & `chain.rs`**:
   * Mengekspor konstanta `pub const DEFAULT_MAX_REORG_DEPTH: u64 = 100;`.
   * Menambahkan field `max_reorg_depth` pada struct `ChainTree`.
   * Menambahkan builder `with_max_reorg_depth`, getter `max_reorg_depth()`, dan setter `set_max_reorg_depth()`.
   * Menambahkan evaluasi batas pada `ChainTree::process_block`.
6. **`apps/scytale-node/src/config.rs`, `node.rs`, & `main.rs`**:
   * Menambahkan field `max_reorg_depth: u64` pada `NodeConfig`.
   * Menambahkan argumen flag `--max-reorg-depth <u64>` (default: 100) pada subperintah `scytale-node start`.
   * Memastikan replikasi dan instansiasi `ChainTree` di `Node` mengadopsi konfigurasi tersebut.
7. **`crates/scytale-consensus/tests/chain_reorg_tests.rs`**:
   * Menambahkan test `test_max_reorg_depth_protection` yang memverifikasi penolakan reorg di luar batas, kekekalan tip kanonikal, dan penerimaan reorg saat batas dinaikkan.
8. **`crates/scytale-vm/src/lib.rs`**:
   * Mendefinisikan `MAX_WASM_MEMORY_PAGES = 64`, `WASM_PAGE_SIZE = 65536`, `MAX_WASM_MEMORY_BYTES = 4194304`.
   * Menambahkan varian error `VmError::MemoryLimitExceeded { pages: u32, max_pages: u32 }`.
   * Mengintegrasikan `StoreLimits` dan pembatas memori ganda pada `ScyVM::execute_validator`.
   * Menambahkan implementasi `Debug, Clone, PartialEq, Eq` pada `ExecutionResult`.
9. **`crates/scytale-vm/tests/memory_limits_tests.rs` [NEW]**:
   * Menambahkan pengujian `test_normal_memory_execution`, `test_reject_excessive_initial_memory`, dan `test_reject_memory_grow_beyond_upper_bound`.

---

## Verifikasi & Hasil Pengujian

1. **Go P2P Test Suite:**
   ```bash
   cd network && go test -v -race ./...
   ```
   *Hasil: PASS 100% pada seluruh paket (`internal/peer`, `internal/gossip`, `internal/wire`, `internal/sync`, `internal/seeder`, `internal/bridge`).*

2. **Consensus Reorganization Test Suite:**
   ```bash
   cargo test -p scytale-consensus --test chain_reorg_tests
   ```
   *Hasil: 8 passed; 0 failed.*

3. **ScyVM Memory Bounds Test Suite:**
   ```bash
   cargo test -p scytale-vm
   ```
   *Hasil: 4 passed; 0 failed.*

4. **Full Workspace Regression & Clippy Linting:**
   ```bash
   cargo test --workspace
   cargo clippy --workspace --all-targets -- -D warnings
   ```
   *Hasil: Seluruh suite unit, integrasi, dan doc-tests lulus 100%. Clippy linter bersih (0 warnings).*
