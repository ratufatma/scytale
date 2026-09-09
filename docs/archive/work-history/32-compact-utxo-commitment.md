# Scytale Protocol — Technical Specification & Architecture Record: Task 32
## Compact UTXO Commitment (utxo_root in BlockHeader) & Fast Sync Engine

```text
Document ID   : SPEC-TASK-32
Task ID       : 32
Task Name     : Compact UTXO Commitment (utxo_root in BlockHeader) & Fast Sync Engine
Phase         : Phase 3 — Protocol Hardening & State Authenticity
Target Crates : crates/scytale-core, crates/scytale-storage, crates/scytale-mining, apps/scytale-node
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Canonical Lexicographical Merkle Tree, Post-State Commitment, Zero Float, Fail-Closed
```

---

## 1. Latar Belakang & Masalah (Problem Statement)

Sebelum Task 32, node Scytale melakukan sinkronisasi dengan metode **Full History Replay (Initial Block Download / IBD)**:

* Setiap node baru yang bergabung harus mengunduh setiap blok mulai dari Blok #0 (Genesis) hingga blok kanonikal puncak (*tip*).
* Setiap transaksi historis harus dieksekusi ulang satu per satu melalui `ScriptEngine`, UTXO dikurangkan dan ditambahkan secara bertahap.
* Seiring rantai blok bertumbuh ke ribuan atau jutaan blok, waktu sinkronisasi node baru membengkak secara linear $O(N)$ dan membebani I/O disk secara masif.

**Solusi Task 32:**
Menyematkan komitmen kriptografis status UTXO aktif (**`utxo_root: Hash`**) langsung ke dalam `BlockHeader`.

1. **Pohon Merkle Kanonikal UTXO:** Setiap blok mengunci komitmen kriptografis dari seluruh koin yang belum dibelanjakan (*unspent coins*) segera setelah transaksi pada blok tersebut diaplikasikan (*post-state commitment*).
2. **Fondasi Fast Sync (State Snapshot):** Node baru dapat mengunduh hanya header rantai (verifikasi PoW), lalu mengunduh *State Snapshot* pada ketinggian tertentu, memverifikasi `BLAKE3_Merkle(snapshot) == header.utxo_root`, dan langsung melompat ke ketinggian tersebut tanpa memproses jutaan transaksi lama.

---

## 2. Invarian Arsitektur & Perhitungan Merkle Root UTXO

```text
┌────────────────────────────────────────────────────────────────────────┐
│                          UTXOS Table (redb)                            │
│                                                                        │
│   OutPoint_1 (000...0:0) ──► Leaf_1 = BLAKE3("UTXO_LEAF_V1" || ...)    │
│   OutPoint_2 (000...0:1) ──► Leaf_2 = BLAKE3("UTXO_LEAF_V1" || ...)    │
│   ...                                                                  │
│   OutPoint_N (fff...f:3) ──► Leaf_N = BLAKE3("UTXO_LEAF_V1" || ...)    │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                         Urutan Leksikografis Kunci
                                    │
                                    ▼
       ┌─────────────────────────────────────────────────────────┐
       │             Canonical Balanced Merkle Tree              │
       │                                                         │
       │                        [UTXO ROOT]                      │
       │                       /           \                     │
       │                 Node_A             Node_B               │
       │                 /    \             /    \               │
       │             Leaf_1  Leaf_2     Leaf_3  Leaf_4           │
       └────────────────────────────┬────────────────────────────┘
                                    │
                                    ▼
       ┌─────────────────────────────────────────────────────────┐
       │             BlockHeader (Post-State Hash)               │
       │   - version                                             │
       │   - previous_block_hash                                 │
       │   - merkle_root  (Transactions Tree)                    │
       │   - utxo_root    (Active UTXO Set Merkle Root) ◄── BARU │
       │   - timestamp                                           │
       │   - difficulty                                          │
       │   - nonce                                               │
       └─────────────────────────────────────────────────────────┘
```

### Invarian Kritis:

1. **Spesifikasi Daun Merkle Kanonikal (Canonical Leaf Preimage):**
Setiap entri UTXO aktif dipetakan menjadi leaf hash 32-byte:

$$\text{Leaf} = \text{BLAKE3}\Big(\text{"SCYTALE\_UTXO\_LEAF\_V1"} \,\Vert{}\, \text{txid (32B)} \,\Vert{}\, \text{index (4B LE)} \,\Vert{}\, \text{value\_quanta (8B LE)} \,\Vert{}\, \text{locking\_script}\Big)$$

2. **Urutan Deterministik (Strict Lexicographical Ordering):**
Daun disusun berurutan berdasarkan kunci unik `OutPoint`:
* Urut primer: `txid` secara leksikografis menaik (`a < b`).
* Urut sekunder: `index` integer 32-bit menaik (`0, 1, 2...`).
*(Tabel `UTXOS` di basis data `redb` secara native sudah tersimpan dalam urutan B-Tree leksikografis berdasarkan kunci serialized `OutPoint`).*

3. **Pohon Merkle Biner Seimbang:**
* Jika jumlah daun ganjil, daun terakhir digandakan untuk membentuk pasangan cabang.
* Pasangan hash digabungkan: $\text{Parent} = \text{BLAKE3}(\text{Left} \,\Vert{}\, \text{Right})$.
* Jika himpunan UTXO kosong (state awal mutlak): $\text{utxo\_root} = \mathbf{0}_{32}$.

