# Laporan Analisis Konsistensi Dokumentasi vs Implementasi Kode

**Tanggal:** 2026-09-09
**Versi:** 0.3.0-devnet
**Ruang Lingkup:** 31 spesifikasi tingkat atas (`docs/*.md`) + 48 working docs (`docs/work/`) vs implementasi aktual
**Metode:** Analisis read-only (tidak ada kode yang diubah)

---

## Ringkasan Eksekutif

Analisis ini membandingkan 79 dokumen dengan implementasi kode aktual di 11 crate, 4 aplikasi, dan 2 kontrak. **Verdict keseluruhan: dokumentasi ~60% akurat.** Masalah terbesar bukan pada angka protokol (yang mayoritas sudah benar), melainkan pada:

1. **Subsistem Go P2P TIDAK ADA** — spesifikasi, README, dan audit checklist mendeskripsikan daemon Go, DNS seeder, dan wire protocol sebagai "PRODUCTION READY", tetapi seluruh `network/` directory beserta semua file `.go` **tidak ada di repository**.
2. **Dokumen ekonomi saling kontradiktif internal** — banyak dokumen masih membawa model lama 42M SCY yang sudah digantikan 94.98M SCY di kode.
3. **Parameter yang di-spec sebagai "TBD" sudah diputuskan di kode** — kode telah maju melampaui dokumen pada difficulty, target, chain selection, dan mempool.
4. **16 file duplikat identik** di `docs/work/` (tasks 34-41 dalam dua versi penamaan).

---

## Skema Penilaian

| Verdict | Definisi |
|---------|----------|
| **MATCH** | Spesifikasi = implementasi |
| **PARTIAL** | Sebagian besar benar, ada tambahan/kurang |
| **MISMATCH** | Spesifikasi bertentangan dengan implementasi |
| **NOT-IMPLEMENTED** | Fitur di-spec tapi tidak ada di kode |
| **RESOLVED** | Spec bertuliskan TBD, kode telah memutuskan nilainya |

---

## Bagian 1: Dokumen Ekonomi & Genesis (5 dokumen)

| Parameter | Nilai Spec | Nilai Kode | Verdict |
|-----------|-----------|-----------|---------|
| `QUANTA_PER_SCY` | 100,000,000 | `100_000_000` (primitives/lib.rs:10) | **MATCH** |
| Initial block reward | 10 SCY | `INITIAL_REWARD = 10 * QUANTA_PER_SCY` | **MATCH** |
| Halving interval | 2,100,000 blok | `HALVING_INTERVAL = 2_100_000` | **MATCH** |
| Target block interval | 60 detik | `TARGET_BLOCK_INTERVAL_SECS = 60` | **MATCH** |
| Max supply | 94,980,000 SCY | `MAX_SUPPLY_QUANTA = 9.498e15` | **MATCH** |
| Genesis allocation | 66M SCY | `TOTAL_GENESIS_QUANTA = 6.6e15` | **MATCH** |
| Founder/Dev/Eco | 30/20/50% = 19.8/13.2/33M | sama persis (genesis.rs:24-33) | **MATCH** |
| Mining reserve | 28,980,000 SCY | `MINING_RESERVE_QUANTA = 2.898e15` | **MATCH** |
| Reward end height | 3,696,000 | `MINING_REWARD_END_HEIGHT = 3_696_000` | **MATCH** |
| Alamat genesis | `scy1nw7…1q5n…1nrl…` | identik (genesis.rs:39-48) | **MATCH** |
| Locking scripts | `73a0209b…` dll | identik (genesis.rs:51-60) | **MATCH** |

### Temuan Ketidaksesuaian

| # | Temuan | Tingkat |
|---|--------|---------|
| 1 | **MONETARY-POLICY.md kontradiktif internal**: §5 dan §11 masih menyebut 4,200,000,000,000,000 quanta (model 42M), bertentangan dgn §3 (94.98M) dan kode | **MISMATCH** (dokumen) |
| 2 | **§6 MONETARY-POLICY** masih menggambarkan "42,000,000 SCY ceiling" sebagai **"CONSENSUS ISSUE — REQUIRES RESOLUTION"** yang belum diputuskan, padahal kode **sudah** memutuskan dan menerapkan hard cap 28.98M via terminasi reward di height 3,696,000 | **MISMATCH** (dokumen ketinggalan) |
| 3 | **PROTOCOL-CONSTANTS.md §4** masih mencantumkan model 42M/10.5M/15%/5%/5% di samping model baru, mendeklarasikan "Requires Resolution" | **MISMATCH** (dokumen) |
| 4 | **GENESIS-SPEC.md §4.1** menggunakan diagram lama (15%/5%/5%) bertentangan dgn §4 dan §6 di file yang sama serta kode | **MISMATCH** (dokumen) |
| 5 | **GENESIS-ALLOCATION.md §3.2/§3.3** menuliskan Treasury 5%/2.1M SCY dan Ecosystem 5%/2.1M, padahal §1 file yang sama dan kode memakai 20%/13.2M dan 50%/33M | **MISMATCH** (dokumen) |
| 6 | `DEFAULT_DIFFICULTY_EPOCH_BLOCKS = 1440` sudah ada di kode (difficulty.rs:6), tapi PROTOCOL-CONSTANTS L79 masih TBD | **RESOLVED** (kode maju) |
| 7 | Komentar 42M-SCY lama di genesis.rs:20-21 (kosmetik, nilai sudah benar 94.98M) | **MISMATCH** (komentar kode) |

