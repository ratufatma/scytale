# HARDENING VERIFICATION REPORT

**Status:** Final — Read-only verification. No code was modified.
**Tanggal:** 2026-09-09
**Branch / HEAD:** `main` `1eacb41`
**Metode:** Pengecekan langsung terhadap kode dan hasil eksekusi `cargo test` / `clippy` / `fmt`, dibandingkan dengan klaim di Work Record 46 & 47, `AUDIT_CHECKLIST_v0.3.0.md`, dan `SECURITY-THREAT-MODEL.md`.

---

## 1. Ringkasan Eksekutif

| Area Hardening | Verdict |
|---|---|
| Fase 1 (mnemonic wallet, indexer catch-up, fuzz) | ✅ **HARDENED** |
| Fase 2 — max reorg depth | ✅ **HARDENED** |
| Fase 2 — Wasm memory bound | ✅ **HARDENED** |
| Fase 2 — Go P2P fallback seed peers | ❌ **NOT IMPLEMENTED** (direktorat `network/` tidak ada) |
| Kontrol keamanan target model | ⚠️ Sebagian besar hadir, **3 kelemahan** ditemukan |

**Temuan kunci:**
- Semua test suite hardening **PASS**: codec fuzz (5), script fuzz (5), reorg depth (8), memory limits (4), wallet mnemonic (2 di antara 8), pin vault (2).
- Go P2P daemon **tidak ada** — konsisten dengan temuan sebelumnya bahwa transport sudah dimigrasi ke Rust/NATS (commit `2a53e14`). Work Record 47 item #1 disalin ke dokumentasi arsip tanpa dasar kode.
- `cargo clippy --workspace --all-targets` bersih dari error, namun ada **3 warning** (`scytale-mining`, `scytale-studio`).
- `cargo fmt --check` menemukan **1 file tidak terformat** (`apps/scytale-cli/src/bin/generate_genesis_keys.rs`).
- `scytale-script` dan `scytale-vm` **tidak mewarisi** workspace lint `float_arithmetic = "deny"`.
- HTTP gateway **bind `0.0.0.0:8332`** + CORS bebas + `POST /api/v1/tx` tanpa autentikasi → permukaan serangan di jaringan nyata.

---

## 2. Verifikasi Fase 1 — Work Record 46 (mnemonic, indexer, fuzz)

Sumber: `docs/archive/work-history/46-phase-1-hardening-mnemonic-indexer-catchup-fuzzing.md`

| # | Klaim | Kod | Kode file | Bukti | Verdict |
|---|---|---|---|---|---|
| 1 | WalletFile v2, field `mnemonic: Option<String>` | ✅ Yes | `apps/scytale-cli/src/wallet.rs` | L.47 `mnemonic: Option<String>`; v2 di `generate_with_mnemonic` (L.143) & `restore_from_mnemonic` (L.185) | ✅ |
| 2 | `generate_with_mnemonic` | ✅ | `wallet.rs` | L.106-155 | ✅ |
| 3 | `restore_from_mnemonic` | ✅ | `wallet.rs` | L.158-197, pakai `bip39::Mnemonic::parse_in_normalized` | ✅ |
| 4 | File permission 0600 | ✅ | `wallet.rs` | L.227 `.mode(0o600)` + test L.357-363; `generate_genesis_keys.rs` L.97-102 | ✅ |
| 5 | CLI `--mnemonic` & `--words <12\|24>` | ✅ | `main.rs` | L.193, L.196 | ✅ |
| 6 | `wallet restore --phrase` | ✅ | `main.rs` | L.203-216 | ✅ |
| 7 | `print_wallet_mnemonic_created` / `print_wallet_restored` | ✅ | `formatter.rs` | L.279-310, L.313-332 | ✅ |
| 8 | Dependency `bip39 = "2.1"` aktif | ✅ | Cargo.toml + `wallet.rs` | root L.37; dipakai L.126 & L.169 | ✅ |
| 9 | HTTP `BlocksQuery` (offset/from_height/order) | ✅ | `http_gateway.rs` | struct L.461-467, `get_blocks` L.469-531 | ✅ |
| 10 | Explorer reconciler 15 detik | ✅ | `explorer/server.mjs` | `reconcileBlocks()` L.372; `setInterval(..., 15000)` L.455 | ✅ |
| 11 | Fuzz codec — 5 test | ✅ | `tests/canonical_codec_fuzz_tests.rs` | 5 `#[test]` (noise, bit-flip, truncation, trailing, length headers) — **lulus** | ✅ |
| 12 | Fuzz script — 5 test | ✅ | `tests/script_fuzz_tests.rs` | 5 `#[test]` (random, budget DoS, stack overflow, unbalanced, overflow) — **lulus** | ✅ |

