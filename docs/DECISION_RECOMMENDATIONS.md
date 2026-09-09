# REKOMENDASI KEPUTUSAN FINAL — 4 ITEM (Untuk Persetujuan)

**Tanggal:** 2026-09-09
**Status:** DRAFT — Menunggu persetujuan, belum diterapkan ke dokumen mana pun
**Prinsip:** Tidak ada kode yang diubah; rekomendasi berbasis bukti aktual dari git history + implementasi.

---

## Memo 1: Status Resmi Subsystem Go P2P

### Fakta dari Git History (bukan asumsi)

| Bukti | Detail |
|-------|--------|
| **Commit `2a53e14`** | `feat: migrate P2P stack to async-nats and add wasm smoke guard` — **menghapus 38 file Go** (semua `network/cmd/*`, `network/internal/{bridge,gossip,peer,seeder,sync,wire}/*`, `go.mod`, `go.sum`) |
| **Commit `cb6cd33`** | `merge(p2p): integrate native async-nats networking and consensus propagation` — finalisasi migrasi |
| **File yang ditambahkan** | `apps/scytale-node/src/network/mod.rs` + `message.rs` (Rust, NATS), `apps/scytale-node/tests/nats_smoke.rs` |
| **Transport aktual** | Rust `async_nats` pub/sub: `scytale.v1.blocks.new`, `scytale.v1.mempool.tx`, `scytale.v1.nodes.heartbeat` |
| **Dokumen yang tertinggal** | README §P2P, P2P-NETWORK-SPEC, DNS-SEEDER-GUIDE, TASKS_32-38, AUDIT_CHECKLIST — semuanya masih mendeskripsikan arsitektur Go |

**Kesimpulan:** Bukan "kehilangan kode" melainkan **keputusan migrasi yang SUDAH dilakukan** (Go → Rust/async-nats). Yang salah hanya dokumentasi yang belum diselaraskan.

### Opsi Keputusan

| Opsi | Deskripsi | Risiko |
|------|-----------|--------|
| **Rekomendasi: A. NATS = resmi; Go = deprecated historic** | Dokumentasi ditulis ulang untuk mendeskripsikan transport NATS. Rujukan Go diberi label "deprecated (pre-v0.3.0)". `P2pSupervisor`/`scytale-bridge` (dead code) ditandai legacy | Rendah. Selaras dgn kode aktual. Kode Go bisa direstor dari git bila dibutuhkan |
| B. Restorasi Go dari git history | `git checkout 2a53e14^ -- network/` + re-integrasi | Tinggi. Menghidupkan kembali arsitektur yang sengaja dibuang; konflik dengan engine NATS yang sudah ter-tes |
| C. Biarkan stagnan | Tidak ada tindakan | Tinggi. Dokumentasi terus menyesatkan; audit akan gagal |

### File yang Terdampak (jika Opsi A disetujui)
- `README.md`, `docs/P2P-NETWORK-SPEC.md`, `docs/DNS-SEEDER-DEPLOYMENT-GUIDE.md`,
- `docs/TASKS_32_TO_38.md`, `docs/AUDIT_CHECKLIST_v0.3.0.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY-THREAT-MODEL.md`,
- `docs/work/13-p2p.md`, `35*`, `36*`, `37*`, `44*` — tambahkan banner "superseded by NATS"
- `CHANGELOG.md` — tambah entri migrasi

---

## Memo 2: Aturan Terminal Mining Reward

### Bukti Implementasi Aktual (`crates/scytale-consensus/src/lib.rs:25-52`)

```rust
pub const INITIAL_REWARD: Quanta = 10 * QUANTA_PER_SCY;        // 10 SCY
pub const HALVING_INTERVAL: u64 = 2_100_000;                   // 2.1M blok
pub const MINING_RESERVE_QUANTA: Quanta = 2_898_000_000_000_000; // 28.98M SCY
pub const MINING_REWARD_END_HEIGHT: u64 = 3_696_000;           // Terminal height

pub fn calculate_block_reward(height: u64) -> Quanta {
    if height >= MINING_REWARD_END_HEIGHT { return 0; }        // hard stop
    let halvings = height / HALVING_INTERVAL;
    if halvings >= 64 { 0 } else { INITIAL_REWARD >> halvings }
}
```

- Epoch 0 (0..2,099,999): 21,000,000 SCY
- Epoch 1 parsial (2,100,000..3,695,999): 7,980,000 SCY
- **Total mining = 28,980,000 SCY = MINING_RESERVE_QUANTA** (diuji `test_exact_mining_reserve_issuance`)
- Ini persis **Option A (Subsidy Hard Cap)** dari MONETARY-POLICY.md §6

### Opsi Keputusan