---

## Bagian 2: Block, Transaksi, UTXO, Hashing (4 dokumen)

### BlockHeader
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Jumlah field | **6 field** (BLOCK-SPEC §2) | **7 field** — kode menambahkan `utxo_root: Hash256` (32 byte) | **PARTIAL** |
| Urutan serialisasi | version, prev_hash, tx_commitment, timestamp, difficulty, nonce | version, prev_hash, tx_commitment, **utxo_root**, timestamp, difficulty, nonce | **PARTIAL** |
| Ukuran terserialisasi | (tidak eksplisit) | **120 byte** (4+32+32+32+8+4+8) | **PARTIAL** — 120 byte hanya benar jika termasuk `utxo_root` yang TIDAK ada di spec |
| `difficulty_target` | tipe TBD (u32/[u8;32]) | `u32` (compact bits) | **RESOLVED** |
| `nonce` | ukuran TBD | `u64` | **RESOLVED** |
| BlockID = BLAKE3(canonical header) | §3 | `header.hash()` persis | **MATCH** |

> **Temuan paling kritis di area ini:** field `utxo_root` — komitmen Merkle UTXO yang divalidasi (error `InvalidUtxoRoot`) — **tidak disebut sama sekali di semua spesifikasi** (BLOCK-SPEC, UTXO-SPEC, HASHING-SPEC). Karena `utxo_root` ikut di-serialize ke dalam header dan ikut dalam formula BlockID, node yang menghitung BlockID berdasarkan spec 6-field akan menghasilkan hash berbeda dari kode 7-field.

### Transaksi
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Field transaksi | version, inputs, outputs | **menambahkan `lock_time: u64`** (consensus-relevant, belum di-spec) | **PARTIAL** |
| OutPoint | txid (32B) + index (u32) = 36 byte | identik | **MATCH** |
| TxIn | previous_output + authorization | identik | **MATCH** |
| TxOut | value (u64) + locking_condition | identik | **MATCH** |
| Formula fee | ΣIn − ΣOut, non-negatif | `calculate_fee` + `apply_transaction`, dengan `checked_sub` | **MATCH** |
| Aturan zero-value | setiap output non-zero (!) | mengizinkan zero-value HANYA jika `0x6a` (OP_RETURN) | **PARTIAL** (spec lebih ketat, OP_RETURN exception tidak terdokumentasi) |
| TxID | BLAKE3(canonical tx) | BLAKE3(canonical tx **termasuk lock_time**) | **PARTIAL** |
| Coinbase "no OutPoint" | tanpa OutPoint | memakai sentinel `OutPoint::null()` (ZERO + u32::MAX) | **PARTIAL** |

### UTXO
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Struktur record | value + locking_condition | `UtxoEntry` menambahkan `block_height` + `is_coinbase` | **PARTIAL** |
| Backend storage | redb (scytale-storage) | `UtxoSet` in-memory `HashMap` di scytale-core (persistensi ada di crate terpisah) | **PARTIAL** — gap scope |
| `utxo_root` Merkle tree | **TIDAK ADA di spec** | Lengkap: leaf `SCYTALE_UTXO_LEAF_V1`, balanced tree, proof, verifikasi | **MISMATCH / spec-silent** |
| Aturan lifecycle (double-spend, atomicity, solvency) | §2 | semua diterapkan atomik + teruji | **MATCH** |

### Hashing & Serialisasi
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| BLAKE3 + 32-byte | §1 | konsisten di seluruh kode | **MATCH** |
| Format serialisasi | **TBD** ("tanpa commit ke crate pihak ketiga") | codec manual lengkap: LE integers + u32 length-prefix, `MAX_VECTOR_LENGTH = 16MB`, reject trailing bytes | **RESOLVED** (kode maju) |
| Domain separation | "Pending" (TxID) | `txid()` tanpa domain tag (konsisten), tapi `compute_hash`/`compute_sighash` memakai tag undocumented | **PARTIAL** |

