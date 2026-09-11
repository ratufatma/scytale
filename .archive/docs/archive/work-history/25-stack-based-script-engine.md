# Scytale Protocol — Technical Specification & Architecture Record: Task 25
## Minimalist Stack-Based Script Engine (`crates/scytale-script`)

```text
Document ID   : SPEC-TASK-25
Task ID       : 25
Task Name     : Minimalist Stack-Based Script Engine (crates/scytale-script)
Phase         : Phase 3 — Protocol Engine & Smart Scripting
Target Crates : crates/scytale-script
Status        : COMPLETED / PRODUCTION-READY
Invariants    : Non-Turing-Complete, Pure Integer Math (Zero Float), Bounded Execution Budget, Sandboxed VM
Quality Gates : 100% Rust Tests PASS | 0 Warnings Clippy | Stack Depth & Budget Limits Enforced
```

---

## 1. Problem Statement

Untuk berevolusi dari transfer koin sederhana berbasis kecocokan byte mentah menuju transaksi cerdas (*programmable smart transactions*), Scytale memerlukan mesin virtual (VM) berbasis tumpukan (*stack-based*) yang deterministik, terisolasi (*sandboxed*), dan non-Turing-complete.

---

## 2. Arsitektur & Spesifikasi Teknis

### 2.1 Arsitektur Crate (`crates/scytale-script`)
- `OpCode`: Enumerasi byte-code instruksi mesin virtual.
- `ScriptStack`: Tumpukan LIFO berelemen vektor byte (`Vec<u8>`) dengan operasi aritmatika integer murni (`i64`) dan proteksi pembagian nol.
- `ScriptEngine`: Mesin evaluasi eksekusi skrip.
- `ScriptBuilder`: Pembangun bytecode skrip dengan antarmuka fluent.

### 2.2 Kategori Instruksi
- **Manipulasi Stack:** `OP_DUP`, `OP_DROP`, `OP_SWAP`, `OP_ROT`, `OP_2DUP`, `OP_2DROP`.
- **Aritmatika & Perbandingan:** `OP_ADD`, `OP_SUB`, `OP_MUL`, `OP_DIV`, `OP_MOD`, `OP_EQUAL`, `OP_EQUALVERIFY`, `OP_NUMEQUAL`, `OP_LESSTHAN`, `OP_GREATERTHAN`.
- **Kriptografi:** `OP_BLAKE3`, `OP_CHECKSIG`, `OP_CHECKSIGVERIFY` (verifikasi tanda tangan Ed25519 terhadap sighash 32-byte).
- **Aliran Kontrol & Timelock:** `OP_CHECKLOCKTIMEVERIFY`, `OP_IF`, `OP_ELSE`, `OP_ENDIF`, `OP_RETURN`.

### 2.3 Batasan Pasir (*Sandbox Boundaries*)
- Anggaran eksekusi opcode: Maksimum 256 instruksi per transaksi.
- Kedalaman stack: Maksimum 1.024 elemen.
- Ukuran maksimal elemen stack: 520 byte.

### 2.4 Kompatibilitas Mundur (*Backward Compatibility*)
- Tetap mendukung skrip legacy: Jika `locking_script.len() <= 32 && unlocking_script == locking_script`, transaksi langsung dievaluasi valid tanpa eksekusi VM lanjutan.
