# Task 06 — Hashing & Canonical Serialization

This document is the permanent **Task Execution Runbook** for Task 06: Hashing and Canonical Serialization. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's deterministic binary codecs, BLAKE3 hash digest routines, and transaction identifier (`TxID`) derivation.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 06
Task Name   : Hashing / Serialization
Phase       : Ledger
Level       : MEDIUM
Status      : PLANNED
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Integer quanta values.
- **Task 02 — Genesis Allocation:** Genesis OutPoint formats.
- **Task 03 — Transaction:** `Transaction`, `TxIn`, `TxOut`, and `OutPoint` data structures.
- **Task 04 — UTXO:** Output references and OutPoint primary keys.
- **Task 05 — Authorization:** Context preimage and unlocking proof buffers.

### Core Reference Specifications:
- [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md)
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective

> **Task Goal:** *Implement pure, deterministic binary serialization codecs and BLAKE3 cryptographic hashing primitives in `scytale-core`, establishing the immutable mathematical pipeline that converts logical protocol data structures into canonical bytes and unforgeable 32-byte identifiers.*

### Core Pipeline Invariant:
```text
Logical Protocol Object (Transaction)
                  ↓
   [ Canonical Binary Encoder ]
                  ↓
        Canonical Byte Vector
                  ↓
    [ BLAKE3 Cryptographic Hash ]
                  ↓
       32-Byte Immutable TxID
```

---

## 3. Locked Architectural Decisions

The following parameters are formally locked and must **NOT** be altered:

```text
Primary Hashing Primitive : BLAKE3
Cryptographic Digest Size : 32 bytes (256 bits)

Transaction Identifier (TxID) Derivation:
TxID = BLAKE3(Serialize_canonical(Transaction))

OutPoint Primary Key:
OutPoint { txid: 32-byte Hash, index: u32 }
```

---

## 4. Core Serialization Principles

1. **Deterministic Canonical Mapping:** Every valid logical data structure maps to exactly **one** valid byte sequence.
   $$\text{Semantic Equality} \implies \text{Canonical Byte Equality} \implies \text{Digest Equality}$$
2. **Platform & Endianness Invariance:** Codecs must encode integers using fixed standard endianness (e.g. little-endian integer byte order) to guarantee cross-architecture parity.
3. **No Floating-Point Serialization:** Monetary amounts are strictly integer `quanta` (`u64`).
4. **No Environmental Contamination:** Timestamps, memory pointers, map hash-seeds, or debug strings must never pollute canonical protocol bytes.
5. **Fail-Closed Deserialization:** Any malformed, truncated, or trailing unexpected bytes must trigger an immediate deserialization error.

---

## 5. Scope & Non-Goals

### In Scope:
- Core `Hash` domain wrapper (32 bytes) around the `blake3` crate.
- Canonical binary encoding and decoding traits for `Transaction`, `TxIn`, `TxOut`, and `OutPoint`.
- Deterministic `TxID` derivation pipeline.
- Deserialization validation, bounds checking, and integer overflow protection.
- Round-trip serialization and hash regression test suites with deterministic test vectors.

### Out of Scope / Non-Goals:
- Finalizing `BlockID` derivation (deferred to Task 07 / Block).
- Designing P2P wire framing protocols (deferred to Task 12 / P2P).
- Designing disk storage table keys (deferred to Task 14 / Storage).
- Implementing human-readable JSON APIs, RPCs, or Passbook presentation encoders.

---

## 6. Work Items

### W1 — Inspect Existing Primitives & Dependencies
- Inspect `crates/scytale-core/src/lib.rs` and `Cargo.toml`.
- Re-use the existing `blake3` dependency and `Hash` struct without introducing redundant types.

### W2 — Evaluate Canonical Serialization Codec
- Implement a zero-ambiguity binary serialization codec.
- If the binary format is insufficiently specified to implement without guesswork:
  - Mark task as `Status: BLOCKED` (Reason: *Canonical serialization specification awaiting format lock*).

### W3 — Canonical Transaction Byte Layout
- Standardize the exact field encoding sequence:
  ```text
  Transaction Payload:
  ├── version         : u32 (4 bytes, fixed endianness)
  ├── input_count     : compact/fixed integer length prefix
  ├── inputs          : vector of canonical TxIn
  ├── output_count    : compact/fixed integer length prefix
  └── outputs         : vector of canonical TxOut
  ```

### W4 — Deterministic Integer & Length Encoding
- Encode integer `quanta`, indices, and versions using deterministic integer byte representations.
- Enforce strict parsing rules preventing non-canonical integer encodings.

