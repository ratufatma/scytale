# Panduan Penambangan (Mining Guide)

Dokumen ini menjelaskan operasi penambangan solo (solo mining) di jaringan Scytale, tuning performa CPU, dan parameter biner penambang.

---

## 1. Arsitektur Proof-of-Work

Scytale menggunakan fungsi hash **BLAKE3**:
- Hashing sangat efisien di CPU arsitektur modern (x86-64 dengan instruksi AVX2/AVX-512 dan ARM64 dengan NEON).
- Algoritma pencarian nonce diimplementasikan menggunakan parallel iterator `rayon`, yang secara otomatis membagi rentang pencarian nonce ke semua thread CPU fisik/logis.

---

## 2. Opsi Command Line Penambang

Saat menjalankan `scytale-node start`:

| Flag / Opsi | Deskripsi | Contoh |
| :--- | :--- | :--- |
| `--mine` | Mengaktifkan thread penambang internal node. | `--mine` |
| `--payout-address <ADDR>` | Alamat Bech32 (`scy1...`) penerima subsidi blok. | `--payout-address scy1...` |
| `--miner-payout <HEX>` | Locking script hex mentah (alternatif tingkat rendah). | `--miner-payout 73a020...88ac` |
| `--data-dir <DIR>` | Direktori penyimpanan basis data rantai redb. | `--data-dir ~/.scytale/data` |
| `--p2p-port <PORT>` | Port listen P2P TCP (default: `9000`). | `--p2p-port 9005` |
| `--http-bind <IP:PORT>` | Alamat bind gateway HTTP (default: `127.0.0.1:8332`). | `--http-bind 127.0.0.1:8336` |

---

## 3. Log Siklus Penambangan

Saat menambang, daemon akan mencatat progres pencarian nonce secara berkala:

```text
2026-09-11T13:25:45Z INFO scytale_node::node: mining cycle progress height=1 searched=4560000000
2026-09-11T13:26:02Z INFO scytale_node::node: found valid PoW solution, attempting process_block height=1 nonce=4820336845 hash=0000000074857748e2050bff75595a287e1e7b507d2a4dcfabc9db4ee70ef552
2026-09-11T13:26:02Z INFO scytale_node::node: mined new block successfully committed height=1 hash=0000000074857748e2050bff75595a287e1e7b507d2a4dcfabc9db4ee70ef552 tx_count=1
```

- **`searched`**: Jumlah nonce yang telah dievaluasi dalam siklus saat ini.
- **`found valid PoW solution`**: Nonce memenuhi target kesulitan konsensus.
- **`mined new block successfully committed`**: Blok diverifikasi lolos validasi konsensus lokal dan disebarkan ke jaringan melalui Gossipsub.

---

## 4. Hadiah & Waktu Pematangan (Coinbase Maturity)

- Hadiah penambangan di era 0 adalah **25 SCY** per blok.
- Saldo coinbase yang baru dicetak memerlukan **100 konfirmasi blok** (*coinbase maturity delay*) sebelum dapat dibelanjakan atau ditransfer ke dompet lain.
