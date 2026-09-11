# Scytale CLI Reference (`scytale-cli`)

Perkakas antarmuka baris perintah (`scytale-cli`) digunakan untuk pengelolaan dompet HD, pembuatan kunci Ed25519, penandatanganan transaksi, dan kueri interaktif ke node.

---

## 1. Perintah Dompet (Wallet Management)

### A. Membuat Dompet Baru
```bash
./target/release/scytale-cli wallet new
```
- Menghasilkan 12 atau 24 kata BIP-39 seed phrase.
- Menghasilkan pasangan kunci Ed25519 dan alamat publik Bech32 (`scy1...`).

### B. Memulihkan Dompet dari Mnemonic
```bash
./target/release/scytale-cli wallet restore --mnemonic "kata1 kata2 ... kata12"
```

### C. Cek Saldo Dompet
```bash
./target/release/scytale-cli wallet balance --address <ALAMAT_BECH32>
```

### D. Mengirim SCY (Transfer Transaksi)
```bash
./target/release/scytale-cli wallet send \
  --from-key <PATH_PRIVATE_KEY_ATAU_MNEMONIC> \
  --to <ALAMAT_PENERIMA_SCY1> \
  --amount <JUMLAH_SCY> \
  --fee <FEE_QUANTA>
```

---

## 2. Kueri Status Node via CLI

`scytale-cli` dapat mengueri daemon `scytale-node` baik melalui Unix Domain Socket lokal (`/run/scytale/node.sock`) maupun via gateway HTTP:

```bash
# Cek info status node
./target/release/scytale-cli status

# Cek tip blok kanonikal
./target/release/scytale-cli tip

# Ambil informasi detail blok
./target/release/scytale-cli block --height 1
```
