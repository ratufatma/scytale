# Task 03 — Transaction Model

This document is the permanent **Task Execution Runbook** for Task 03: Transaction Model. It provides comprehensive technical instructions for agents and engineers to design, implement, test, and verify Scytale's transaction primitives, validation rules, fee accounting, and identity derivation.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 03
Task Name   : Transaction
Phase       : Ledger
Level       : MEDIUM
Status      : VERIFIED
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Denomination ($1\text{ SCY} = 100,000,000\text{ quanta}$), integer accounting, and fee invariants.
- **Task 02 — Genesis Allocation:** Genesis OutPoint boundaries and fixed supply ceiling.

### Core Reference Specifications:
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/LEDGER-SPEC.md`](../LEDGER-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md)
- [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/GENESIS-SPEC.md`](../GENESIS-SPEC.md)
- [`docs/GENESIS-ALLOCATION.md`](../GENESIS-ALLOCATION.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)

---

## 2. Objective

> **Task Goal:** *Implement the minimal, deterministic, canonical Transaction data model and validation primitives in `scytale-core`, establishing the fundamental building blocks of the UTXO ledger with strictly non-floating-point monetary arithmetic.*

### Core Target Data Model:
```text
Transaction
├── version      : Protocol transaction version (u32)
├── inputs       : Vector of TxIn
└── outputs      : Vector of TxOut

TxIn
├── previous_output : OutPoint (TxID + OutputIndex)
└── authorization   : Cryptographic unlocking proof (byte vector / proof struct)

TxOut
├── value           : Amount in integer quanta (u64)
└── locking_condition: Encumbrance script / public key condition (byte vector)

OutPoint
├── txid            : 32-byte BLAKE3 transaction hash
└── output_index    : 0-based integer position (u32)
```

---

## 3. Core Principles & Monetary Rules

1. **Strict Integer Accounting:** Every monetary calculation (values, inputs, outputs, fees) uses unsigned 64-bit integer `quanta` (`u64`). Floating-point operations (`f32`, `f64`) are **strictly prohibited**.
2. **Value Conservation Invariant:**
   $$\sum \text{Input Values} = \sum \text{Output Values} + \text{Transaction Fee}$$
   $$\text{Transaction Fee} = \sum \text{Input Values} - \sum \text{Output Values} \ge 0$$
3. **No Arbitrary Minting:** Standard non-coinbase transactions can never generate new quanta ($\sum \text{Outputs} \le \sum \text{Inputs}$).
4. **Deterministic Identity:** Transaction identifier is pure and immutable:
   $$\text{TxID} = \text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{Transaction}))$$
5. **No Implicit Environmental State:** Zero timestamps, randomized salts, or machine-specific data may pollute transaction hashing.

---

## 4. Scope & Non-Goals

### In Scope:
- Transaction primitive structs (`Transaction`, `TxIn`, `TxOut`, `OutPoint`).
- Integer `quanta` domain wrappers and overflow-safe arithmetic helpers.
- Transaction-local structural validation.
- Fee calculation and value conservation logic.
- Transaction identity (`TxID`) integration via BLAKE3.
- Transaction error taxonomy.
- Unit and invariant test suites.

### Out of Scope / Non-Goals:
- Implementing UTXO database storage or `redb` table bindings (deferred to Task 04 / Storage).
- Implementing full block headers, coinbase logic, or mining routines (deferred to Block/Consensus tasks).
- Creating mempool queues, replacement policies, or P2P network wire daemons.
- Implementing Wallet or Passbook presentation UI components.
- Designing advanced cryptographic scripting languages (beyond minimal byte condition wrappers).

---

## 5. Work Items

### W1 — Inspect Existing Core Structure
- Review existing structs in `crates/scytale-core/src/lib.rs` and workspace `Cargo.toml`.
- Ensure zero duplicate primitive declarations or breaking workspace changes.