**Catatan kecil:** `wallet new --mnemonic` melewati `print_human_wallet_created` (B. Indonesia), bukan `print_wallet_mnemonic_created` — fungsi ada dan diekspor tapi tidak dipakai di jalur CLI saat ini (divergensi kosmetik, bukan security issue).

---

## 3. Verifikasi Fase 2 — Work Record 47 (reorg depth, Wasm, Go P2P)

Sumber: `docs/archive/work-history/47-phase-2-network-resilience-and-consensus-hardening.md`

| # | Klaim | Kode | Bukti | Verdict |
|---|---|---|---|---|
| 1 | Go P2P `network/` + `DefaultFallbackSeeds` + `--fallback-seed` | ❌ | Direktori `network/` **tidak ada**; 0 file `.go` di repo. Hanya ada di docs/CHANGELOG | ❌ **NOT IMPLEMENTED** |
| 2 | `ChainError::ReorgDepthExceeded` | ✅ | `crates/scytale-consensus/src/error.rs` L.43-46 | ✅ |
| 3 | `DEFAULT_MAX_REORG_DEPTH = 100` | ✅ | `chain.rs` L.65; re-export `lib.rs` L.12 | ✅ |
| 4 | `max_reorg_depth` field + builder + getter + setter | ✅ | `chain.rs` L.71, L.103-106, L.109-111, L.114-116 | ✅ |
| 5 | Enforcement di `process_block` | ✅ | `chain.rs` L.326-334 (reject, DAG dipertahankan utk arsip) | ✅ |
| 6 | `NodeConfig.max_reorg_depth` + flag `--max-reorg-depth` | ✅ | `config.rs` L.25,39; `main.rs` L.88-90 & L.166-168 | ✅ |
| 7 | Adopsi di semua path konstruksi `ChainTree` | ✅ | `node.rs` L.190-191, L.237-238, L.310-311 | ✅ |
| 8 | `MAX_WASM_MEMORY_PAGES = 64` (4 MiB) | ✅ | `scytale-vm/src/lib.rs` L.5,7,9 (`MAX_WASM_MEMORY_BYTES=4194304`) | ✅ |
| 9 | `VmError::MemoryLimitExceeded` + `StoreLimits` trap | ✅ | `lib.rs` L.18, L.48-54, pre/post check L.172-177, L.214-219 | ✅ |
| 10 | `chain_reorg_tests::test_max_reorg_depth_protection` | ✅ | L.398 + 3 skenario — **lulus** (8/8) | ✅ |
| 11 | `memory_limits_tests` | ⚠️ | ada **4** test (klaim bilang 3) — semua **lulus** | ✅ (ketlein claim) |

**Konklusi Fase 2:** Reorg-depth & Wasm memory **fully implemented & tested**. Go P2P **tidak ada** → item risilien jaringan tersebut harus ditandai `NOT IMPLEMENTED`; fungsi peer-discovery kini dipegang sistem NATS/async_nats, bukan daemon Go.

---

## 4. Verifikasi Kontrol Keamanan (SECURITY-THREAT-MODEL)

| # | Kontrol | Verdict | Bukti | Kelemahan |
|---|---|---|---|---|
| 1 | Zero float arithmetic | ⚠️ **Partial** | `float_arithmetic="deny"` di workspace lint, dipakai 10 crate | `scytale-script` & `scytale-vm` **tidak** `[lints] workspace=true` → tidak terproteksi |
| 2 | Argon2 PIN vault | ✅ | `pin_vault.rs`: Argon2id, 64 MiB, iterasi 3, ChaCha20-Poly1305, salt 32B, nonce 12B, Zeroize, PIN 6 digit, TracerPid check L.49-60 | Linux-only; `ITERATIONS=3` rendah utk OWASP (maksarma memory) |
| 3 | Wallet 0600 | ✅ | `wallet.rs` L.227 `.mode(0o600)` | di non-Unix tidak ada kontrol permission |
| 4 | Fail-closed consensus | ✅ | `node.rs` `pre_validate_reorg_branch` L.498-594; `process_block` tak pernah dipanggil saat gagal | `is_block_invalid` cek hanya candidate+parent — dimitigasi oleh pre_validate penuh |
| 5 | Bounded script engine | ✅ | `MAX_STACK_DEPTH=1024`, `MAX_ITEM_SIZE=520`, `DEFAULT_MAX_OPS_BUDGET=256` | — |
| 6 | Mempool DoS limits | ✅ | 5000 txs / 5 MB / fee 1000; eviction fee+descendants; cegah coinbase/dup/double-spend | — |
| 7 | BLAKE3 PoW | ✅ | `pow.rs` verify vs Target | — |
| 8 | Bech32 checksum | ⚠️ **Partial** | HRP `"scy"`, bech32 crate v0.11 | HRP `tscy` **tidak ada** di kode |
| 9 | Canonical codec bounds | ✅ | `MAX_VECTOR_LENGTH=16 MiB`, `LengthExceedsLimit`, `TrailingBytes`, capacity `.min(1024)` | — |
| 10 | Wasm sandbox (fuel + mem) | ✅ | wasmi, `consume_fuel(true)`, `add_fuel`, 64 pages | fuel host function rendah (flat 200/ed25519) → bisa disesuaikan |
| 11 | REDB atomicity | ✅ | `commit_block` single `write_tx` L.300-436; reorg & unwind sama | — |
| 12 | HTTP gateway bind/auth/CORS | ❌ **Weakness** | `DEFAULT_HTTP_BIND="0.0.0.0:8332"` (L.28), CORS `allow_origin(Any)` L.944-947, `POST /api/v1/tx` & `/api/v1/alias/bind` tanpa auth | terpapar public + write endpoint tanpa batas rate |