---

## Bagian 3: Consensus, PoW, Difficulty, Chain Selection (4 dokumen)

### PoW
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Primitive/output | BLAKE3, 32 byte | `blake3::hash` → 32 byte | **MATCH** |
| Input | BLAKE3(canonical header) | `compute_pow_hash = header.hash()` | **MATCH** |
| Perbandingan ≤ target | hash ≤ target | byte-array big-endian `<=` | **MATCH** |
| **Domain separation PoW vs BlockID** | §6: "formally distinguished by the protocol" | **TIDAK ADA** — PoW dan BlockID memakai `header.hash()` identik | **MISMATCH** |
| Nonce width | TBD | `u64` | **RESOLVED** |

### Difficulty
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Target interval | 60 detik | `TARGET_BLOCK_INTERVAL_SECS = 60` | **MATCH** |
| Epoch length | **TBD** | `DEFAULT_DIFFICULTY_EPOCH_BLOCKS = 1440` (1 hari) | **RESOLVED** |
| Formula retarget | `T_new = T_old × T_obs/T_exp` | `scale_target_by_ratio` persis, integer u128 | **MATCH** |
| Clamping | **TBD** (contoh 4×) | `CLAMPING_FACTOR = 4` (clamp di rentang expected/4..expected×4) | **RESOLVED** |
| Max target floor | **TBD** | `max_target ≈ 2^236` | **RESOLVED** |
| Min target ceiling | **TBD** | tidak ada konstanta eksplisit (hanya saturasi 256-bit) | **PARTIAL** |

### Target Representation
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Width | 256-bit vs compact (**TBD**) | `Target([u8; 32])` 256-bit internal + `u32` compact di wire | **RESOLVED** |
| Field layout header | difficulty_target | `u32` compact bits | **MATCH** |
| Interpretasi hash | endianness **TBD** | big-endian (byte 0 = MSB) | **RESOLVED** |

### Chain Selection
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Aturan kanonik | argmax cumulative work | `cum_work > active_tip` → reorg | **MATCH** |
| Formula work | `2^256/(T+1)` | `div256(~T, T+1)+1` = eksak | **MATCH** |
| Cumulative work | u256 **TBD** | `CumulativeWork([u64;4])` + checked_add | **RESOLVED** |
| Max reorg depth | **TBD**/opsional | `DEFAULT_MAX_REORG_DEPTH = 100` + error `ReorgDepthExceeded` | **RESOLVED** (feature tambahan) |
| Tie-break work sama | first-arrived (**TBD**) | only `>` triggers reorg; `<=` keeps existing | **MATCH** |
| Re-admission transaksi | **TBD** | dikumpulkan dari disconnected blocks | **RESOLVED** |

### Emisi Reward
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Initial reward | 10 SCY | `INITIAL_REWARD = 10*QUANTA_PER_SCY` | **MATCH** |
| Halving | 2.1M blok, 50% | `INITIAL_REWARD >> halvings` | **MATCH** |
| **Reward cessation** | tabel §4/§6 menunjukkan **halving tak terbatas** (Epoch 1 penuh = 10.5M SCY) | **hard stop** di height 3,696,000 → Epoch 1 dipotong jadi 7.98M; total mining 28.98M | **MISMATCH** (kode memutuskan hal yang dokumen masih tulis TBD) |
| Supply cap | 94.98M (§3 "locked") vs 42M (di §4/§6) | 94.98M via reserve ter-truncate 28.98M | Kode konsisten dgn tabel "locked", dokumen tidak konsisten internal |

---

## Bagian 4: Mempool, Mining, Storage (3 dokumen)

### Mempool
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Pipeline admission | 6 langkah (spec) | 9 langkah (kode); urutan berbeda (kode: UTXO dulu, auth kemudian) | **PARTIAL** |
| Double-spend invariant | §5 dipenuhi | `ConflictDoubleSpend`, First-Seen (policy TBD di spec) | **MATCH** (policy resolved) |
| Fee rate unit | **TBD** | `fee * 1000 / size_bytes` = milli-quanta/byte | **RESOLVED** |
| Eviction policy | **TBD** | lowest-fee-rate + cascading descendant eviction | **RESOLVED** |
| Max mempool size | **TBD** | `DEFAULT_MAX_MEMPOOL_COUNT = 5_000`, `MAX_MEMPOOL_BYTES = 5_000_000` | **RESOLVED** |
| Min relay fee | **TBD** | `DEFAULT_MIN_RELAY_FEE_RATE = 1_000` | **RESOLVED** |
| Dependency tracking | topologi parent-child (§8) | `parent_to_children`/`child_to_parents`, topological selection | **MATCH** |
| Block connection sync | §9 | `on_block_connected()` + cascading removal | **MATCH** |
| Transaction expiration/TTL | **TBD** | **TIDAK diimplementasikan** | **NOT-IMPLEMENTED** |
| Persistence | opsional (§12) | murni in-memory (konsisten dgn status opsional) | **MATCH** |
| Coinbase rejection | tidak disebut | `CoinbaseNotAllowed` (rule defensif) | Kode-only (bonus) |