### W2 — Define Transaction Primitive Types
- Implement clean, documented Rust structs for `Transaction`, `TxIn`, `TxOut`, and `OutPoint` in `scytale-core`.
- Derive appropriate trait implementations (`Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `Serialize`, `Deserialize`).

### W3 — Define Quanta Amount Representation
- Enforce that all output amounts are denominated in atomic `quanta` ($1\text{ SCY} = 100,000,000\text{ quanta}$).
- Provide safe conversion helpers between human-readable SCY strings and `u64` quanta without floating-point intermediary steps.

### W4 — Implement `OutPoint` Primary Key
- Implement `OutPoint { txid: Hash, index: u32 }`.
- Ensure `OutPoint` supports equality and hashing for use as primary keys in downstream UTXO maps.

### W5 — Implement `TxIn` Structure
- Structure input references to prior outputs: `previous_output: OutPoint`.
- Provide `authorization` proof container as an isolated byte vector / domain wrapper without premature assumptions about final signature schemas.

### W6 — Implement `TxOut` Structure
- Structure output value encumbrance: `value: u64` quanta and `locking_condition: Vec<u8>`.

### W7 — Implement Transaction-Local Validation
- Implement stateless verification checking:
  1. Transaction `version` is supported.
  2. Input vector is non-empty for standard transactions.
  3. Output vector is non-empty.
  4. Every individual output value is $> 0$ (no dust/negative underflow).
  5. Sum of output values does not overflow `u64::MAX`.
  6. No duplicate identical input `OutPoints` within the same transaction payload.

### W8 — Implement Value Conservation & Fee Helper
- Implement checked summation helpers:
  $$\text{calculate\_fee}(\text{total\_input\_quanta}, \text{total\_output\_quanta}) \rightarrow \text{Result<u64, TransactionError>}$$
- Return explicit overflow or deficit errors if $\sum \text{Outputs} > \sum \text{Inputs}$.

### W9 — Implement Transaction Identity (`TxID`)
- Compute $\text{TxID} = \text{BLAKE3}(\text{canonical\_bytes})$.
- Ensure hashing operates over canonical binary representations matching [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md).

### W10 — Verify Deterministic Behavior
- Assert that identical transaction inputs and outputs produce bit-for-bit identical `TxID` hashes across any runtime architecture.

---

## 6. Coinbase Transaction Boundary

- **Isolation:** Ordinary transactions consume existing UTXOs; Coinbase transactions do not reference standard inputs.
- **Scope Division:** The full coinbase consensus rules (subsidy curve $R(h)$, height encoding, miner fee claims) belong to the Block & Consensus tasks.
- **Primitive Readiness:** The `Transaction` struct must be flexible enough to represent coinbase transactions (e.g., special `OutPoint` null values or dedicated coinbase constructors) without leaking block consensus dependencies into `scytale-core`.

---

## 7. Validation Boundaries: Stateless vs. Stateful

Scytale strictly decouples transaction validation layers:

```text
+-------------------------------------------------------------------------+
|                  Stateless Transaction-Local Validation                 |
|  - Executed within `scytale-core`                                       |
|  - Validates syntax, non-empty vectors, output value ranges, arithmetic |
+------------------------------------+------------------------------------+
                                     │
                                     ▼
+-------------------------------------------------------------------------+
|                   Stateful UTXO & Consensus Validation                  |
|  - Executed within `scytale-consensus` / `scytale-storage`              |
|  - Queries active `UTXO_SET` in `redb`                                  |
|  - Verifies cryptographic unlocking proofs against locking conditions   |
|  - Asserts unspent status and double-spend absence across the ledger    |
+-------------------------------------------------------------------------+
```

---

## 8. Error Model

Transaction-local operations must return strongly-typed domain errors:

```rust
pub enum TransactionError {
    InvalidVersion(u32),
    EmptyInputs,
    EmptyOutputs,
    ZeroOutputValue,
    OutputValueOverflow,
    DuplicateInput(OutPoint),
    InputValueDeficit { total_in: u64, total_out: u64 },
    ArithmeticOverflow,
    SerializationFailure(String),
}
```

---

## 9. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 03 must include comprehensive unit and property tests:

### Unit Tests:
- `test_outpoint_equality_and_hashing`: Verify equality, formatting, and hashing.
- `test_txin_txout_instantiation`: Verify struct construction and quanta validation.
- `test_transaction_creation_valid`: Verify valid transaction construction.
- `test_fee_calculation_exactness`: Verify $\text{Fee} = \sum \text{In} - \sum \text{Out}$.
- `test_quanta_safe_arithmetic`: Verify checked arithmetic prevents wrap-around.
- `test_txid_derivation_blake3`: Verify deterministic 32-byte BLAKE3 transaction hash generation.

### Negative Validation Tests:
- `test_reject_empty_inputs`: Reject transaction with zero inputs.
- `test_reject_empty_outputs`: Reject transaction with zero outputs.
- `test_reject_zero_output_value`: Reject zero-value outputs.
- `test_reject_output_sum_overflow`: Reject output sums exceeding `u64::MAX`.
- `test_reject_duplicate_inputs`: Reject intra-transaction duplicate input `OutPoints`.
- `test_reject_input_deficit`: Reject transactions where $\sum \text{Outputs} > \sum \text{Inputs}$.

---

## 10. Acceptance Criteria Checklist

Task 03 can only be marked as **VERIFIED** when:

- [x] `Transaction`, `TxIn`, `TxOut`, and `OutPoint` primitives are implemented in `scytale-core`.
- [x] Monetary amounts are strictly typed as unsigned integer `quanta` (`u64`).
- [x] Zero floating-point calculations exist in monetary math.
- [x] Deterministic fee calculation and value conservation helpers are implemented.
- [x] Stateless transaction-local validation is implemented and passes all negative tests.
- [x] Stateless vs. stateful validation boundaries are cleanly preserved.
- [x] `TxID` derivation matches the BLAKE3 canonical specification.
- [x] All unit and invariant test suites pass with 100% success.
- [x] Code passes all workspace quality gates (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 11. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Source code implementation underway in `scytale-core`.
     │
     ├── If serialization/auth dependency blocks execution ──> [ BLOCKED ]
     │                                                               │
     │ <─────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit tests and quality gates pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for consumption by Task 04 (UTXO).
```

- **Current Status:** **`VERIFIED`**

---

## 12. Dependency for Downstream Tasks

- **Task 04 (UTXO Model):** Consumes `OutPoint`, `TxIn`, `TxOut`, and `Transaction` structs to build the unspent transaction output state machine and `redb` storage adapters.

---

## 13. Agent Operating Rules

1. Treat `docs/work/03-transaction.md` as the authoritative work runbook.
2. Read all referenced specifications before writing code.
3. Check existing codebase structures before introducing new structs.
4. Never expand scope into storage, blocks, mempool, or network layers.
5. Adhere strictly to the definition of done and quality gates.

---

## 14. Cross-Specification References

- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction specification.
- **[`docs/LEDGER-SPEC.md`](../LEDGER-SPEC.md)**: Master ledger model.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: OutPoint and UTXO definitions.
- **[`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md)**: Unlocking conditions.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 digests.
- **[`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)**: Quanta arithmetic and fee rules.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Parameter registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