> `DEFAULT_HTTP_BIND = "0.0.0.0:8332"` adalah nilai yang **sudah ter-commit** di HEAD (bukan diff uncommitted).

---

## 5. Verifikasi Claim AUDIT_CHECKLIST_v0.3.0.md & AUDIT-MATRIX

- **Semua test suite yang dikutip AUDIT-MATRIX terverifikasi ada & lulus** (lihat §6). Tidak ditemukan test yang diklaim tapi tidak ada.
- CLAIM model "42M" yang di-flag di `DOCS_CONSISTENCY_REPORT` **masih** muncul di `docs/MONETARY-POLICY.md` L.65 (`10,500,000`) & L.116 (`42,000,000`), dan dirujuk oleh `AUDIT-MATRIX.md` L.34 sebagai query grep. §§ 5/11 kontradiktif dgn kode (94.98M / 28.98M reserve) → **belum dibereskan** meski dokumen audit sudah diarsipkan.

---

## 6. Hasil Eksekusi Test / Clippy / Fmt

```
scytale-consensus chain_reorg_tests : 8/8 OK          (incl. test_max_reorg_depth_protection)
scytale-vm memory_limits            : 4/4 OK
scytale-core codec fuzz             : 5/5 OK
scytale-script script fuzz          : 5/5 OK
scytale-cli (wallet etc.)           : 8/8 OK          (incl. test_wallet_mnemonic_generate_and_restore)
scytale-account pin_vault           : 2/2 OK          (round-trip + wrong-PIN; ~6.6s each — Argon2 real)
scytale-account account_wrapper     : 4/4 OK
scytale-mempool                     : 13/13 OK
scytale-storage                     : 13/13 OK
scytale-mining                      : 7/7 OK
scytale-core                        : 49/49 OK
scytale-consensus                   : 31+ OK
clippy --workspace --all-targets    : 0 error, 3 warning  (scytale-mining x1, scytale-studio x2)
cargo fmt --check                   : 1 file tidak terformat (generate_genesis_keys.rs)
```

Catatan: beberapa test memakan waktu (±6.6 s per test pin_vault) karena Argon2id sungguhan — ini justru konfirmasi parameter KDF diterapkan.

---

## 7. Tindakan yang Disarankan (untuk approval — bukan bagian verifikasi ini)

Prioritas:
1. **HIGH** — HTTP gateway: default bind sebaiknya `127.0.0.1:8332`; tambahkan auth (bearer untuk `POST`), rate limiting, dan CORS ketat. Write-endpoint (`POST /api/v1/tx`) sebaiknya opsional / non-aktif saat ter-expose.
2. **MEDIUM** — Tambahkan `[lints] workspace = true` ke `scytale-script` & `scytale-vm` agar `float_arithmetic` deny ikut berlaku.
3. **MEDIUM** — Perbarui `docs/MONETARY-POLICY.md` §5/§11 (hapus model 42M), selaraskan dengan kode 94.98M/28.98M (keputusan Option A di DECISION_RECOMMENDATIONS terkait).
4. **LOW** — `cargo fmt` pada `generate_genesis_keys.rs`; bereskan 3 warning clippy; perbaiki claim "3 test" → "4 test" di Work Record 47.
5. **LOW** — Tandai Work Record 47 item Go P2P sebagai `SUPERSEDED` (digantikan NATS) agar tidak dikira belum dikerjakan, atau tambahkan fallback seed NATS bila diinginkan.
6. **NON-CRITICAL** — HRP `tscy` hanya jika testnet diformalkan.

---

## 8. Lampiran — Status Working Tree (uncommitted)

`git status --short` menunjukkan reorganisasi dokumentasi yang **belum di-commit**: `docs/work/*` → `docs/archive/work-history/*` (47+ rename tracked), 4 file laporan audit ke `docs/archive/reports/`, README.md modified. Perubahan ini murni dokumentasi — tidak memengaruhi kode hardening yang diverifikasi.