# Scytale Consensus Protocol Specification

Dokumen ini menguraikan mesin konsensus Proof-of-Work (PoW), aturan seleksi rantai terberat (*heaviest accumulated work*), validasi blok, dan pencegahan reorg pada jaringan Scytale.

---

## 1. Algoritma Proof-of-Work: BLAKE3

Scytale mengimplementasikan **BLAKE3** sebagai fungsi hash PoW inti:
- **Karakteristik**: Berkecepatan tinggi, paralel secara native (struktur tree-hashing), dan memiliki ketahanan kriptografis terhadap serangan *length-extension*.
- **Domain Separation**: Hash header blok dihitung dari representasi byte serial kanonikal:
  $$\text{Block Header} = \langle \text{version}, \text{prev\_hash}, \text{tx\_commitment}, \text{timestamp}, \text{target}, \text{nonce} \rangle$$
- **Evaluasi PoW**:
  $$\text{Hash}(\text{Header}) \le \text{Target}$$

---

## 2. Format Target Kesulitan (*Compact Target*)

Target kesulitan dikodekan dalam format floating 32-bit compact (mirip Bitcoin *nBits*):
- **Target Awal Kanonikal**: `0x1d00ffff`
  - Mantissa: `0x00ffff`
  - Eksponen: `0x1d`
  - Nilai Numerik: $0x00ffff \times 256^{(0x1d - 3)}$
- **Target Maksimum (Kesulitan Terendah)**: `0x1d00ffff`

---

## 3. Validasi Monotonik Waktu (Median Time Past - MTP)

Untuk mencegah manipulasi waktu oleh penambang jahat:
- Setiap blok baru harus memiliki `timestamp` yang lebih besar daripada **Median Time Past dari 11 blok sebelumnya** (MTP-11):
  $$\text{timestamp}_{\text{block}} > \text{Median}(\text{timestamp}_{h-1}, \dots, \text{timestamp}_{h-11})$$
- Timestamp blok juga tidak boleh lebih dari **2 jam ke masa depan** dari waktu lokal node (*network time tolerance*).

---

## 4. Aturan Seleksi Rantai (Chain Selection Rule)

Scytale mengikuti aturan **Rantai Terberat dengan Kerja Kumulatif Terbesar** (*Accumulated PoW Work*), bukan sekadar jumlah blok tertinggi (*height*):
- Setiap blok memiliki bobot kerja:
  $$\text{Work} = \left\lfloor \frac{2^{256}}{\text{Target} + 1} \right\rfloor$$
- **Canonical Chain**: Rantai dengan $\sum \text{Work}$ tertinggi.
- **Tie-Breaking DeterministiK**: Jika dua rantai kandidat memiliki total kerja identik, penentu cabang kanonikal adalah leksikografis terkecil dari `block_hash`.

---

## 5. Batas Kedalaman Reorganisasi (*Max Reorg Depth*)

Untuk melindungi finalitas transaksi dan node storage dari serangan branch panjang:
- Parameter `max_reorg_depth`: **`100` blok**.
- Setiap blok alternatif yang mencoba memotong rantai kanonikal lebih dari 100 blok ke belakang akan langsung ditolak sebagai invalid.
