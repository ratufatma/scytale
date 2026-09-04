# Work Record: 45. In-Process Non-Blocking Indexer Module

## Overview
Implementasi modul pengamat internal (*in-process observer*) pada biner node Scytale (`scytale-node`). Modul ini bertindak sebagai listener otonom yang mengekstraksi metadata blok kanonikal secara langsung dari pipeline komit penyimpanan/konsensus, lalu mendispatchkannya ke server Web Explorer eksternal melalui koneksi outbound HTTP/HTTPS POST. Modul dirancang dengan isolasi penuh pada dedicated OS thread untuk menjamin zero performance impact terhadap proses penambangan Proof-of-Work dan validasi konsensus.

---

## Arsitektur & Karakteristik Desain

```text
[ Thread Penyimpanan / Mining ]
          │
          │ 1. commit_block() sukses pada redb storage
          │ 2. sender.try_send(payload) (Latency < 1 µs, Non-blocking)
          ▼
[ crossbeam-channel (Bounded: 100) ]
          │
          │ 3. rx.recv() (Tertidur saat antrean kosong)
          ▼
[ Dedicated OS Thread: "scytale-indexer" ]
          │
          │ 4. Serialisasi JSON & ureq::post ke External Explorer
          ▼
[ Server Explorer API ] (Menerima metadata otentik tanpa perlu ekspos RPC publik)
```

1. **Isolasi Mutlak (Dedicated OS Thread):**
   Worker indexer berjalan pada thread terpisah (`std::thread::Builder::new().name("scytale-indexer".into())`). Kegagalan koneksi jaringan, latensi HTTP, maupun timeout server explorer tidak akan pernah menahan (*block*) thread miner atau siklus validasi konsensus.
2. **Non-Blocking Ingress Hook (`try_send`):**
   Komunikasi antar-thread menggunakan saluran terbatas (*bounded channel*) berkapasitas 100 (`crossbeam-channel`). Jika buffer penuh karena koneksi explorer lambat, payload dibuang (*dropped*) tanpa membebani memori dan tanpa menahan pemanggil.
3. **Resilience & Safe Retry:**
   Worker mengimplementasikan mekanisme *retry loop* maksimal 3 kali dengan jeda tidur 2 detik saat menghadapi galat jaringan/transport. Setelah 3 percobaan gagal, payload dilepaskan secara aman agar worker dapat melanjutkan pemrosesan blok berikutnya tanpa *deadlock*.
4. **Otentikasi Aman:**
   Mendukung header `Authorization: Bearer <api_key>` opsional untuk mengamankan komunikasi ke endpoint explorer privat.
5. **Aktivasi Bersyarat:**
   Modul indexer hanya diinisialisasi jika parameter CLI `--explorer-url <URL>` diberikan. Jika diabaikan, node beroperasi normal tanpa overhead thread indexer.

---

## Komponen & Perubahan Kode

### 1. Workspace & Package Dependencies
- **`Cargo.toml` (Workspace Root)**:
  - Menetapkan `default-members = ["apps/scytale-node"]` agar `cargo run` langsung menjalankan daemon node.
  - Mendaftarkan `crossbeam-channel = "0.5"` dan `ureq = { version = "2.10", features = ["json", "tls"] }` pada `[workspace.dependencies]`.
- **`apps/scytale-node/Cargo.toml`**:
  - Mengimpor `crossbeam-channel` dan `ureq` dari workspace.

### 2. Modul Indexer Internal (`apps/scytale-node/src/indexer/mod.rs` [NEW])
Menyediakan domain model dan thread worker loop:
- **`BlockPayload`**: Struct JSON yang dikirimkan ke explorer:
  - `height`: Tinggi blok kanonikal (`u64`).
  - `hash`: Hash blok BLAKE3 format heksadesimal lowercase (`String`).
  - `prev_hash`: Hash induk format heksadesimal lowercase (`String`).
  - `miner`: Alamat penerima subsidi coinbase (Bech32 `scy1...` jika P2PKH standar, atau representasi heksadesimal untuk script kustom).
  - `timestamp`: Unix timestamp blok (`u64`).
  - `tx_count`: Total transaksi dalam blok (`usize`).
  - Fungsi helper `BlockPayload::from_block(&block, height)` untuk ekstraksi otomatis.
- **`IndexerHandle`**: Pembungkus `crossbeam_channel::Sender<BlockPayload>` dengan method non-blocking `try_send`.
- **`start_indexer(target_url, api_key)`**: Menginisialisasi saluran bounded dan memicu OS thread `scytale-indexer`.
- **`worker_loop`**: Loop pemrosesan dengan `ureq::Agent` bertimeout 10 detik dan siklus retry otomatis.

### 3. Re-Exports (`apps/scytale-node/src/lib.rs`)
- Mengekspos `pub mod indexer;`.
- Re-export `start_indexer`, `BlockPayload`, `IndexerHandle`, serta helper `commit_block`.

