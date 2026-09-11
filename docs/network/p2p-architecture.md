# Scytale P2P Networking Architecture

Dokumen ini mendokumentasikan tumpukan protokol jaringan P2P (Peer-to-Peer) berbasis Rust `libp2p` v0.56 pada Scytale.

---

## 1. Lapisan Transport & Keamanan

- **Transport**: TCP dengan integrasi `tokio` asynchronous runtime.
- **Resolusi Domain**: `libp2p::dns::tokio::Transport` untuk memetakan multiaddr DNS (`/dns4/...`) menjadi alamat IP target.
- **Enkripsi Saluran**: **Noise Protocol Framework** (pola handshake `Noise XX`) menggunakan kunci identitas Ed25519.
- **Multiplexing**: **Yamux**, memungkinkan banyak sub-stream logis (sinkronisasi, gossipsub, ping, Kademlia) berjalan di atas satu koneksi TCP tunggal.

---

## 2. Protokol Sub-sistem P2P

```text
libp2p Swarm
├── Identity: Ed25519 PeerId
├── Transport: TCP + DNS (Port 9000/9005)
├── Encryption: Noise XX
├── Multiplexing: Yamux
└── Behaviours:
    ├── Gossipsub: Propagasi blok & transaksi real-time
    ├── Kademlia DHT: Penemuan rekan terdesentralisasi
    ├── Request-Response: Sinkronisasi blok terarah (Direct Sync)
    ├── Ping: Pemantauan liveness & RTT latency
    └── Identify: Pertukaran metadata versi protokol
```

---

## 3. Topik Gossipsub (`libp2p-gossipsub v1.1`)

Propagasi data di Scytale mengandalkan mesh terdesentralisasi:
- `/scytale/blocks/1.0.0`: Menerbitkan blok baru yang dicetak oleh penambang ke seluruh validator jaringan (latensi propagasi < 10 milidetik).
- `/scytale/transactions/1.0.0`: Menyebarkan transaksi mempool baru antar node.

Semua pesan yang disebarkan diverifikasi integritas hash dan tanda tangannya sebelum diteruskan ulang (*forwarding filter*), mencegah serangan banjir spam (*DoS amplification*).

---

## 4. Sinkronisasi Blok (Direct Block Sync)

Ketika node mendeteksi dirinya tertinggal dari ujung rantai rekan (*tip height*):
1. Node mengirimkan request `GetHeaders { locator, stop_hash }` ke rekan terdekat.
2. Rekan membalas dengan daftar header blok yang hilang.
3. Node meminta batch blok penuh via `GetBlocks { hashes }`.
4. Blok divalidasi secara berurutan dan di-commit ke basis data redb lokal hingga mencapai konsensus rantai terberat.
