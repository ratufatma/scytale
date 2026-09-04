# Scytale Protocol — Technical Specification & Milestone Record: Tasks 28–31

```text
Project Scope  : Scytale Layer-1 Protocol
Milestone Span : Phase 3 (Programmable Consensus, Network Autonomy & Client Tooling)
Current Status : 129 Workspace Tests PASS | 0 Race Conditions (Go) | 0 Clippy Warnings | Zero Float Arithmetic
```

---

## RINGKASAN CAPAIAN ARSITEKTUR

Paruh kedua Fase 3 mematangkan Scytale dari protokol konsensus dasar menjadi ekosistem jaringan terdesentralisasi yang mandiri, tahan terhadap serangan DoS, memiliki mekanisme pasar biaya ruang blok, dan menyediakan antarmuka pengguna yang aman dengan kode koreksi kesalahan:

```text
┌────────────────────────────────────────────────────────────────────────┐
│                        SCYTALE PROTOCOL STACK                          │
│                                                                        │
│  [ UI / Explorability ]                                                │
│  • Embedded Web Explorer (Task 31): Live Gauges & Mempool Inspector    │
│  • Bech32 Address Format (Task 30): scy1... (BCH Error Detection)      │
│                                                                        │
│  [ Transaction & Fee Layer ]                                           │
│  • Priority Mempool (Task 28): Zero-Float Milli-Quanta/Byte Fee Density│
│  • Anti-Spam DoS Defense: Bounded 5K Txs / 5 MB & Cascade Eviction     │
│  • Miner Subsidy + Fee Accrual: Otomatis masuk ke Coinbase             │
│                                                                        │
│  [ P2P Swarm Intelligence ]                                            │
│  • Autonomous Discovery (Task 29): Wire getaddr / addr Messages        │
│  • Persistent AddrBook (peers.json) with Exponential Backoff           │
│  • Auto-Dialer Goroutine (Target: 8 Outbound Peers)                    │
└────────────────────────────────────────────────────────────────────────┘
```

---

## TASK 28: DYNAMIC FEE MARKET & PRIORITY MEMPOOL EVICTION

* **Tujuan:** Mengganti antrean FIFO sederhana dengan pasar lelang ruang blok (*block-space auction*) deterministik yang memprioritaskan transaksi bernilai densitas biaya tinggi serta melindungi node validator dari serangan banjir transaksi sampah (*dust spam*).
* **Komponen:** `crates/scytale-core/src/transaction.rs`, `crates/scytale-mempool/`, `crates/scytale-mining/src/worker.rs`, `apps/scytale-node/`
* **Arsitektur & Invarian:**
  * **Zero-Float Fee Density:** Densitas biaya dihitung dalam satuan integer murni *milli-quanta per byte* untuk mematuhi `#![deny(clippy::float_arithmetic)]`:

    $$\text{Fee Rate} = \frac{\text{Fee (quanta)} \times 1.000}{\text{Serialized Size (bytes)}}$$

  * **Dual-Index Memory Layout:** Menggabungkan `HashMap<Hash, MempoolEntry>` untuk akses instan via TxID dengan `BTreeSet<PriorityKey>` untuk pengurutan prioritas:

    $$\text{PriorityKey} = (\text{fee\_rate } \mathbf{DESC}, \; \text{added\_time } \mathbf{ASC}, \; \text{txid } \mathbf{ASC})$$

  * **Kapasitas Terbatas & Cascade Eviction:**
    * Batas keras: Maksimum 5.000 transaksi atau total ukuran memori 5.000.000 byte (~5 MB).
    * Batas bawah: `MIN_RELAY_FEE_RATE = 1.000` milli-quanta/byte (setara 1 quantum/byte).
    * Saat kapasitas terlampaui, jika transaksi baru menawarkan `fee_rate` lebih tinggi daripada transaksi terendah (`lowest_entry`), transaksi terendah beserta turunannya digusur (*evicted*). Jika tidak, transaksi baru ditolak (`MempoolFull`).
  * **Miner Fee Accrual:** Saat perakitan template blok, total fee transaksi yang diambil otomatis diakumulasikan ke nilai transaksi Coinbase penambang:

    $$\text{Coinbase Value} = \text{Current Subsidy} + \sum \text{Transaction Fees}$$

---

## TASK 29: DYNAMIC P2P PEER DISCOVERY (`getaddr` / `addr` WIRE PROTOCOL)