| Opsi | Deskripsi | Risiko |
|------|-----------|--------|
| **Rekomendasi: A. Formal-kan Option A sebagai FINAL** | Kode sudah mengimplementasikan hard cap; dokumentasi diperbarui agar menjadikan trunkasi di height 3,696,000 sebagai aturan resmi dan menghapus marker "REQUIRES RESOLUTION" | Rendah. Kode, test, dan tabel "locked parameters" semuanya sudah konsisten; hanya §6/§7 narasi yang tinggal |
| B. Pilih Option B/C (recalibration) | Mengubah kode konsensus | **Tinggi.** Melanggar genesis yang sudah berjalan; wajib hard fork; bertentangan dgn komitmen "genesis supply locked" |
| C. Biarkan TBD | Tidak ada tindakan | Tinggi. Kode sudah final; dokumen mempertanyakan kebijakan yang sudah berlaku |

### File yang Terdampak (jika Opsi A disetujui)
- `docs/MONETARY-POLICY.md` (§6 & §7: hapus "REQUIRES RESOLUTION", jadikan trunkasi rule)
- `docs/PROTOCOL-CONSTANTS.md` (jadikan terminal rule + height sebagai FINAL)
- `docs/POW-SPEC.md`, `docs/CONSENSUS-SPEC.md`, `docs/ECONOMIC-MODEL.md`
- `docs/work/01-monetary-policy.md`, `08-pow.md`, `18-protocol-audit-baseline.md`
- `crates/scytale-core/src/genesis.rs` (L20-21: perbaiki komentar stale "42M" → 94.98M) — *kode, hanya komentar; ditahan karena prinsip "jangan ubah kode"*

---

## Memo 3: Dokumentasi `utxo_root`

### Fakta Implementasi

| Item | Detail |
|------|--------|
| Field | `BlockHeader.utxo_root: Hash256` (32 byte) di posisi ke-4 dari 7 field |
| Konsekuensi | Header canonical = 120 byte; BlockID = BLAKE3(header 120-byte) |
| Merkle leaf | `compute_utxo_leaf = BLAKE3("SCYTALE_UTXO_LEAF_V1" \|\| txid \|\| index \|\| value \|\| locking_condition)` |
| Merkle tree | balanced binary; odd → duplicate last; parent = BLAKE3(left\|\|right); empty → Hash256::ZERO |
| Proof | `UtxoMerkleProof { outpoint, value, locking_condition, leaf_hash, audit_path: Vec<(Hash256, bool)>, leaf_index }` |
| Validasi | `BlockError::InvalidUtxoRoot` (digital validation, error.rs:104-108) |
| Status di spec | ABSEN total dari BLOCK-SPEC, UTXO-SPEC, HASHING-SPEC, POW-SPEC |

### Opsi Keputusan

| Opsi | Deskripsi | Risiko |
|------|-----------|--------|
| **Rekomendasi: A. Dokumentasikan sebagai fitur resmi (tanpa ubah kode)** | Tambahkan ke BLOCK-SPEC (header 7-field, 120 byte), UTXO-SPEC (bagian baru "State Commitment" dengan leaf formula + proof format), HASHING-SPEC (domain tag `SCYTALE_UTXO_LEAF_V1`). README §fitur sudah menyebut `utxo_root` — lengkapi | Rendah. Murni dokumentasi; mengunci format yang sudah diaudit |
| B. Ubah kode agar sesuai spec (hapus utxo_root) | Menghapus field dari header | **Sangat tinggi.** Breaking change total; merusak syncing, snapshot, semua state; mustahil tanpa hard fork |
| C. Panjang 120-byte ditulis sebagai "TBD" | Dokumentasi parsial | Sedang. Tetap ada gap antara kalangan implementasi |

### Catatan Penting
- Commit `3409721` (2026-09-08) `feat(consensus): lock genesis supply` & `32-compact-utxo-commitment.md` (work doc) menunjukkan `utxo_root` adalah fitur **Task 32 yang SUDAH SELESAI** — tinggal dipromosikan ke spec top-level.
- `docs/work/32-compact-utxo-commitment.md` sudah ada (208 baris) — sumber utama untuk diformalkan.

### File yang Terdampak (jika Opsi A disetujui)
- `docs/BLOCK-SPEC.md` (header + serialisasi, hapus ambiguitas field)
- `docs/UTXO-SPEC.md` (section "UTXO State Commitment": leaf hash, merkle, proof)
- `docs/HASHING-AND-SERIALIZATION-SPEC.md` (daftar domain tags)
- Referensi silang di `docs/POW-SPEC.md`, `docs/STORAGE-SPEC.md`, `docs/VALUE-PROVENANCE-SPEC.md`

---

## Memo 4: Klasifikasi Dokumen Work

### Fakta (audit 57 file di `docs/work/`)

