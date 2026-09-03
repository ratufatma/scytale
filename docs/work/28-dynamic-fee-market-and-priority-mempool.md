# Scytale Protocol — Technical Specification & Architecture Record: Task 28
## Dynamic Fee Market & Priority Mempool Eviction

```text
Document ID   : SPEC-TASK-28
Task ID       : 28
Task Name     : Dynamic Fee Market & Priority Mempool Eviction
Phase         : Phase 3 — Protocol Engine & Smart Scripting
Target Crates : crates/scytale-core, crates/scytale-mempool, apps/scytale-node
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Zero Float Arithmetic, Deterministic Priority Ordering, DoS-Resistant Bounds
Quality Gates : 100% Rust Tests PASS | Fee Market Integration Tests PASS
```

---

## 1. Problem Statement

Sebelum Task 28, transaksi memiliki ukuran bervariasi karena adanya skrip P2PKH dan payload data `OP_RETURN`, namun mempool masih berupa antrean FIFO sederhana tanpa batas kuota memori yang kaku. Hal ini membuka celah DoS / spam dust transaksi dan tidak memberikan mekanisme ekonomi lelang ruang blok bagi penambang.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Densitas Biaya Berbasis Integer Murni (Zero-Float Fee Density)
$$\text{Fee Rate (milli-quanta/byte)} = \frac{\text{Fee (quanta)} \times 1000}{\text{Serialized Size (bytes)}}$$
- Menggunakan pembagian integer `u64` murni tanpa float untuk kepatuhan `#![deny(clippy::float_arithmetic)]`.

### 2.2 Batas Kapasitas & Minimum Relay Fee
- `MAX_MEMPOOL_COUNT` = 5.000 transaksi.
- `MAX_MEMPOOL_BYTES` = 5.000.000 byte (~5 MB).
- `MIN_RELAY_FEE_RATE` = 1.000 milli-quanta/byte (setara 1 quantum/byte). Transaksi di bawah ambang ini ditolak saat admisi.

### 2.3 Deterministik Priority Ordering & Eviction Policy
- Indeks prioritas disusun berdasarkan kunci komposit:
  $$\text{Priority} = (\text{fee\_rate } \mathbf{DESC}, \; \text{added\_time } \mathbf{ASC}, \; \text{txid } \mathbf{ASC})$$
- Jika mempool penuh dan transaksi baru memiliki `fee_rate` lebih tinggi dari transaksi terendah, transaksi terendah digusur (*evicted*).

### 2.4 Akumulasi Biaya Penambang (Miner Fee Accrual)
- Saat merakit kandidat blok, penambang mengambil transaksi berdensitas tertinggi.
- Nilai keluaran *coinbase* diatur tepat:
  $$\text{Coinbase Value} = \text{Current Subsidy} + \text{Total Fees}$$
