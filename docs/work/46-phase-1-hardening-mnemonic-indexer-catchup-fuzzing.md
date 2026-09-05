# Work Record: 46. Phase 1 Hardening — Mnemonic BIP-39, Indexer Catch-Up, & Fuzz Testing

## Overview
Implementasi komprehensif paket pembaruan **Fase 1 (Hardening & Keamanan)** pada ekosistem Scytale. Pembaruan ini mencakup tiga area fundamental:
1. **Standardisasi Kunci Mnemonic BIP-39 & Pemulihan Dompet Deterministik (`scytale-cli`):** Memungkinkan pengguna menghasilkan frasa pemulihan 12 atau 24 kata, menurunkan pasangan kunci Ed25519 dan alamat Bech32 (`scy1...`) secara deterministik, serta memulihkan dompet tanpa kehilangan backward compatibility.
2. **Mekanisme Historical Catch-Up & Reconciler pada Indexer/Explorer (`scytale-node` & `explorer`):** Menambahkan filter rentang (`from_height`, `offset`, `order=asc/desc`) pada endpoint `/api/v1/blocks` node HTTP gateway, serta loop rekonsiliasi otomatis saat server Explorer menyala dan berkala (setiap 15 detik) untuk menutup *gap* historis blok.
3. **Adversarial & Property-Based Fuzz Testing (`scytale-core` & `scytale-script`):** Menambahkan test harness otomatis untuk menguji decoder biner kanonikal dan interpreter ScytaleScript terhadap ribuan mutasi acak, truncated stream, bit flips, dan payload berbahaya (*fail-closed validation*).

---

## Arsitektur & Karakteristik Desain

### 1. Alur Derivasi Kunci Mnemonic BIP-39 (Ed25519)
```text
Entropy Acak (128 / 256 bits via OsRng)
          │
          ▼
Frasa Mnemonic BIP-39 (12 atau 24 kata bahasa Inggris)
          │
          ▼
Seed HMAC-SHA512 (64 bytes / 512 bits)
          │
          ▼
32-Byte Secret Entropy Slice [0..32]
          │
          ▼
ed25519_dalek::SigningKey
    ├── Public Key: VerifyingKey (32 bytes)
    └── Address: BLAKE3(Public Key) -> Bech32 "scy1..."
```

* **Zero Memory Leak:** Entropi rahasia dihasilkan via `rand::rngs::OsRng` sistem operasi.
* **Backward Compatibility:** `WalletFile` versi 1 tanpa mnemonic tetap dapat dimuat dan ditransaksikan seperti biasa. Dompet baru dengan frasa mnemonic diberi tanda versi 2 dengan field `mnemonic: Option<String>` yang diserialisasi secara aman.

### 2. Alur Rekonsiliasi & Historical Catch-Up Indexer Explorer
```text
[ Explorer Startup / Heartbeat (15s) ]
                 │
                 ▼
  1. GET /api/v1/status (Dapatkan canonical_height node)
                 │
                 ▼
  2. Kueri SELECT MAX(height) FROM blocks di SQLite lokal
                 │
                 ▼
     Apakah local_height < canonical_height?
        ├── TIDAK: Selesai (Database up-to-date)
        └── YA   : 
                 │
                 ▼
  3. GET /api/v1/blocks?from_height=${local+1}&limit=50&order=asc
                 │
                 ▼
  4. Ekstraksi miner & upsertBlock() batch ke SQLite lokal
                 │
                 ▼
     Ulangi hingga database lokal sinkron dengan tip kanonikal
```

* **Resilience:** Jika node offline saat explorer menyala, reconciler menangkap error secara anggun (*graceful timeout*) dan mencoba kembali pada siklus berikutnya.
* **Eliminasi Data Gap:** Sekalipun koneksi in-process indexer mengalami *dropped packet* saat beban tinggi, reconciler secara otomatis melengkapi blok yang hilang.

### 3. Matriks Pengujian Fuzzing & Adversarial
* **Canonical Binary Codec (`canonical_codec_fuzz_tests.rs`):**
  * `test_fuzz_random_noise_inputs`: 1.000 buffer acak dengan panjang 0 s.d. 2048 bytes; memastikan parser tidak pernah panik (*zero panic*).
  * `test_fuzz_bit_flips_on_valid_payloads`: 500 mutasi bit-flipping pada payload transaksi kanonikal valid; memverifikasi penolakan terstruktur via `SerializationError`.
  * `test_fuzz_truncation_resilience`: Pengujian setiap prefix truncation dari panjang 0 hingga $N-1$ byte; memverifikasi perilaku *fail-closed*.
  * `test_fuzz_trailing_bytes_detection`: Penyisipan byte sampah di akhir payload valid; memverifikasi `SerializationError::TrailingBytes`.
  * `test_fuzz_malicious_length_headers`: Penyisipan panjang vektor berbahaya ($> 16\text{ MB}$, hingga $2^{32}-1$); memverifikasi `LengthExceedsLimit` terpicu sebelum terjadi alokasi memori heap.