### Mining
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Candidate header | 6 field | 7 field (+ `utxo_root`) | **PARTIAL** |
| Transaction commitment | "root" (implikasi Merkle) | flat hash dari concatenated TxID (bukan Merkle tree) | **PARTIAL** |
| Coinbase index 0 | §5 | dipaksa index 0, value = subsidy + fees | **MATCH** |
| Payout | miner-designated | `miner_locking_condition` parameter | **MATCH** |
| Nonce search | loop nonce | `run_pow_search()` in-place nonce bytes | **MATCH** |
| Multi-threading | **TBD** | `std::thread::scope` up to 8 threads | **RESOLVED** (bonus) |
| Cancellation | **TBD** | `Arc<AtomicBool>` per 4095 iterasi | **RESOLVED** (bonus) |
| Template refresh policy | **TBD** | **tidak ada mekanisme refresh** di crate | **NOT-IMPLEMENTED** |
| Pre-broadcast validation | §9: 8 langkah | hanya error variant `LocalValidationFailed`; pipeline ada di caller | **PARTIAL** |
| Miner state machine | 5 state | 5 state + `Failed(String)` | **PARTIAL** |
| `MAX_BLOCK_PAYLOAD_SIZE` | tidak di-spec | `2_000_000` (2 MB) | Kode-only |

### Storage
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Engine | redb, ACID, MVCC | `Database::create` + InMemory backend test | **MATCH** |
| Jumlah table | **5 primary tables** (§4) | **6 tables** (+ `ADDRESS_TX_INDEX`) | **PARTIAL** |
| Nama UTXO table | `UTXO_SET` | `UTXOS` (legacy alias `UTXO_TABLE`) | **MISMATCH** (penamaan) |
| BLOCK_INDEX | key BlockID → Height, PrevHash, Timestamp, DifficultyTarget, CumWork, ValidationStatus | `BlockMeta` HANYA {height, cumulative_work, timestamp} = 48 byte; **PrevHash, DifficultyTarget, ValidationStatus hilang** | **PARTIAL** |
| CHAIN_STATE | single singleton value (tip + metadata) | dua entry terpisah string-keyed (`tip_hash`, `tip_height`); **BestCumulativeWork & ConsensusEpochMetadata hilang** | **PARTIAL** |
| Atomic block commit | §10: single WriteTransaction | `commit_block()` single transaction | **MATCH** |
| Rollback on error | immediate rollback | RAII drop → rollback otomatis oleh redb | **MATCH** |
| Reorg atomicity | implied | `apply_reorganization()` single WriteTransaction | **MATCH** |
| ADDRESS_TX_INDEX | §3.3 "opsional" | diimplementasikan penuh (key 40-byte) | Kode melebihi spec |
| Snapshot | tidak dijabarkan | `export/apply_utxo_snapshot` + verifikasi Merkle root | Kode-only |
| Komentar `BlockMeta` | — | komentar bilang "56 bytes" tp `BYTE_LEN = 48` | **MISMATCH** (komentar) |

---

## Bagian 5: P2P, Node Lifecycle, DNS Seeder, Security (4 dokumen) — ⚠️ AREA PALING KRITIS

### ⚠️ Temuan Utama: Sub-sistem Go P2P TIDAK ADA

```
Direktori /mnt/ssd/scytale-lab/scytale/network/  →  TIDAK ADA
File *.go                                      →  0 file
go.mod                                         →  tidak ada
```

