# Scytale Extended UTXO (eUTXO) Architecture

Dokumen ini mendefinisikan model transaksi eUTXO (Extended Unspent Transaction Output), format transaksi, validasi input/output, dan skrip pada jaringan Scytale.

---

## 1. Satuan Nilai (Quanta)

- **1 SCY** = $10^8$ Quanta ($100,000,000$ Quanta).
- Semua kalkulasi nilai internal konsensus, biaya transaksi (*fee*), dan saldo UTXO disimpan dalam integer 64-bit (`u64`) bernilai Quanta untuk mengeliminasi potensi galat pembulatan floating point (*zero-drift integer accounting*).

---

## 2. Anatomi Transaksi

Setiap transaksi Scytale terdiri dari komponen serial kanonikal:

```text
Transaction
├── version: u32
├── inputs: Vec<TxInput>
│   ├── previous_output: OutPoint (txid: Hash, index: u32)
│   └── authorization_proof: Vec<u8> (Signature + Public Key)
├── outputs: Vec<TxOutput>
│   ├── value_quanta: u64
│   ├── locking_script: Vec<u8>
│   └── op_return_payload: Option<Vec<u8>>
└── locktime: u64
```

---

## 3. Jenis Skrip Penguncian (Locking Scripts)

Scytale mendukung dua kategori skrip output kanonikal:

### A. Pay-to-Public-Key-Hash (P2PKH)
Bentuk skrip standar untuk pembayaran ke alamat pengguna:
- **Format Skrip**:
  ```text
  OP_DUP OP_HASH256 <PubkeyHash 32-byte> OP_EQUALVERIFY OP_CHECKSIG
  ```
- **Byte Prefix Kanonikal**: `73 a0 20 <32-byte hash> 88 ac`
- **Format Alamat Bech32**: Berawalan `scy1...` dengan encoding Bech32m checksum.
- **Kriptografi Tanda Tangan**: Skema tanda tangan digital **Ed25519** 64-byte dengan public key 32-byte.

### B. Data Carrier (OP_RETURN)
Digunakan untuk menanamkan bukti keaslian (*proof-of-existence*), metadata aplikasi terdesentralisasi, atau pesan arbiter:
- Nilai `value_quanta` wajib `0`.
- Kapasitas payload maksimal: `256` byte.
- Output OP_RETURN bersifat *provably unspendable* dan tidak disimpan dalam UTXO state aktif (menghemat kapasitas memori node).

---

## 4. Validasi Aturan Konsensus State

Untuk mempertahankan invarian konservasi massa moneter:
1. **No Inflation Rule**:
   $$\sum \text{Inputs} \ge \sum \text{Outputs}$$
   Selisihnya merupakan biaya transaksi (*miner transaction fee*):
   $$\text{Fee} = \sum \text{Inputs} - \sum \text{Outputs}$$
2. **Double-Spend Prevention**: Setiap input merujuk ke OutPoint yang harus ada di UTXO set saat itu. Setelah dibelanjakan, OutPoint langsung dihapus dari set.
3. **Coinbase Maturity**: Output dari transaksi coinbase hanya dapat dibelanjakan setelah rantai tumbuh minimal **100 blok** (*maturity delay*).
