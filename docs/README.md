# Scytale Official Documentation (Public Testnet Release)

Selamat datang di repositori dokumentasi resmi blockchain **Scytale** — protokol Layer 1 terdesentralisasi berbasis Proof-of-Work BLAKE3, model transaksi Extended UTXO (eUTXO), dan tumpukan jaringan P2P native Rust `libp2p`.

---

## Peta Dokumentasi

### 1. Protokol & Konsensus (`docs/protocol/`)
- [**Canonical Genesis**](protocol/genesis.md): Identitas blok genesis, hash kanonikal `4033f099...`, alokasi token awal 33M SCY (Founder 30% & Dev 20%).
- [**Tokenomics & Moneter**](protocol/tokenomics.md): Batas suplai maksimum 66M SCY, satuan Quanta ($10^8$), jadwal halving 660k blok, dan formula subsidi era 0 (25 SCY).
- [**Mesin Konsensus**](protocol/consensus.md): Spesifikasi PoW BLAKE3, target kesulitan `0x1d00ffff`, MTP-11, seleksi rantai terberat (*accumulated work*), dan batas reorg.
- [**Model eUTXO**](protocol/eutxo-model.md): Format transaksi, skrip penguncian P2PKH Ed25519, OP_RETURN payload, dan aturan konservasi massa.

### 2. Jaringan & Topologi (`docs/network/`)
- [**Bootnodes & Discovery**](network/bootnodes.md): Multiaddr bootnode resmi (`seed.myratu.com:9000` & IP 116.212.72.89), Kademlia DHT, serta deprecation notice DNS seeder port 53.
- [**Arsitektur P2P**](network/p2p-architecture.md): Protokol libp2p v0.56, enkripsi Noise XX, multiplexing Yamux, topik Gossipsub mesh, dan sinkronisasi rantai terarah.

### 3. Panduan Operasional Node (`docs/node-ops/`)
- [**Quickstart 5 Menit**](node-ops/quickstart.md): Panduan cepat kompilasi, membuat dompet, dan menjalankan node penambang publik.
- [**Panduan Penambangan (Mining Guide)**](node-ops/mining-guide.md): Operasi solo mining, pemanfaatan core CPU paralel dengan Rayon, dan log progres nonce.
- [**Deployment Produksi systemd VPS**](node-ops/systemd-vps.md): Konfigurasi layanan background Linux, isolasi hak akses pengguna, dan sandboxing keamanan.

### 4. Referensi Antarmuka API (`docs/api/`)
- [**HTTP RPC v1 Reference**](api/rpc-v1.md): Dokumentasi skema JSON endpoint `GET /api/v1/status`, `GET /api/v1/blocks/tip`, `POST /api/v1/tx`, format galat, dan rate limiting.

### 5. Perkakas Pengguna (`docs/tools/`)
- [**CLI Wallet Tool (`scytale-cli`)**](tools/cli.md): Manajemen kunci Ed25519, pembuatan alamat Bech32, transfer koin, dan kueri status.
- [**Web Explorer & Indexer**](tools/explorer.md): Setup penjelajah blok visual, worker metadata ingest, dan panduan reverse proxy HTTPS di [explorer.myratu.com](https://explorer.myratu.com).

---

## Tautan Cepat Komunitas & Layanan Publik

- **Web Explorer Resmi**: [https://explorer.myratu.com](https://explorer.myratu.com)
- **Kanonikal Bootnode**: `/dns4/seed.myratu.com/tcp/9000/p2p/12D3KooWMNVcoP79QMoLfg8NKeFyCRfLBq6HngTQpmnkbD9oBWMc`
- **Repositori Sumber**: [https://github.com/ratufatma/scytale](https://github.com/ratufatma/scytale)
