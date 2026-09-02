# Task 07 — Block Model

This document is the permanent **Task Execution Runbook** for Task 07: Block Model. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's block header structures, transaction vectors, coinbase placement invariants, and stateless block validation primitives.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 07
Task Name   : Block
Phase       : Ledger / Consensus Foundation
Level       : MEDIUM → HEAVY
Status      : PLANNED
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Block subsidy constants and supply cap boundaries.
- **Task 02 — Genesis Allocation:** Block 0 Genesis bootstrap transaction definitions.
- **Task 03 — Transaction:** Core `Transaction` primitives.
- **Task 04 — UTXO:** Output OutPoint creation hooks and state transition semantics.
- **Task 05 — Authorization:** Input unlocking proof structures.
- **Task 06 — Hashing / Serialization:** 32-byte `Hash` wrappers and canonical byte codecs.

### Core Reference Specifications:
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/GENESIS-SPEC.md`](../GENESIS-SPEC.md)
- [`docs/GENESIS-ALLOCATION.md`](../GENESIS-ALLOCATION.md)
- [`docs/POW-SPEC.md`](../POW-SPEC.md)
- [`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective

> **Task Goal:** *Implement the deterministic Block and BlockHeader domain data structures in `scytale-core`, establishing the immutable container that links historical ledger states, binds committed transactions to cryptographic headers, and enforces intra-block structural validation rules.*

### Core Target Data Model:
```text
Block
├── header       : BlockHeader
└── transactions : Vector of Transaction

BlockHeader
├── version                : Protocol block version (u32)
├── previous_block_hash    : 32-byte Hash of preceding canonical block
├── transaction_commitment : 32-byte cryptographic commitment root
├── timestamp              : Unix epoch timestamp in integer seconds (u64)
├── difficulty_target      : Proof-of-Work threshold representation (u32/u64/bytes)
└── nonce                  : Proof-of-Work search counter (u64)
```

---

## 3. Core Principles & Architectural Invariants

1. **Deterministic Structure:** Every block object is uniquely and unambiguously structured with bit-level reproducibility across all node platforms.
2. **Immutable Historical Anchor:** Once accepted into canonical storage, a block is permanently immutable; modifying any header or transaction field completely changes its identity and invalidates its Proof-of-Work.
3. **Coinbase Placement Invariant:** Every valid block contains at least one transaction, and strictly **index 0** (`transactions[0]`) is reserved for the Coinbase transaction.
4. **Decoupled Domain Layer:** Task 07 establishes the *data model and stateless validation*; it does not implement PoW mining loops, dynamic retargeting math, chain reorganization algorithms, or database storage transactions.

---

## 4. Subsystem Responsibility Boundaries

To maintain strict modular isolation, Task 07 strictly avoids implementing responsibilities allocated to downstream tasks:

```text
┌─────────────────────────────────────────────────────────────────┐
│                    `scytale-core` (Task 07)                     │
│  - Block & BlockHeader struct definitions                       │
│  - Stateless structural validation (non-empty txs, header form) │
│  - Coinbase position assertions (transactions[0] == coinbase)   │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Downstream Subsystem Tasks                    │
│  - Proof-of-Work Evaluation (Hash <= Target)    ──> Task 08     │
│  - Difficulty Adjustment Calculation           ──> Task 09     │
│  - Chain Selection & Reorganization Engine     ──> Task 10     │
│  - Mempool Block Template Assembly             ──> Task 11     │
│  - Background Autonomous Mining Loop           ──> Task 12     │
│  - redb Database Persistence & Atomic Commits  ──> Task 14     │
│  - P2P Wire Framing & Message Transport        ──> Task 15     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Work Items

### W1 — Inspect Existing Primitives
- Inspect `crates/scytale-core/src/lib.rs` and workspace types.
- Re-use `Transaction`, `TxId`, `Hash`, `OutPoint`, and `Quanta` types directly from Task 03 and Task 06 without introducing duplicate definitions.

### W2 — Implement `BlockHeader` Struct
- Define `BlockHeader` in `scytale-core` containing:
  `version`, `previous_block_hash`, `transaction_commitment`, `timestamp`, `difficulty_target`, and `nonce`.
- Implement common derived traits (`Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`).

### W3 — Implement `Block` Struct
- Define `Block` in `scytale-core` bundling `header: BlockHeader` and `transactions: Vec<Transaction>`.

### W4 — Implement Coinbase Position Validation
- Enforce the structural rule:
  $$\text{assert}(\text{transactions.len()} \ge 1)$$
  $$\text{assert}(\text{transactions}[0].\text{is\_coinbase}() == \text{true})$$
  $$\text{assert}(\forall i \in [1..\text{len}-1], \text{transactions}[i].\text{is\_coinbase}() == \text{false})$$

### W5 — Previous Block Reference Integration
- Represent the linear DAG link connecting a candidate block to its parent header (`previous_block_hash: Hash`).
- For Genesis Block 0, support the protocol-defined null parent reference ($32\text{ zero bytes}$).

### W6 — Transaction Commitment Field Handling
- Embed the 32-byte `transaction_commitment` field into `BlockHeader`.
- *Note on Status:* `Transaction Commitment Tree Algorithm: TBD` (Merkle tree vs. BLAKE3 tree). If implementation requires computing the root without an agreed specification, mark status as `BLOCKED`.

### W7 — Integer Timestamp Representation
- Store timestamps strictly as explicit unsigned 64-bit integer seconds (`u64`) since Unix epoch.
- Never invoke system clock APIs implicitly during block hashing or validation.

### W8 — Difficulty Target & Nonce Fields
- Provide standard field representations for `difficulty_target` and `nonce: u64` to store Proof-of-Work solution headers.

---

## 6. Block Identification & Hashing Boundary

```text
Primary Primitive: BLAKE3 (32 bytes)
BlockID Derivation Status: TBD
```

- **Safety Rule:** The executing agent must **NOT** invent a custom BlockID hashing formula (e.g. ad-hoc double hashing or unvetted prefix formatting) before formal protocol specification.
- `BlockHeader` must provide the canonical byte serialization needed to compute its hash digest once the exact formula is locked.

---

## 7. Stateless Block Validation Primitives

The block domain object implements stateless structural verification:

```rust
impl Block {
    pub fn validate_structure(&self) -> Result<(), BlockError> {
        if self.transactions.is_empty() {
            return Err(BlockError::EmptyTransactionVector);
        }
        if !self.transactions[0].is_coinbase() {
            return Err(BlockError::MissingCoinbase);
        }
        for (idx, tx) in self.transactions.iter().enumerate().skip(1) {
            if tx.is_coinbase() {
                return Err(BlockError::DuplicateCoinbase(idx));
            }
            tx.validate_structure().map_err(BlockError::TransactionError)?;
        }
        Ok(())
    }
}
```

- **Separation of Concerns:** State-dependent checks (UTXO existence, signature authorization, cumulative difficulty, subsidy calculation) belong exclusively to the consensus engine.

---

## 8. Error Model

Block structural validation returns strongly-typed domain errors:

```rust
pub enum BlockError {
    EmptyTransactionVector,
    MissingCoinbase,
    DuplicateCoinbase(usize),
    InvalidHeader(String),
    TransactionCommitmentMismatch,
    TransactionError(TransactionError),
    SerializationFailure(String),
}
```

---

## 9. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 07 must fulfill the following test suites:

### Unit Tests:
- `test_block_header_instantiation`: Verify struct field assignments and equality.
- `test_block_with_single_coinbase`: Construct a minimal valid block containing only a coinbase transaction.
- `test_block_with_transactions`: Construct a multi-transaction block and verify vector ordering.
- `test_timestamp_integer_determinism`: Prove timestamps maintain bit-level parity across integer casts.

### Negative & Structural Tests:
- `test_reject_empty_block`: Assert `EmptyTransactionVector` when transaction list is empty.
- `test_reject_non_coinbase_at_index_0`: Assert `MissingCoinbase` if index 0 is a standard transaction.
- `test_reject_multiple_coinbases`: Assert `DuplicateCoinbase` if another coinbase exists at index $> 0$.
- `test_reject_nested_transaction_failure`: Assert block rejection if any included transaction fails structural validation.

---

## 10. Acceptance Criteria Checklist

Task 07 can only be marked as **VERIFIED** when:

- [ ] `BlockHeader` and `Block` domain structs are implemented in `scytale-core`.
- [ ] Existing `Transaction`, `TxId`, `Hash`, and `OutPoint` types are cleanly re-used.
- [ ] Linear parent linkage via `previous_block_hash` is supported.
- [ ] Exactly one coinbase at index 0 is strictly enforced by structural validation.
- [ ] Timestamp is represented as an explicit integer (`u64`) without system clock coupling.
- [ ] `difficulty_target` and `nonce` fields are present and accessible.
- [ ] Stateless structural validation is implemented and passes all negative test cases.
- [ ] No PoW mining loops, difficulty formulas, or database storage logic are introduced.
- [ ] 100% of unit and structural validation tests pass.
- [ ] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 11. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Domain structs and validation implementation underway.
     │
     ├── If commitment algorithm or block serialization is TBD ──> [ BLOCKED ]
     │                                                                   │
     │ <─────────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, negative, and structural tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 08 (Proof-of-Work).
```

- **Current Status:** **`PLANNED`**

---

## 12. Dependency for Downstream Tasks

- **Task 08 (Proof-of-Work):** Evaluates `BLAKE3(BlockHeader) <= Target` and verifies mathematical mining validity.

---

## 13. Agent Operating Rules

1. Treat `docs/work/07-block.md` as the authoritative work runbook.
2. Re-use primitives from Tasks 03–06; never duplicate types.
3. Keep the block model purely structural; do not implement mining or consensus rules in `Block`.
4. If transaction commitment or BlockID algorithms require protocol specification, set status to `BLOCKED`.
5. Adhere strictly to the definition of done and quality gates.

---

## 14. Cross-Specification References

- **[`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)**: Master block specification.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction specification.
- **[`docs/POW-SPEC.md`](../POW-SPEC.md)**: Proof-of-Work threshold rules.
- **[`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)**: Dynamic retargeting.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: 13 consensus validation rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Parameter registry.
- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Storage layout.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Threat model.