* **Tujuan:** Mengeliminasi ketergantungan konfigurasi peering statis manual (`--peer`), memungkinkan node menemukan tetangga secara otonom dalam jaringan P2P terdistribusi.
* **Komponen:** `network/internal/wire/`, `network/internal/peer/`, `network/cmd/scytale-p2p/`
* **Arsitektur & Invarian:**
  * **Protokol Wire Binary Baru:**
    * `CmdGetAddr = "getaddr"`: Permintaan daftar peer aktif tanpa payload.
    * `CmdAddr = "addr"`: Payload daftar alamat biner kanonikal (maksimal 1.000 alamat per pesan). Setiap entri berukuran 34 byte: `Timestamp (8B) | Services (8B) | IP (16B) | Port (2B BigEndian)`.
  * **Komponen Thread-Safe `AddrBook`:**
    * Filtrasi `IsRoutable`: Memblokir alamat loopback (`127.0.0.0/8`), RFC1918 private LAN (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`), multicast, dan port 0 (kecuali flag `--allow-local-peers` aktif pada mode devnet).
    * Algoritma *Exponential Backoff* saat mencoba mendial ulang peer yang gagal:

      $$\text{Backoff} = \min\left(2\text{ jam}, \; 5\text{ detik} \times 2^{\text{attempts}}\right)$$

    * Persistensi disk atomik ke `peers.json` menggunakan file temporer dan penggantian atomik file (*rename*).
  * **Lifecycle Pertukaran Peer & Auto-Dialer:**
    * Pertukaran discovery otomatis: Saat jabat tangan (*handshake*) selesai via `verack`, node inisiator segera mengirim `getaddr`.
    * Goroutine `autoDialerLoop`: Berjalan berkala memeriksa jumlah koneksi keluar (*outbound*). Jika berada di bawah `maxOutbound` (default: 8), goroutine secara acak memilih kandidat dari `AddrBook` untuk dihubungi.

---

## TASK 30: HUMAN-READABLE BECH32 ADDRESS ENCODING (`scy1...`)

* **Tujuan:** Mengganti format alamat heksadesimal 64-karakter yang rawan kesalahan ketik dengan alamat ramah pengguna berstandar Bech32 yang memiliki deteksi kesalahan matematis.
* **Komponen:** `crates/scytale-core/src/address.rs`, `apps/scytale-cli/`, `apps/scytale-node/src/http_gateway.rs`
* **Arsitektur & Invarian:**
  * **Standar BIP-173 Bech32:**
    * Human-Readable Part (HRP): `scy`.
    * Pemisahan karakter: Angka `1`.
    * Konversi Radix: Memetakan 32 byte hash publik BLAKE3 (256-bit) ke dalam representasi 5-bit array (52 elemen data) ditambah 6 karakter BCH Checksum.
  * **Deteksi Kesalahan Matematis (BCH Code):** Mampu mendeteksi secara pasti hingga 4 kesalahan pengetikan karakter acak dan kesalahan transposisi posisi karakter yang bersebelahan.
  * **Dual Backward-Compatible Parser (`Address::parse`):**
    * Menerima format Bech32 berawalan `scy1...` (bersifat case-insensitive).
    * Menerima fallback format legacy hex 64-karakter (dengan/tanpa prefix `0x`) agar skrip otomatisasi dan basis data lama tidak mengalami kerusakan.
  * **Integrasi End-to-End:**
    * Subperintah dompet `scytale-cli wallet new` dan `wallet info` menampilkan dan menyimpan alamat dalam format `scy1...`.
    * Subperintah `transfer-p2pkh --to <addr>` menerima kedua format alamat secara transparan.
    * Endpoint HTTP Gateway `/api/v1/passbook/:lock` otomatis mendekode alamat `scy1...` ke hash 32-byte untuk inspeksi tabungan Passbook.

---

## TASK 31: EMBEDDED WEB EXPLORER UPGRADE (BECH32 DISPLAY & MEMPOOL INSPECTOR)

* **Tujuan:** Memperkaya antarmuka Web Explorer tersemat (*zero-dependency embedded SPA*) untuk menampilkan visualisasi pasar biaya mempool live dan parsing alamat Bech32 tanpa pustaka eksternal.
* **Komponen:** `apps/scytale-node/src/http_gateway.rs`, `explorer/index.html`
* **Arsitektur & Invarian:**
  * **Pengayaan Backend HTTP DTO:**
    * `analyze_locking_script`: Mengurai tipe skrip transaksi secara otomatis menjadi `p2pkh`, `op_return`, atau `custom`.
    * `TxOutputDto`: Menyediakan field `address` (`"scy1..."`), `script_type`, dan `op_return_payload` (string teks UTF-8 atau heksadesimal).
    * `MempoolStatusDto`: Menyertakan kapasitas batas `max_count: 5000`, `max_bytes: 5000000`, dan `min_relay_fee_milli: 1000`.
  * **Mempool & Fee Market Inspector UI:**
    * Progress bar kapasitas visual untuk memantau slot transaksi (`0/5.000 txs`) dan memori (`0 KB/5.000 KB`) dengan gradasi warna adaptif (Hijau $<70\%$, Kuning $70-90\%$, Merah $\ge 90\%$).
    * Kartu telemetri pasar biaya: Menampilkan *Total Pending Fees* dalam desimal koin SCY dan *Min Relay Floor* (`1.00 q/B`).
    * Tabel antrean transaksi terurut prioritas lengkap dengan rank prioritas (#1, #2...), ukuran byte, nilai fee, rasio densitas (`X.XX q/B`), dan badge risiko penggusuran.
  * **Formatting Bech32 & Interaktivitas:**
    * Alamat dipotong rapi (`scy1qpzr...5ptk`) dengan tombol salin satu-klik (*copy-to-clipboard*).
    * Badge hijau `P2PKH` dengan tautan langsung ke buku tabungan (*Passbook*).
    * Badge ungu `DATA / OP_RETURN` untuk output metadata dokumen/bukti komitmen.
    * Kotak pencarian universal mendukung input alamat `scy1...` untuk langsung membuka modal riwayat tabungan Passbook.

---

## MATRIKS VERIFIKASI MUTU PROTOKOL

| Komponen / Pipeline | Status Verifikasi | Catatan Mutu & Invarian |
| --- | --- | --- |
| **Rust Unit & Integration Tests** | **129 LULUS** | `cargo test --workspace --all-targets` |
| **Go P2P Network Suite** | **LULUS** | `go test -v -race ./...` (0 race conditions, 0 deadlocks) |
| **Linting & Code Style** | **LULUS** | `cargo fmt --check` & `cargo clippy -D warnings` |
| **Float Arithmetic Ban** | **TERPENUHI** | 0 operasi float di seluruh layer konsensus, skrip, dan mempool |
| **Live P2P Sync Harness** | **LULUS** | `./scripts/testnet_2node.sh` |
| **Chaos Reorg Harness** | **LULUS** | `./scripts/testnet_fork_reorg.sh` |
| **HTTP Explorer Endpoints** | **LULUS** | Validasi live JSON `/api/v1/mempool` dan UI inspection |