| Fitur | Klaim Spec/README/Audit | Realita Kode | Verdict |
|-------|------------------------|--------------|---------|
| Arsitektur P2P | daemon Go (peer discovery, pooling, framing, relay) + Rust validation boundary | transport aktual = **Rust `async_nats` pub/sub** ke `nats://116.212.72.89:4222`, subject `scytale.v1.blocks.new`, `scytale.v1.mempool.tx`, `scytale.v1.nodes.heartbeat` | **MISMATCH** |
| Rust↔Go IPC boundary | Unix socket antara Go dan Rust | `P2pSupervisor` (Unix-socket bridge) + `scytale-bridge` ABI **ada tapi tidak pernah di-instantiate** (dead code) | **NOT-IMPLEMENTED** |
| Wire protocol | Handshake, Ping/Pong, ChainLocator, Tx/BlockAnnouncement | tidak ada codec; NATS kirim raw bytes; tanpa handshake/network-ID/genesis match | **NOT-IMPLEMENTED** |
| Peer discovery | static + DNS + PEX | static `--peer` hanya dikonsumsi supervisor yang tak pernah jalan; `ConnectPeer` events di-ignore di `main.rs` (`Ok(P2pBridgeEvent::ConnectPeer{..}) => {}`) | **NOT-IMPLEMENTED** |
| Misbehavior scoring & flood protection | §13-14; audit klaim **PASS** via `network/internal/wire/wire_test.go` | tidak ada; file audit yang dikutip **tidak ada** | **NOT-IMPLEMENTED** |
| DNS Seeder | daemon `scytale-seeder`, NS :53, TTL 60, "Production Ready" | **TIDAK ADA**; `--dns-seed` flags di-parse tapi **tidak pernah dikonsumsi** kode | **NOT-IMPLEMENTED** |
| Audit checklist | "28 test suites PASS", "network/=go test clean" | kutipan ke file yang tidak ada | **Dokumen menyesatkan** |

**Bukti:** `peers.json` (format addrbook Go dengan `addr/src/attempts/last_seen`, terakhir ditulis 2026-09-05) membuktikan daemon Go pernah berjalan, tetapi sumbernya sudah dihapus. README.md L33-43, CHANGELOG.md L168, dan TASKS_32_TO_38.md semuanya merujuk layout `network/` yang tidak eksis.

### Node Lifecycle
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| State machine | STARTING→READY→RUNNING↔DEGRADED→FAILED→STOPPING→STOPPED | semua kecuali **DEGRADED** (tidak ada) dan **FAILED** (dideklarasikan tapi tidak pernah di-set); ada tambahan `Recovering` | **PARTIAL** |
| Startup pipeline | 10 langkah (incl. P2P spawn + IBD) | 6 langkah non-network; **P2P spawn (step 7) dan IBD (step 8) ABSEN** | **PARTIAL** |
| Readiness | termasuk "peer networking operational" | tak pernah mengkonsultasi P2P | **PARTIAL** |
| IBD workflow | §6: query tips, locator, header download, block stream | `get_block_locator()` ada tapi loop download/apply **tidak diimplementasikan** | **NOT-IMPLEMENTED** |
| Shutdown | 6 langkah (mempool flush, P2P disconnect, storage flush, timeout) | hanya stop mining; **tidak ada mempool flush, P2P disconnect, enforcement `shutdown_timeout_secs`** | **PARTIAL** |
| Reorg orchestration | §10 | `submit_external_block` + `apply_reorganization` + `mempool.on_reorg`, teruji | **MATCH** |

### Security Controls
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Argon2 PIN vault | crypto suite TBD | **diimplementasikan**: Argon2id (64MiB/3 iter/4 lanes), 6-digit PIN, anti-tracer `TracerPid`, zeroize | **MATCH** (di area lain, bukan threat model) |
| ChaCha20-Poly1305 | TBD | diimplementasikan (12-byte nonce, versioned envelope) | **MATCH** |
| File permissions | secrecy | `mode(0o600)` wallet, `generate_genesis_keys` 0600 | **MATCH** |
| Double-spend/UTXO solvency | — | diimplementasikan + teruji | **MATCH** |
| Integer quanta | u64 only, deny float | `#![deny(clippy::float_arithmetic)]` diworkspace | **MATCH** |
| Mempool DoS limits (threat G) | size + fee eviction | DIPENUHI (5000/5MB/1000 feed) | **MATCH** |
| Peer anti-eclipse, misbehavior, per-IP (threat H/I) | **TBD** di spec; audit klaim PASS | tidak ada | **NOT-IMPLEMENTED** |
| Fail-closed consensus | principle 4 | fail-closed reorg (`pre_validate_reorg_branch`), snapshot root mismatch rejection | **MATCH** |

---

## Bagian 6: Smart Contract, Authorization, Passbook, Provenance (5 dokumen)

