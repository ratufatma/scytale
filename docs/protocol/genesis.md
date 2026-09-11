# Canonical Genesis Specification

Dokumen ini mendefinisikan spesifikasi kanonikal blok Genesis (Tinggi 0) untuk jaringan Scytale Layer 1.

---

## 1. Identifikasi Genesis

| Parameter | Nilai Kanonikal |
| :--- | :--- |
| **Genesis Block Hash** | `4033f099ae89051a629c871e9af28a215898ff345505ffdbbce65c27a29585c9` |
| **Height** | `0` |
| **Previous Block Hash** | `0x0000000000000000000000000000000000000000000000000000000000000000` |
| **Difficulty Target** | `0x1d00ffff` (Initial Difficulty Baseline) |
| **Transaction Commitment** | `0x72aa95d0b010318d5ad4418ae5a60eee7381e463920f20128d606c714daa4ca9` |
| **Timestamp** | `0` (Kanonikal epoch awal) |
| **Nonce** | `0` |
| **Network Protocol Magic** | `0x53435901` (`SCY1`) |

---

## 2. Alokasi Token Awal (33,000,000 SCY)

Blok Genesis mencetak tepat **33.000.000 SCY** (`3.300.000.000.000.000` Quanta) melalui satu transaksi coinbase kanonikal:
- **Coinbase TxID**: `0xf8c04455dd8982944f59cf017ac701dbb12c42aa2b022b353886d5382816fc21`

Alokasi dibagi menjadi dua output P2PKH:

### A. Founder Allocation (30% / 19,800,000 SCY)
- **Nominal**: `19,800,000.00000000 SCY` (`1,980,000,000,000,000` Quanta)
- **Locking Script (Hex)**:
  ```text
  73a0209bbccb9b6620952fb8e5608e75b58589f4551ac5d1f8699cbe3f93dab94ff40f88ac
  ```
- **Alamat Kanonikal Bech32**:
  ```text
  scy1nw7vhxmxyz2jlw89vz88tdv938692xk968uxn89787fa4w207s8sddvv3q
  ```

### B. Core Development & Treasury (20% / 13,200,000 SCY)
- **Nominal**: `13,200,000.00000000 SCY` (`1,320,000,000,000,000` Quanta)
- **Locking Script (Hex)**:
  ```text
  73a02005277dd5198ed5b20f543520793539812865edf67b5b6da81cec743769427e8588ac
  ```
- **Alamat Kanonikal Bech32**:
  ```text
  scy1q5nhm4ge3m2myr65x5s8jdfesy5xtm0k0ddkm2qua36rw62z06zswrq8e0
  ```

---

## 3. Alokasi Proof-of-Work (50% / 33,000,000 SCY)

Sisa **33.000.000 SCY** (50% dari total suplai maksimum 66.000.000 SCY) dialokasikan murni untuk hadiah penambangan publik (*block reward subsidy*) yang dicetak melalui mekanisme konsensus Proof-of-Work mulai Blok 1.