| Kategori | Jumlah | Detail |
|----------|--------|--------|
| Duplikat identik (hyphen vs underscore) | **16 file** | Task 35,36,37,38,39,40,41 (7 pasang × 2) + 35a juga identik dgn 35b |
| Duplikat kontradiktif | 2 file | Task 34: hyphen "COMPLETED/PRODUCTION-READY" (143 baris) vs underscore "READY FOR EXECUTION" (161 baris) |
| Sudah dipromosikan ke top-level | 01-17, 37 | 18 file → specs di `docs/` |
| Orphan (belum punya top-level counterpart) | 18-48 (minus yang terpromosikan) | ±29 file |
| Superseded | `docs/TASKS_32_TO_34.md` | Digantikan `TASKS_32_TO_38.md` |
| Dalam progress | Task 39 (dua versi) | "IN PROGRESS" |
| Marker stale | 01-17 | `[ IN PROGRESS ]` frozen di pipeline section padahal COMPLETED |

### Opsi Keputusan

| Opsi | Deskripsi | Risiko |
|------|-----------|--------|
| **Rekomendasi: A. Arsipkan + deduplikasi** | Buat subdirektori `docs/archive/` untuk seluruh `work/`; pindahkan sebagai arsip sejarah. Deduplikasi dengan simpan versi canonical: hyphen utk 35-41, pertahankan BOTH task 34 beserta catatan konflik server-to-delete readability | Rendah. `work/` bukan rijukan operasional; specs top-level sudah ada |
| B. Hapus duplikat langsung | Hapus 16 file underscore + tulis `TASKS_32_TO_34.md` superseded | Rendah tapi menghilangkan jejak — tidak disarankan utk repo bersejarah |
| C. Biarkan | Tidak ada tindakan | Rendah namun membingungkan perambah repo |

### Sub-keputusan yang disarankan dalam Opsi A
1. **Canonical naming**: hyphenated (`34-multi-node-docker-chaos-and-fast-sync.md`) — konsisten dgn 01-33.
2. **Task 34**: simpan dua-duanya; di hyphenated tambah satu baris `> Conflict: v2 underscore (READY FOR EXECUTION) is a later draft; content supersedes this file` — atau langsung ganti isi hyphenated dgn isi underscore dan set status COMPLETED. **Direkomendasikan: ganti isi hyphenated dengan underscore (revisi terakhir), set status COMPLETED, hapus underscore.**
3. **TASKS_32_TO_34.md**: tambah banner `> SUPERSEDED by TASKS_32_TO_38.md` di header (jangan hapus).
4. **Marker `[ IN PROGRESS ]`** di 01-17: biarkan sebagai jejak (arsip), karena file utama di `docs/` sudah benar.
5. **Task 39 duplikat**: arsipkan sebagai "pending completion".

### File yang Terdampak (jika Opsi A disetujui)
- `docs/work/*` → `docs/archive/work/*` (atau `docs/archive/`)
- `docs/TASKS_32_TO_34.md` (banner superseded)
- Task 34 (resolusi konten)
- README jika mereferensikan `docs/work/`

---

## Ringkasan Rekomendasi

| # | Item | Rekomendasi | Perubahan Kode? | Tingkat Risiko |
|---|------|-------------|-----------------|----------------|
| 1 | Status Go P2P | NATS resmi; Go = deprecated historic | Tidak | Rendah |
| 2 | Terminal mining reward | Option A (hard cap @ height 3,696,000) = FINAL | Tidak | Rendah |
| 3 | Dokumentasi `utxo_root` | Dokumentasikan fitur existing (7-field, 120B, Merkle) | Tidak | Rendah |
| 4 | Klasifikasi work docs | Arsipkan `work/`, deduplikasi, tandai superseded | Tidak | Rendah |

**Keempat rekomendasi murni dokumentasi** — konsisten dengan instruksi "jangan mengubah kode". Setelah keempat keputusan disetujui, langkah berikutnya adalah:
1. Terapkan kecabutan ke dokumen
2. Scan ulang seluruh codebase untuk memastikan konsistensi penuh
3. Terbitkan laporan `DOCS_CONSISTENCY_REPORT.md` edisi final

---

## Formulir Persetujuan

Centang setiap item yang disetujui:

- [ ] **Memo 1 — Opsi A**: NATS resmi, Go deprecated historic
- [ ] **Memo 2 — Opsi A**: Option A hard cap = FINAL (dokumen diselaraskan)
- [ ] **Memo 3 — Opsi A**: Dokumentasikan `utxo_root` (7-field/120B/Merkle)
- [ ] **Memo 4 — Opsi A**: Arsipkan `work/`, deduplikasi, banner superseded; task 34 pakai versi underscore terbaru
- [ ] Opsi lain / modifikasi: ...