### Smart Contracts
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Model eUTXO `Validator(Datum, Redeemer, Ctx)` | yes | `ScyVM::execute_validator` | **MATCH** |
| Datum di UTXO | ya | `OutputLock::Script { script_hash, datum }` | **MATCH** |
| Redeemer di spender | ya | di-encode di `TxIn.authorization` | **MATCH** |
| TxContext fields | "block height, block time, fee, hash" (§26) | `{tx_hash, block_time, input_amount, fee_burned}` — **tidak ada `block_height`!** Kontrak tidak bisa gate pada block height walau spec bilang bisa | **PARTIAL** |
| Wasm entrypoint `validate` | 1 fungsi | kedua kontrak ekspor `validate(ptr,len)×3 → i32` | **MATCH** |
| Runtime wasmi | yes | `Engine`, `Config::consume_fuel(true)` | **MATCH** |
| VALIDATION constants | 1=lolos, 0=ditolak | `VALIDATION_SUCCESS: i32 = 1`, `VALIDATION_REJECT: i32 = 0` | **MATCH** |
| Gas metering | konseptual | wasmi fuel; `MAX_TX_GAS = 5_000_000`, `MAX_BLOCK_GAS = 50_000_000` (constants hanya di work-notes, bukan spec utama) | **MATCH** (gap dokumentasi) |
| Memory limits | tidak ada angka | `MAX_WASM_MEMORY_PAGES = 64` (4 MiB), `trap_on_grow_failure` | **MATCH** (gap dokumentasi) |
| **Ukuran Wasm** | **"< 30 KB"** | artifact release aktual: vault 91,760 byte; scy20 91,969 byte (**±92 KB, ~3× klaim**) | **MISMATCH** |
| Build target | wasm32-unknown-unknown | sama + teruji | **MATCH** |
| CLI contract tooling | 4 subcommand + auto dry-run | 4 subcommand + `--skip-dry-run`, print fuel | **MATCH** |

### Authorization
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Models: locking_condition + proof | ya | `TxOut.locking_condition` + `TxIn.authorization` | **MATCH** |
| Algorithm | **TBD** (Ed25519/Schnorr/PQ kandidat) | **Ed25519 dikunci & diimplementasikan** (verify_strict) | **MISMATCH** (kode maju) |
| Format public key | **TBD** | 32-byte Ed25519 | **MISMATCH** (kode maju) |
| Format signature | **TBD** | 64-byte Ed25519 | **MISMATCH** (kode maju) |
| Model scripting | **None (Baseline)** — "tanpa VM overhead" | kode punya **dua-duanya**: stack engine 22 opcode (incl OP_CHECKSIG, OP_BLAKE3, CLTV) DAN Wasm eUTXO VM | **MISMATCH** (spec stale + kontradiksi internal dgn SMART_CONTRACTS) |
| P2PKH | (tidak eksplisit) | `OP_DUP OP_BLAKE3 <32B> OP_EQUALVERIFY OP_CHECKSIG` | **MATCH** (kode maju) |
| Stateless verification | invariant 1 | `AuthorizationVerifier` + `ConsensusScriptVerifier` | **PARTIAL** (context juga bawa block_height utk CLTV) |
| No malleability/replay | invariant 3-4 | `signature_preimage_digest` bind version+inputs+outputs+lock_time; anti-replay tested | **MATCH** |

### Passbook
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Read-only | strict | `passbook.rs` read-only, derive fresh dari node | **MATCH** |
| Balance derive dinamis | ya, tak pernah statis | `confirmed_balance_quanta()` sum fresh | **MATCH** |
| Denomination | integer quanta, zero float | `QUANTA_PER_SCY = 100_000_000`, u64 + checked_add | **MATCH** |
| Zero initial | never display unbacked credit | `zero_balance_initialization` test | **MATCH** |
| **Lifecycle status** | **Pending/Confirmed/Rejected/Unknown/Unverified** | kode: `Confirmed/Pending/Reorganized` — **tidak ada Rejected & Unknown; ditambah Reorganized** | **MISMATCH** |
| Detail view | TxID, block ref, gross amount, inputs consumed, outputs created | entry punya txid/block_height/outpoint; **tidak ada block hash, tidak ada breakdown per-input/output** | **PARTIAL** |
| Provenance viewer | DAG traversal | single-path trace (`inputs.first()` saja, tidak merge multi-input DAG) | **PARTIAL** |
| SCY-20 tracking | tak ada di spec | `PassbookAsset::Scy20`, token_balances | Kode superset |
| Balance display SCY | "renders SCY and quanta" | hanya integer quanta; **tidak ada modul format SCY** | **NOT-IMPLEMENTED** (spec pun defer) |
| Re-verification | selalu re-verify thd node | tidak ada cache internal | **MATCH** |

### Value Provenance
| Aspek | Spec | Kode | Verdict |
|-------|------|------|---------|
| Lokasi trace | passbook viewer | `Passbook::provenance()` (node.rs passbook layer) | **MATCH** |
| Konsensus enforcement | "intrinsic consensus invariant" di block connection | tidak ada verifier DAG eksplisit; enforcement **implisit struktural** (setiap input harus ada di UTXO set staged) | **PARTIAL** |
| Genesis mapping 30/20/50 | OutPoint(GTx,0/1/2) = 1.98e15/1.32e15/3.30e15 | identik + test | **MATCH** |
| Macro reconciliation | 9.498e15 = 6.6e15 + 2.898e15 | `MAX_SUPPLY_QUANTA` + `MINING_RESERVE_QUANTA` + test eksak | **MATCH** |
| DAG lineage | inherit complete ancestral DAG, merge | **hanya single-path** (ikuti `inputs.first()`) | **PARTIAL** |
| Deterministic query alg. | retrieve→block→genesis/coinbase→recurse | persis bentuk itu | **MATCH** |
| Provenance Query API | **TBD** | ada API internal, **tidak ada endpoint RPC/JSON** | **PARTIAL/ NOT-IMPLEMENTED** (konsisten dgn TBD) |