4. **Post-State Consensus Rule:**
* `block.header.utxo_root` wajib mencerminkan keadaan tabel UTXO **setelah** seluruh transaksi dalam blok (termasuk output coinbase penambang) diterapkan, dan setelah semua output `OP_RETURN` dibuang.
* Jika hasil kalkulasi pada validator berbeda 1 bit saja dengan `block.header.utxo_root`, blok ditolak mentah-mentah (*fail-closed*) dengan error `BlockError::InvalidUtxoRoot`.

---

## 3. Core Primitives Update (`crates/scytale-core`)

### A. Struktur BlockHeader (`crates/scytale-core/src/block.rs`)
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub version: u32,
    pub previous_block_hash: Hash,
    pub merkle_root: Hash,
    pub utxo_root: Hash, // BARU: Komitmen status UTXO
    pub timestamp: u64,
    pub difficulty: u32,
    pub nonce: u64,
}
```

### B. Header Hashing (`BlockHeader::compute_hash`)
Hasher BLAKE3 memuat secara serial:
`version` (4B LE), `previous_block_hash` (32B), `merkle_root` (32B), `utxo_root` (32B), `timestamp` (8B LE), `difficulty` (4B LE), dan `nonce` (8B LE).

### C. Utilitas Merkle UTXO (`crates/scytale-core/src/utxo.rs`)
```rust
pub fn compute_utxo_leaf(outpoint: &OutPoint, output: &TxOut) -> Hash {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"SCYTALE_UTXO_LEAF_V1");
    hasher.update(outpoint.txid.as_bytes());
    hasher.update(&outpoint.index.to_le_bytes());
    hasher.update(&output.value.to_le_bytes());
    hasher.update(&output.locking_condition);
    Hash::from_bytes(*hasher.finalize().as_bytes())
}

pub fn compute_utxo_merkle_root(mut leaves: Vec<Hash>) -> Hash {
    if leaves.is_empty() {
        return Hash::ZERO;
    }
    while leaves.len() > 1 {
        if leaves.len() % 2 == 1 {
            let last = *leaves.last().unwrap();
            leaves.push(last);
        }
        let mut next_level = Vec::with_capacity(leaves.len() / 2);
        for chunk in leaves.chunks_exact(2) {
            let mut hasher = blake3::Hasher::new();
            hasher.update(chunk[0].as_bytes());
            hasher.update(chunk[1].as_bytes());
            next_level.push(Hash::from_bytes(*hasher.finalize().as_bytes()));
        }
        leaves = next_level;
    }
    leaves[0]
}
```

---

## 4. Storage Layer Extension (`crates/scytale-storage/src/engine.rs`)

1. **Method `compute_utxo_root`:**
   - Membaca seluruh entri dari tabel `UTXOS` yang tersimpan (kunci `OutPoint` otomatis terurut leksikografis di B-Tree `redb`).
   - Memetakan setiap pasangan `(outpoint, output)` ke `compute_utxo_leaf`.
   - Menghitung dan mengembalikan `compute_utxo_merkle_root(leaves)`.

2. **Snapshot Engine (Export & Import):**
   - `export_utxo_snapshot(&self) -> Result<UtxoSnapshotDto, StorageError>`
   - `apply_utxo_snapshot(&self, snapshot: &UtxoSnapshotDto) -> Result<(), StorageError>`:
     - Memvalidasi `compute_utxo_merkle_root` terhadap data snapshot sama dengan `snapshot.expected_root`.
     - Membersihkan tabel `UTXOS` lama, mengisi dengan entri snapshot secara atomik dalam satu transaksi tulis `redb`.

---

## 5. Mining & Node Consensus Pipeline

1. **Perakitan Template Penambang (`crates/scytale-mining/src/worker.rs`):**
   - Saat merakit template blok (`build_template`), simulasikan mutasi UTXO sementara (menghapus UTXO yang dibelanjakan oleh transaksi terpilih, menambahkan output baru transaksi terpilih dan coinbase reward).
   - Hitung prospective `utxo_root`, lalu sematkan ke `header.utxo_root`.

2. **Validasi Konsensus Node (`apps/scytale-node/src/node.rs`):**
   - Setelah menerapkan transaksi ke database staging / transaksi commit:
     ```rust
     let calculated_root = storage.compute_utxo_root(&txn)?;
     if block.header.utxo_root != calculated_root {
         return Err(NodeError::InvalidBlock(BlockError::InvalidUtxoRoot));
     }
     ```

---

## 6. Adaptasi Genesis Block & Tests

1. **Genesis UTXO Root:**
   - Genesis Block memuat transaksi Coinbase tunggal.
   - Daun tunggal dihitung dari output Coinbase Genesis, sehingga `genesis_block.header.utxo_root == compute_utxo_leaf(&genesis_coinbase_outpoint, &genesis_coinbase_output)`.
2. **Penyesuaian Test Fixtures:**
   - Seluruh fixture tes blok dan mock header di `crates/scytale-core`, `apps/scytale-node`, dan `crates/scytale-storage` disesuaikan agar menyertakan `utxo_root` yang valid.

---

## 7. Rencana Verifikasi & Quality Gates

1. **Kompilasi & Linting:**
   ```bash
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   ```
2. **Workspace Test Suite:**
   ```bash
   cargo test --workspace --all-targets
   (cd network && go test -v -race ./...)
   ```
3. **Live Network Testnet:**
   ```bash
   ./scripts/testnet_2node.sh
   ./scripts/testnet_fork_reorg.sh
   ```