* **ScytaleScript Interpreter (`script_fuzz_tests.rs`):**
  * `test_fuzz_random_script_bytes`: 1.000 kombinasi bytecode unlocking dan locking acak; menjamin eksekusi deterministik tanpa crash.
  * `test_fuzz_budget_exhaustion_dos`: Pengujian batas eksekusi opcodes (default 256) terhadap script tak terhingga (`OP_1 OP_DROP` berulang).
  * `test_fuzz_stack_overflow_dos`: Pengujian batas kedalaman stack maksimum (`MAX_STACK_DEPTH = 1000`).
  * `test_fuzz_unbalanced_conditionals`: Evaluasi perlakuan error `UnbalancedConditionals` pada `OP_IF`/`OP_ELSE`/`OP_ENDIF` yang tidak berpasangan.
  * `test_fuzz_arithmetic_overflow_extremes`: Pengujian operasi matematika ekstrem (`i64::MAX + 1`, `i64::MIN - 1`).

---

## Komponen & Perubahan Kode

1. **`Cargo.toml` & `apps/scytale-cli/Cargo.toml`**:
   * Mendaftarkan dependensi `bip39 = "2.1"` pada `[workspace.dependencies]` dan mengimpornya pada `scytale-cli`.
2. **`apps/scytale-cli/src/wallet.rs`**:
   * Menambahkan varian `WalletError::Mnemonic(String)`.
   * Menambahkan field `mnemonic: Option<String>` pada struct `WalletFile`.
   * Mengimplementasikan `WalletFile::generate_with_mnemonic(path, overwrite, word_count)`.
   * Mengimplementasikan `WalletFile::restore_from_mnemonic(path, phrase, overwrite)`.
3. **`apps/scytale-cli/src/main.rs` & `formatter.rs`**:
   * Menambahkan flag `--mnemonic` dan `--words <12|24>` pada subperintah `scytale-cli wallet new`.
   * Menambahkan subperintah baru `scytale-cli wallet restore --phrase "<words>"`.
   * Menambahkan fungsi format `print_wallet_mnemonic_created` dan `print_wallet_restored`.
4. **`apps/scytale-node/src/http_gateway.rs`**:
   * Memperluas `BlocksQuery` dengan parameter `offset: Option<usize>`, `from_height: Option<u64>`, dan `order: Option<String>`.
   * Memperbarui handler `get_blocks` untuk mendukung pemilahan dan pengurutan sekuensial menaik (*ascending*).
5. **`explorer/server.mjs`**:
   * Menambahkan fungsi `reconcileBlocks()` dengan polling gap-closing otomatis.
   * Menghubungkan rekonsiliasi ke siklus startup server dan interval heartbeat (15 detik).
6. **`crates/scytale-core/tests/canonical_codec_fuzz_tests.rs` [NEW]**:
   * Suite pengujian fuzzing decoder kanonikal (5 skenario pengujian adversarial).
7. **`crates/scytale-script/tests/script_fuzz_tests.rs` [NEW]**:
   * Suite pengujian fuzzing interpreter script (5 skenario pengujian ketahanan anti-DoS).

---

## Verifikasi & Hasil Uji

* **CLI Wallet Tests (`apps/scytale-cli/tests/wallet_p2pkh_tests.rs`):**
  * 8/8 tests passed (termasuk verifikasi determinisme derivasi 12-kata, 24-kata, penolakan frasa rusak, dan backward compatibility).
* **Node HTTP Gateway Tests (`apps/scytale-node/tests/http_gateway_tests.rs`):**
  * 8/8 tests passed (termasuk verifikasi kueri `/api/v1/blocks?from_height=0&limit=10&order=asc`).
* **Canonical Codec Fuzz Tests (`crates/scytale-core/tests/canonical_codec_fuzz_tests.rs`):**
  * 5/5 tests passed (1.000 permutasi acak, bit-flips, truncations, trailing bytes, malicious headers).
* **Script Fuzz Tests (`crates/scytale-script/tests/script_fuzz_tests.rs`):**
  * 5/5 tests passed (1.000 random bytecode, budget exhaustion, stack overflow, unbalanced conditionals, arithmetic overflows).
* **Explorer Server Syntax:**
  * `node --check explorer/server.mjs` passed (0 syntax error).