### W5 — Raw Byte Vector Serialization
- Encode `authorization` proofs and `locking_condition` scripts as raw length-prefixed byte vectors (never as hex or ASCII strings).

### W6 — `TxID` Derivation Implementation
- Provide a clean, idiomatic helper method on `Transaction`:
  ```rust
  impl Transaction {
      pub fn txid(&self) -> Hash {
          let canonical_bytes = self.to_canonical_bytes();
          Hash::hash(&canonical_bytes)
      }
  }
  ```

### W7 — Domain Separation (`TBD`)
- Maintain isolation between transaction hashing and block header hashing.
- Note: `Domain Separation Prefix: TBD` (Do not invent custom prefixes without specification approval).

---

## 7. Round-Trip & Determinism Invariants

The serialization implementation must satisfy two mandatory mathematical properties:

```text
[ Property 1: Exact Round-Trip Reversibility ]
Object ──(Serialize)──> Bytes ──(Deserialize)──> Reconstructed Object == Original Object

[ Property 2: Canonical Determinism ]
Reconstructed Object ──(Serialize)──> Re-encoded Bytes == Original Bytes
```

- If deserializing and re-serializing generates a differing byte vector, the codec is **non-canonical** and must be rejected.

---

## 8. Error Model

Serialization and parsing routines must return strongly-typed domain errors:

```rust
pub enum SerializationError {
    UnexpectedEof,
    InvalidLengthPrefix { length: usize, max: usize },
    InvalidIntegerEncoding,
    TrailingBytesDetected(usize),
    UnsupportedVersion(u32),
    MalformedByteVector,
}
```

---

## 9. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 06 must satisfy the following test suites:

### Unit Tests:
- `test_blake3_digest_length_32`: Assert BLAKE3 digest is strictly 32 bytes.
- `test_transaction_roundtrip`: Assert $\text{Deserialize}(\text{Serialize}(\text{tx})) == \text{tx}$.
- `test_canonical_byte_determinism`: Assert identical transactions produce bit-identical byte buffers.
- `test_txid_reproducibility`: Assert identical transactions generate identical `TxID` hashes across multiple iterations.

### Negative & Malformed Input Tests:
- `test_reject_truncated_bytes`: Assert `UnexpectedEof` on truncated payloads.
- `test_reject_trailing_bytes`: Assert error when unexpected data follows a valid payload.
- `test_reject_excessive_length_prefix`: Assert rejection on out-of-bounds length declarations.

### Deterministic Test Vectors (Regression Fixtures):
- Maintain fixed, hardcoded hex test fixtures in unit tests asserting exact expected `TxID` hashes for canonical sample transactions.

---

## 10. Acceptance Criteria Checklist

Task 06 can only be marked as **VERIFIED** when:

- [ ] BLAKE3 is integrated as the primary hashing function with 32-byte digest outputs.
- [ ] Canonical binary serialization is implemented for `Transaction`, `TxIn`, `TxOut`, and `OutPoint`.
- [ ] Codec produces 100% deterministic byte output across all environments.
- [ ] `TxID` derivation is bound strictly to canonical transaction bytes.
- [ ] Round-trip reversibility ($\text{De}(\text{Ser}(x)) == x$) is proven.
- [ ] Strict rejection of malformed, truncated, and trailing bytes is verified.
- [ ] OutPoint stability ($\text{TxID} + \text{index}$) is preserved.
- [ ] No floating-point or non-canonical text representations are used.
- [ ] 100% of unit, negative, and regression vector tests pass.
- [ ] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 11. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Codec and hashing implementation underway.
     │
     ├── If serialization schema requires protocol clarification ──> [ BLOCKED ]
     │                                                                       │
     │ <─────────────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All codec and hash invariant tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 07 (Block).
```

- **Current Status:** **`PLANNED`**

---

## 12. Dependency for Downstream Tasks

- **Task 07 (Block Model):** Utilizes `TxID`, canonical transaction byte vectors, and BLAKE3 digests to construct block transaction vectors and transaction commitments.

---

## 13. Agent Operating Rules

1. Treat `docs/work/06-hashing-serialization.md` as the authoritative work runbook.
2. Never invent ad-hoc serialization formats; ensure complete determinism.
3. If canonical encoding formats remain ambiguous in specifications, mark status as `BLOCKED`.
4. Keep the codec focused on protocol objects (avoid storage or P2P coupling).
5. Adhere strictly to the definition of done and quality gates.

---

## 14. Cross-Specification References

- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)**: Master hashing and serialization specification.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction specification.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: OutPoint definitions.
- **[`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md)**: Unlocking proof serialization.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Parameter registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
