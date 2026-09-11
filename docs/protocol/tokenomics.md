# Scytale Monetary Policy & Tokenomics

Dokumen ini mendefinisikan kebijakan moneter, batas suplai maksimum, dan jadwal emisi/halving dari blockchain Scytale.

---

## 1. Parameter Dasar Moneter

| Parameter | Nilai |
| :--- | :--- |
| **Ticker Simbol** | `SCY` |
| **Satuan Terkecil (Quanta)** | `1 SCY = 100,000,000 Quanta` (`10^8`) |
| **Total Suplai Maksimum** | **`66,000,000.00000000 SCY`** (`6.600.000.000.000.000` Quanta) |
| **Target Block Time** | 60 detik (1 menit) |
| **Blok per Hari** | ~1.440 blok |
| **Blok per Tahun** | ~525.600 blok |

---

## 2. Distribusi Suplai

```text
Total Max Supply: 66,000,000 SCY
┌─────────────────────────────────────────────────────────┐
│  Genesis Allocation: 33,000,000 SCY (50%)              │
│  ├─ Founder Allocation (30%): 19,800,000 SCY            │
│  └─ Core Dev / Treasury (20%): 13,200,000 SCY           │
├─────────────────────────────────────────────────────────┤
│  Proof-of-Work Mining Reserve: 33,000,000 SCY (50%)     │
│  └─ Emisi bertahap melalui subsidi penambangan          │
└─────────────────────────────────────────────────────────┘
```

---

## 3. Jadwal Emisi & Halving

Subsidi penambangan berkurang setengahnya (*halving*) setiap **660.000 blok** (~1,25 tahun per era):

- **Interval Halving**: `660,000` blok
- **Subsidi Awal (Era 0)**: `25.00000000 SCY` (`2,500,000,000` Quanta) per blok
- **Formula Subsidi**:
  $$\text{Subsidy}(h) = \left\lfloor \frac{25 \times 10^8}{2^{\lfloor h / 660,000 \rfloor}} \right\rfloor \text{ Quanta}$$

### Tabel Rincian Era Halving:

| Era | Rentang Blok | Subsidi per Blok | Total Emisi Era (SCY) | Suplai Kumulatif PoW |
| :---: | :---: | :---: | :---: | :---: |
| **0** | `1` – `660,000` | 25.00000000 SCY | 16,500,000 SCY | 16,500,000 SCY |
| **1** | `660,001` – `1,320,000` | 12.50000000 SCY | 8,250,000 SCY | 24,750,000 SCY |
| **2** | `1,320,001` – `1,980,000` | 6.25000000 SCY | 4,125,000 SCY | 28,875,000 SCY |
| **3** | `1,980,001` – `2,640,000` | 3.12500000 SCY | 2,062,500 SCY | 30,937,500 SCY |
| **4** | `2,640,001` – `3,300,000` | 1.56250000 SCY | 1,031,250 SCY | 31,968,750 SCY |
| ... | ... | ... | ... | ... |
| **33** | Blok $\ge 21,780,000$ | 0 Quanta | 0 SCY | **33,000,000 SCY** |

Setelah seluruh era penambangan selesai, insentif validator sepenuhnya bersumber dari biaya transaksi (*transaction fees*).

---

## 4. Penyesuaian Kesulitan (Difficulty Adjustment Algorithm)
- **Interval Retarget**: Setiap **1.440 blok** (~24 jam).
- **Mekanisme**: Membandingkan waktu aktual yang dibutuhkan untuk menambang 1.440 blok terakhir dengan target ideal 86.400 detik.
- **Clamping Factor**: Faktor perubahan target dibatasi dalam rentang $[0.25, 4.0]$ untuk melindungi kestabilan jaringan dari lonjakan atau penurunan drastis hashrate.