---

## Bagian 7: Working Documents (48 file)

### Duplikasi
| Task | Versi Hyphen vs Underscore | Status |
|------|---------------------------|--------|
| 34 | **BERBEDA KONTEN** — hyphen "COMPLETED/PRODUCTION-READY" (143 baris), underscore "READY FOR EXECUTION" (161 baris, diagram lebih besar, nama skenario berbeda) | Kontradiktif |
| 35, 36, 37, 38, 39, 40, 41 | **IDENTIK byte-for-byte** (7 dari 8 pasangan) | Duplikat murni |

**Total: 16 file duplikat identik** yang bisa di-deduplikasi.

### Mapping Top-level Docs ← Working Docs
- **17 dokumen** top-level bersumber langsung dari working docs 01-17 (evolusi langsung): MONETARY-POLICY←01, GENESIS-ALLOCATION←02, TRANSACTION-SPEC←03, UTXO-SPEC←04, AUTHORIZATION-SPEC←05, HASHING←06, BLOCK←07, POW←08, DIFFICULTY←09, CHAIN-SELECTION←10, MEMPOOL←11, MINING-LIFECYCLE←12, P2P←13, STORAGE←14, NODE-LIFECYCLE←15, PASSBOOK←16, VALUE-PROVENANCE←17.
- **Parsial:** SMART_CONTRACTS←41 (top-level lebih lengkap), DNS-SEEDER-DEPLOYMENT-GUIDE←37.
- **Tanpa origin (disintesis):** ARCHITECTURE, AUDIT_CHECKLIST, CONSENSUS-SPEC, ECONOMIC-MODEL, GENESIS-SPEC, LEDGER-SPEC, PROTOCOL-CONSTANTS, SECURITY-THREAT-MODEL, TESTING-STRATEGY.
- **~30 working docs orphaned** (18-48) tidak punya top-level counterpart.

### Marker Stale
- Working docs 01-17 yang sudah "COMPLETED" masih membawa marker `[ IN PROGRESS ]` di pipeline section (frozen artifacts).
- 8 residu TBD di dokumen yang ditandai COMPLETED: 05 (crypto TBD/BLOCKED), 06 (format TBD/BLOCKED), 07 (tx commitment TBD), 08 (target encoding TBD), 09 (epoch TBD), 10 (tie-break TBD), 11 (RBF TBD), 18 (CONSENSUS ISSUE).
- Task 39 = satu-satunya dokumen "IN PROGRESS" (dan duplikatnya).
- Tasks 42-48 memakai format berbeda tanpa status header formal.
- `TASKS_32_TO_34.md` **superseded** oleh `TASKS_32_TO_38.md` (strict superset).

---

## Rangkuman Verdict per Dokumen

