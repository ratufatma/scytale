# Quickstart: Menjalankan Node & Solo Mining

Panduan ini menuntun Anda mengompilasi biner resmi, membuat dompet baru, dan menjalankan node penambang (solo miner) yang terhubung langsung ke Testnet Scytale dalam 5 menit.

---

## 1. Prasyarat Sistem

- **Sistem Operasi**: Linux (Ubuntu 22.04/24.04, Debian 12) atau macOS.
- **Rust**: Versi `1.85+` stable.
- **Paket Pendukung**: `build-essential`, `pkg-config`, `libssl-dev`, `git`, `curl`.

```bash
# Instal dependensi (Ubuntu/Debian)
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev git curl
```

---

## 2. Unduh & Kompilasi Biner

```bash
git clone https://github.com/ratufatma/scytale.git
cd scytale

# Kompilasi daemon node dan CLI wallet
cargo build --release -p scytale-node -p scytale-cli
```

Biner yang dihasilkan akan tersedia di `target/release/scytale-node` dan `target/release/scytale-cli`.

---

## 3. Buat Alamat Dompet untuk Hadiah Tambang

Jalankan perintah berikut untuk menghasilkan pasangan kunci Ed25519 dan alamat Bech32:

```bash
./target/release/scytale-cli wallet new
```

Simpan mnemonic dan alamat Bech32 yang dihasilkan (berawalan `scy1...`).  
*Contoh*: `scy1nw7vhxmxyz2jlw89vz88tdv938692xk968uxn89787fa4w207s8sddvv3q`.

---

## 4. Jalankan Node & Mulai Menambang (Solo Miner)

Jalankan node dengan flag `--mine` dan cantumkan alamat payout Anda:

```bash
./target/release/scytale-node start \
  --mine \
  --payout-address <ALAMAT_BECH32_ANDA>
```

Node akan otomatis:
1. Menghubungi bootnode kanonikal (`seed.myratu.com:9000`).
2. Mengunduh dan menyinkronkan seluruh blok yang sudah ada.
3. Memulai prosesor Proof-of-Work BLAKE3 paralel memanfaatkan seluruh core CPU yang tersedia.
4. Memublikasikan blok baru ke jaringan via Gossipsub begitu solusi PoW ditemukan.

---

## 5. Cek Status Node

Buka terminal baru dan kueri endpoint HTTP lokal node Anda:

```bash
# Cek status runtime & tinggi rantai
curl -s http://127.0.0.1:8332/api/v1/status

# Cek informasi blok terbaru (tip)
curl -s http://127.0.0.1:8332/api/v1/blocks/tip
```