### 4. Konfigurasi Node (`apps/scytale-node/src/config.rs`)
- Menambahkan field opsional pada `NodeConfig`:
  - `pub explorer_url: Option<String>`
  - `pub indexer_key: Option<String>`

### 5. Hook Penyimpanan & Siklus Blok (`apps/scytale-node/src/node.rs`)
- Menambahkan fungsi helper publik:
  ```rust
  #[allow(clippy::result_large_err)]
  pub fn commit_block(
      storage: &StorageEngine,
      block: &Block,
      height: u64,
      cumulative_work: [u64; 4],
      indexer_handle: Option<&IndexerHandle>,
  ) -> Result<(), scytale_storage::StorageError> {
      storage.commit_block(block, height, cumulative_work)?;
      if let Some(indexer) = indexer_handle {
          let payload = BlockPayload::from_block(block, height);
          let _ = indexer.sender.try_send(payload);
      }
      Ok(())
  }
  ```
- Menambahkan field `indexer: Option<Arc<IndexerHandle>>` pada struct `Node`.
- Menyediakan method mutator `Node::set_indexer(&mut self, indexer: IndexerHandle)`.
- Mengintegrasikan hook `commit_block` pada:
  - Inisialisasi Genesis Block saat bootstrap database baru (`Node::recover`).
  - Penerimaan blok valid dari jaringan / RPC (`Node::submit_block`), termasuk dispatch seluruh blok hasil reorganisasi rantai (*chain reorg*).
  - Loop penambang Proof-of-Work otonom (`mining_worker_loop`).

### 6. Integrasi CLI (`apps/scytale-node/src/main.rs`)
- Menambahkan argumen CLI:
  - `--explorer-url <URL>`: URL target HTTP POST explorer.
  - `--indexer-key <KEY>`: Bearer token otentikasi indexer.
- Fleksibilitas pemanggilan: Dapat dipanggil via sub-perintah `start` maupun langsung pada root binary:
  - `scytale-node start --explorer-url http://127.0.0.1:8080 --mine`
  - `scytale-node --explorer-url http://127.0.0.1:8080 --mine`
- Menginisialisasi `start_indexer` dan menyematkannya ke state `Node` sebelum daemon dimulai.

---

## Hasil Verifikasi & Pengujian

### 1. Kompilasi & Linter
- `cargo check`: Berhasil tanpa error atau peringatan.
- `cargo clippy -p scytale-node`: Bersih (0 warning).

### 2. Unit Testing Modul Indexer
```text
running 6 tests
test indexer::tests::test_indexer_bounded_channel_overflow ... ok
test indexer::tests::test_block_payload_from_block ... ok
test indexer::tests::test_indexer_try_send_success ... ok
test indexer::tests::test_indexer_receiver_dropped_no_panic ... ok
test indexer::tests::test_block_payload_serialization ... ok
test indexer::tests::test_start_indexer_spawn ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured
```

### 3. Regresi Keseluruhan Test Suite Node
- Seluruh 36 test unit dan integrasi pada `scytale-node` (mempool, consensus, storage, http gateway, passbook, lifecycle) lulus 100%.

### 4. Uji Isolasi Non-Blocking (Dead Network Target)
- Perintah uji:
  ```bash
  cargo run -- --explorer-url http://127.0.0.1:9999 --mine
  ```
- **Hasil:**
  - Penambang terus aktif menambang dan memproduksi blok.
  - Worker indexer mendeteksi `Connection refused` pada port 9999, melakukan percobaan ulang ke-1, ke-2, dan ke-3 dengan interval 2 detik, kemudian membuang payload setelah kuota retry habis.
  - **Kesimpulan:** Kegagalan jaringan server indexer terisolasi penuh dan tidak mengganggu proses konsensus/mining.

### 5. Uji Pengiriman Otentik (Authentic HTTP Delivery)
- Listener lokal dijalankan pada port 8088.
- Node dijalankan dengan parameter:
  ```bash
  cargo run -- --explorer-url http://127.0.0.1:8088 --indexer-key secret123 --mine
  ```
- **Hasil Verifikasi Listener:**
  - Header otentikasi diterima: `Authorization: Bearer secret123`
  - Header request: `Content-Type: application/json`
  - Payload JSON valid diterima:
    ```json
    {
      "height": 0,
      "hash": "e09cabcb1b8de68d89578f4cc9d1a338bcc0da4d66c32a2c8484b9805d356949",
      "prev_hash": "0000000000000000000000000000000000000000000000000000000000000000",
      "miner": "010203",
      "timestamp": 0,
      "tx_count": 1
    }
    ```
  - Node menerima respons `200 OK` dan mencatat log:
    ```text
    INFO scytale_node::indexer: indexer dispatched block metadata to explorer successfully height=0 status=200
    ```