| Dokumen | Verdict Keseluruhan |
|---------|--------------------|
| MONETARY-POLICY.md | **PARTIAL** (kontradiksi internal 42M vs 94.98M) |
| ECONOMIC-MODEL.md | **PARTIAL** (vague tentang reward cessation) |
| GENESIS-SPEC.md | **PARTIAL** (§4.1 stale) |
| GENESIS-ALLOCATION.md | **PARTIAL** (§2/3.2/3.3 stale) |
| PROTOCOL-CONSTANTS.md | **PARTIAL** (model lama + beberapa TBD yang sudah diputuskan) |
| BLOCK-SPEC.md | **PARTIAL** (utxo_root hilang) |
| TRANSACTION-SPEC.md | **PARTIAL** (lock_time hilang) |
| UTXO-SPEC.md | **MISMATCH/SILENT** (utxo_root & Merkle tak ada) |
| HASHING-AND-SERIALIZATION-SPEC.md | **PARTIAL** (format TBD sudah diresolusi) |
| CONSENSUS-SPEC.md | **PARTIAL** (emisi naratif vs trunkasi) |
| POW-SPEC.md | **PARTIAL** (no domain separation; emisi) |
| DIFFICULTY-SPEC.md | **RESOLVED** (semua TBD sudah diputuskan di kode) |
| CHAIN-SELECTION-SPEC.md | **RESOLVED** (TBD sudah diputuskan) |
| MEMPOOL-SPEC.md | **RESOLVED** (TBD sudah diputuskan; TTL bukan impl) |
| MINING-LIFECYCLE-SPEC.md | **PARTIAL** (template refresh/pre-broadcast belum; utxo_root) |
| STORAGE-SPEC.md | **PARTIAL** (BLOCK_INDEX metadata, CHAIN_STATE, penamaan UTXOS) |
| P2P-NETWORK-SPEC.md | **MISMATCH / NOT-IMPLEMENTED** (subsistem Go absen total) |
| NODE-LIFECYCLE-SPEC.md | **PARTIAL** (DEGRADED/FAILED/P2P/IBD/shutdown) |
| DNS-SEEDER-DEPLOYMENT-GUIDE.md | **NOT-IMPLEMENTED** (seeder tidak ada) |
| SECURITY-THREAT-MODEL.md | **PARTIAL** (crypto OK, network-layer absen) |
| SMART_CONTRACTS.md | **PARTIAL** (TxContext tanpa block_height; ukuran Wasm) |
| AUTHORIZATION-SPEC.md | **MISMATCH** (spec TBD/None, kode konklusif) |
| PASSBOOK-CONCEPT.md | **PARTIAL** (status enum, detail view) |
| LEDGER-SPEC.md | **PARTIAL** |
| VALUE-PROVENANCE-SPEC.md | **PARTIAL** (single-path vs DAG) |
| TESTING-STRATEGY.md | **PARTIAL** (mengklaim cakupan yg tak lagi ada) |
| AUDIT_CHECKLIST_v0.3.0.md | **MISMATCH** (mengutip file P2P yang tidak ada) |
| TASKS_32_TO_34.md | **SUPERSEDED** oleh TASKS_32_TO_38.md |
| Working docs 34-41 | **44% duplikat** (16 file identik) |

---

## Temuan Kritis yang Memerlukan Aksi

### 📛 Kritis (memengaruhi kebenaran operasional)
1. **Go P2P subsystem hilang** tapi seluruh dokumentasi dan audit checklist mengklaimnya lengkap dan lulus. Transport aktual NATS pub/sub ke satu node hardcoded — perlu klarifikasi apakah ini kondisi yang diinginkan atau kehilangan aksidental.
2. **`utxo_root` tidak terdokumentasi** di spec tapi consensus-critical (120-byte header hanya benar dengan field ini).
3. **Emisi reward**: kode sudah menerapkan trunkasi hard-stop (28.98M), dokumen masih menyatakan TBD/berdebat. Spesifikasi dan kode berjalan arah berlawanan.
4. **Node di `.gitignore`**: seluruh `network/` tidak ada; jika ini repo publik, mustahil memverifikasi "go test clean" yang diklaim.

### 🟠 Sedang (inkonsistensi dokumentasi internal)
5. Model 42M SCY lama masih berserakan di 4 dokumen (MONETARY-POLICY, PROTOCOL-CONSTANTS, GENESIS-SPEC, GENESIS-ALLOCATION).
6. `TASKS_32_TO_34.md` sudah digantikan tanpa penandaan.
7. `BlockMeta` comment "56 bytes" vs aktual 48 bytes.
8. AUTHORIZATION-SPEC masih "TBD/None" padahal kode konklusif (Ed25519 + Wasm).

### 🟡 Ringan (harness/kualitas)
9. 16 working docs duplikat identik + 1 pasangan kontradiktif (task 34).
10. Ukuran Wasm aktual 92 KB vs klaim spec <30 KB.
11. Task 39 masih "IN PROGRESS" — belum selesai.
12. `TxContext` tidak menyediakan `block_height` yang dijanjikan spec.

---

## Kesimpulan

- **Yang sudah sangat baik:** parameter ekonomi makro (supply, alokasi, halving, reward end) **100% benar** dengan toleransi detail; mekanisme anti-inflasi, atomicity storage, mempool double-spend, genesis mapping, dan integer-only aritmetika terverifikasi konsisten.
- **Yang paling bermasalah:** (a) ketiadaan subsistem Go P2P yang didokumentasikan sebagai ada, (b) emisi reward yang kodenya sudah memutuskan tapi dokumen masih menulis TBD, (c) field `utxo_root` yang consensus-critical tapi tidak berspesifikasi.
- **Perbaikan yang disarankan (tanpa perubahan kode):** perbarui dokumen agar mencerminkan keputusan kode (trunkasi emisi, difficulty 1440/4×, reorg depth 100, mempool konfigurasi), hapus kontradiksi 42M vs 94.98M, deduplikasi working docs 35-41, tandai `TASKS_32_TO_34.md` sebagai superseded, dan putuskan sikap resmi terhadap sub-sistem P2P (hapus atau tulis ulang dokumentasi agar jujur pada implementasi NATS).