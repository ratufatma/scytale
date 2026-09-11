# Scytale Protocol — Technical Specification & Architecture Record: Task 33
## Fast Sync Wire Protocol (`getsnapshot` / `snapshot`) & State Synchronization

```text
Task ID       : 33
Task Name     : Fast Sync Wire Protocol (getsnapshot / snapshot) & State Synchronization
Phase         : Phase 3 — Protocol Hardening & State Authenticity
Target Modul  : network/internal/wire, network/internal/peer, network/cmd/scytale-p2p, crates/scytale-bridge, apps/scytale-node
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Chunked & Paginated Wire Stream, Fail-Closed Merkle Verification, DoS Rate-Limiting, Zero-Float
```

---

## 1. Arsitektur Sinkronisasi Kilat (Fast Sync Architecture)

Dengan tersedianya komitmen `utxo_root` pada `BlockHeader` (Task 32) dan fungsi `export_utxo_snapshot` / `apply_utxo_snapshot` di `scytale-storage`, Task 33 menghubungkan kapabilitas tersebut ke lapisan jaringan P2P (`network/` Go daemon) melalui protokol wire biner kanonikal.

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        Node Baru (Fast Sync Mode)                      │
│                                                                        │
│   1. Sinkronisasi Header Rantai Selesai (Verifikasi PoW & Cumulative)  │
│   2. Pilih Titik Checkpoint Finalized (misal: Tip - 100 blok)         │
│   3. Kirim: MsgGetSnapshot { BlockHash, ChunkIndex: 0 } ───────────►   │
│                                                                        │
│                                           Peer Sinkron (Full Node)     │
│                                           ┌────────────────────────┐   │
│                                           │ Ekspor Snapshot UTXO   │   │
│                                           │ Pecah per Chunk (2 MB) │   │
│   ◄─── 4. Balas: MsgSnapshot { ChunkIndex, TotalChunks, UTXOs... } ────┘   │
│                                                                        │
│   5. Ulangi ambil Chunk 1 .. N hingga tuntas                          │
│   6. Rekonstruksi State & Hitung Merkle Root:                          │
│      compute_utxo_merkle_root(snapshot_utxos) == header.utxo_root?     │
│      ├── Cocok  ──► Terapkan Atomik ke redb & Lanjut Normal Sync       │
│      └── Gagal  ──► TOLAK Snapshot, Putus Peer, Blacklist              │
└────────────────────────────────────────────────────────────────────────┘
```

### Invarian Kritis & Batas Keamanan:

1. **Transpor Terfragmentasi (Chunked Transfer):**
   - Ukuran per chunk dibatasi maksimal 2.000 entri UTXO atau $\le 2$ MB untuk mencegah kejenuhan alokasi memori buffer socket.
   - Frame header chunk: `BlockHash [32B] | ChunkIndex (4B) | TotalChunks (4B) | ItemCount (4B)`.

2. **Mitigasi DoS & Pembatasan Laju (Rate-Limiting):**
   - Permintaan `getsnapshot` memerlukan I/O pembacaan disk yang intensif. Peer melayani maksimal 1 permintaan `getsnapshot` per 30 detik per koneksi peer.
   - Hanya peer yang telah mencapai status tersinkronisasi penuh (`synced == true`) yang berhak melayani permintaan snapshot.

3. **Validasi Merkle Fail-Closed Sebelum Persistensi:**
   - Node penerima tidak boleh langsung menimpa tabel `UTXOS` di database lokal sebelum **seluruh chunk diterima lengkap** dan akar Merkle terbukti cocok 100% dengan `header.utxo_root`.

---

## 2. Struktur Data Wire Binary (`network/internal/wire/`)

### A. `MsgGetSnapshot` (Command: `"getsnap"`)

```go
type MsgGetSnapshot struct {
    BlockHash  [32]byte
    ChunkIndex uint32
}
```

- Ukuran payload tetap: 36 byte (`32B Hash + 4B LittleEndian Index`).

### B. `MsgSnapshot` (Command: `"snapshot"`)

```go
type UtxoWireEntry struct {
    TxID          [32]byte
    Index         uint32
    Value         uint64
    LockingScript []byte
}

type MsgSnapshot struct {
    BlockHash   [32]byte
    ChunkIndex  uint32
    TotalChunks uint32
    Entries     []UtxoWireEntry
}
```

- Format biner per entri: `TxID [32B] | Index [4B LE] | Value [8B LE] | ScriptLen [4B LE] | LockingScript [N Bytes]`.

---

## 3. Ekstensi IPC Bridge (`crates/scytale-bridge` & `apps/scytale-node`)

### Rust IPC Types
- `NodeRequest::ExportSnapshotChunk { block_hash: Hash256, chunk_index: u32, chunk_size: u32 }`
- `NodeResponse::SnapshotChunk { block_hash: Hash256, chunk_index: u32, total_chunks: u32, entries: Vec<UtxoEntryDto> }`
- `NodeRequest::ApplySnapshot { block_hash: Hash256, snapshot: UtxoSnapshotDto }`
- `NodeResponse::SnapshotApplied { block_hash: Hash256, utxo_count: usize }`

---

## 4. Rencana Pengujian & Quality Gates

1. **Go Unit & Race Tests (`network/`):**
   - Roundtrip serialisasi `MsgGetSnapshot` dan `MsgSnapshot`.
   - Batasan `MaxSnapshotChunkEntries` dan payload terpotong.
   - Deteksi bebas race condition: `go test -v -race ./...`.

2. **Rust Workspace Quality Gates:**
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace --all-targets`

3. **Live Network Integration:**
   - `./scripts/testnet_2node.sh`
   - `./scripts/testnet_fork_reorg.sh`
