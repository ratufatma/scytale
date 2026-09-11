# Scytale Protocol — Technical Specification & Architecture Record: Task 26
## Consensus Script Verification & Sighash Digest Integration

```text
Document ID   : SPEC-TASK-26
Task ID       : 26
Task Name     : Consensus Script Verification & Sighash Digest Integration
Phase         : Phase 3 — Protocol Engine & Smart Scripting
Target Crates : crates/scytale-core, crates/scytale-storage, apps/scytale-node
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Domain-Separated Sighash V1, Replay Attack Immunity, OP_RETURN UTXO Set Exclusion
Quality Gates : 100% Rust Tests PASS | Consensus Script Integration Tests PASS
```

---

## 1. Problem Statement

Mesin virtual skrip harus diintegrasikan langsung ke dalam alur validasi konsensus utama (*consensus pipeline*) sehingga setiap transaksi yang masuk ke mempool atau blok kanonikal diverifikasi secara kriptografis terhadap kondisi penguncian UTXO induk yang dibelanjakan.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Algoritma Digest Sighash V1 (`crates/scytale-core/src/transaction.rs`)
Digest tanda tangan dihitung menggunakan fungsi hash BLAKE3 dengan pemisahan domain:
$$\text{Sighash} = \text{BLAKE3}\Big(\text{"SCYTALE\_SIGHASH\_V1"} \,\Vert\, \text{inputs} \,\Vert\, \text{outputs} \,\Vert\, \text{input\_index} \,\Vert\, \text{prev\_locking\_script}\Big)$$
- Mengikat seluruh referensi OutPoint, jumlah output, skrip penguncian, indeks input yang dibelanjakan, dan kondisi penguncian sebelumnya untuk mencegah serangan pemutaran ulang (*replay attacks*).

### 2.2 Verifikasi Skrip pada Node Konsensus (`apps/scytale-node/src/node.rs`)
- `Node::verify_transaction_scripts`: Mengambil UTXO yang direferensikan, menghitung `sighash`, dan mengeksekusi `ScriptEngine::execute(&input.authorization, &utxo.locking_condition, &ctx)`.
- Ditegakkan secara fail-closed pada admisi mempool (`submit_transaction`) dan validasi blok kanonikal (`submit_external_block`).

### 2.3 Aturan Konsensus Khusus `OP_RETURN`
- Nilai transaksi `output.value == 0` diizinkan secara eksklusif hanya untuk skrip yang diawali oleh opcode `OP_RETURN` (`0x6a`).
- Transaksi `OP_RETURN` tetap dicatat dalam rantai blok dan diindeks pada tabel `tables::TRANSACTIONS`, namun **dikecualikan dari tabel `tables::UTXOS`** dan set memori `UtxoSet` untuk mencegah akumulasi beban state yang tidak dapat dibelanjakan (*unspendable state bloat*).
