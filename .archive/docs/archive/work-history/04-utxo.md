# Task 04 — UTXO Model

This document is the permanent **Task Execution Runbook** for Task 04: UTXO Model. It provides comprehensive technical instructions for agents and engineers to design, implement, test, and verify Scytale's Unspent Transaction Output (UTXO) domain structures, lifecycle transitions, double-spend prevention, and Value Provenance tracking.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 04
Task Name   : UTXO
Phase       : Ledger
Level       : MEDIUM
Status      : COMPLETED / PRODUCTION-READY
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Denomination, integer quanta accounting, and supply invariants.
- **Task 02 — Genesis Allocation:** Genesis OutPoints and initial distribution quotas.
- **Task 03 — Transaction:** `Transaction`, `TxIn`, `TxOut`, and `OutPoint` core primitives.

### Core Reference Specifications:
- [`docs/work/01-monetary-policy.md`](01-monetary-policy.md)
- [`docs/work/02-genesis-allocation.md`](02-genesis-allocation.md)
- [`docs/work/03-transaction.md`](03-transaction.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/LEDGER-SPEC.md`](../LEDGER-SPEC.md)
- [`docs/AUTHORIZATION-SPEC.md`](../AUTHORIZATION-SPEC.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective

> **Task Goal:** *Implement the Unspent Transaction Output (UTXO) domain model and state transition engine in `scytale-core`, establishing the authoritative representation of current spendable value, enforcing zero double-spends, and guaranteeing deterministic integer quanta conservation without adopting an account-balance model.*

### Core Architectural Invariants:
- **Explicit Primary Key:** Every UTXO is uniquely and unambiguously identified by its `OutPoint (TxID, OutputIndex)`.
- **Single-Spend Invariance:** An unspent output can be consumed exactly once in canonical history.
- **Pure Integer Precision:** All balances and UTXO values operate strictly in unsigned integer `quanta` ($1\text{ SCY} = 100,000,000\text{ quanta}$).
- **Value Lineage:** Every spendable UTXO connects deterministically to its creating transaction output.

---

## 3. Core UTXO Lifecycle

The lifecycle of every spendable quantum follows an unbroken sequence:

```text
Confirmed Transaction Output (TxOut)
                  ↓
          [ UTXO Created ]
                  ↓
       [ UTXO in Active Set ]  <── (Spendable State)
                  ↓
    Spending Transaction References OutPoint
                  ↓
         [ UTXO Consumed ]
                  ↓
    [ UTXO Removed from Active Set ]
                  ↓
      New Outputs Created (New UTXOs)
```

- **Exclusivity:** Once consumed in an accepted block, an `OutPoint` is permanently deleted from the active spendable set.

---

## 4. Scope & Non-Goals

### In Scope:
- Domain representation for UTXOs (`UtxoEntry`, `UtxoMap` / memory set).
- `OutPoint` uniqueness and equality operations.
- In-memory UTXO creation, lookup, and atomic consumption semantics.
- Confirmed-state double-spend detection and rejection.
- Value conservation verification across input and output sets.
- Partial value spending / UTXO splitting mechanics.
- Invariant, unit, and integration test suites.

### Out of Scope / Non-Goals:
- Implementing persistent disk tables or `redb` database transactions (deferred to Task 14 / Storage).
- Designing cryptographic signature validation algorithms (deferred to Task 05 / Authorization).
- Implementing mempool conflict resolution or eviction policies (deferred to Task 11 / Mempool).
- Implementing block headers, coinbase emission schedules, or mining loops (deferred to Consensus/Mining tasks).
- Creating Passbook or Wallet user interfaces.

---

## 5. Work Items

### W1 — Inspect Existing Primitives
- Review `Transaction`, `TxIn`, `TxOut`, and `OutPoint` implemented in Task 03.
- Re-use types directly from `scytale-core` without creating duplicate struct definitions.

### W2 — Define UTXO Domain Model
- Structure the unspent output container:
  ```rust
  pub struct UtxoEntry {
      pub output: TxOut,
      pub block_height: u64,
      pub is_coinbase: bool,
  }
  ```
- Implement safe domain wrapper collections (`UtxoSet` / in-memory map).

### W3 — Verify `OutPoint` Primary Key Identity
- Validate that `OutPoint { txid: Hash, index: u32 }` provides deterministic hashing, equality, and ordering.
- Assert that changing either `txid` or `index` yields an entirely distinct primary key.

### W4 — Implement UTXO Creation Mechanics
- Implement logic converting confirmed transaction outputs into active `UtxoEntry` records:
  $$\text{CreateUTXO}(\text{TxID}, \text{index}, \text{TxOut}) \rightarrow (\text{OutPoint}, \text{UtxoEntry})$$

### W5 — Implement UTXO Consumption & Spend Invalidation
- Implement consumption validation:
  1. Lookup referenced `OutPoint` in active set.
  2. If missing $\rightarrow$ return `UtxoError::MissingUtxo(OutPoint)`.
  3. If present $\rightarrow$ extract input value and atomically remove `OutPoint` from the unspent set.

---

## 6. Double-Spend Prevention Architecture

The UTXO engine strictly prevents double-spending across state transitions:

```text
                            Active UTXO A
                                   │
                   ┌───────────────┴───────────────┐
                   ▼                               ▼
       Transaction X (Valid Input)     Transaction Y (Conflicting Input)
      Consumes & Deletes UTXO A       References Missing UTXO A
                   │                               │
                   ▼                               ▼
               ACCEPTED                 REJECTED (Double Spend)
```

- **Stateful Isolation:** Confirmed double-spend rejection is enforced deterministically by checking against the active `UTXO_SET`.

---

## 7. Value Conservation & Partial Splitting

When an input UTXO is consumed to create multiple outputs of smaller values, value is strictly conserved:

```text
Input UTXO (1,000,000 quanta)
            │
            ▼
   Spending Transaction
            │
            ├── Output 0: 600,000 quanta  ──> New UTXO (TxID : 0)
            ├── Output 1: 399,000 quanta  ──> New UTXO (TxID : 1)
            └── Fee     :   1,000 quanta  ──> Implicit Miner Fee
```

$$\sum \text{Input UTXOs} = \sum \text{Created Output UTXOs} + \text{Transaction Fee}$$

- **Zero Coin Serialization:** Quanta values are stored as aggregate integers per UTXO; no individual records are generated per quantum.

---

## 8. Atomic State Transition Semantics

State transitions operate with strict all-or-nothing atomicity:

$$\text{ApplyTransition}(\text{UTXO\_SET}_{H}, \text{Block}) \rightarrow \text{UTXO\_SET}_{H+1}$$

```text
                    Pre-Transition UTXO Set
                               │
                               ▼
               [ 1. Validate All Input OutPoints Exist ]
               [ 2. Validate Authorization Proofs ]
               [ 3. Assert Sum(Inputs) >= Sum(Outputs) ]
                               │
                ├── Any Check Fails ──> Rollback (Zero State Mutation)
                └── All Passed       ──> Apply Atomic Diff:
                                         ├── Remove Consumed OutPoints
                                         └── Insert New Output OutPoints
                               │
                               ▼
                    Post-Transition UTXO Set
```

---

## 9. Architectural Boundaries: Core Semantics vs. Storage

Scytale preserves strict modular separation between memory semantics and database persistence:

```text
┌─────────────────────────────────────────────────────────────────┐
│                   `scytale-core` (Task 04)                      │
│  - UTXO struct definitions, in-memory validation, OutPoint keys │
│  - Zero database dependencies (No redb coupling in core)       │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                 `scytale-storage` (Task 14)                     │
│  - redb table persistence (`UTXO_SET`), disk serialization,     │
│    atomic commits, and crash recovery                           │
└─────────────────────────────────────────────────────────────────┘
```

---

## 10. Coinbase & Genesis Allocation Support

The UTXO engine supports outputs originating from both issuance pathways:
1. **Genesis Allocation Outputs:** Initial Block 0 outputs ($6.3\text{M}$ Founder, $2.1\text{M}$ Treasury, $2.1\text{M}$ Ecosystem) instantiate as standard spendable UTXOs.
2. **Coinbase Mining Outputs:** Block subsidy outputs instantiate as mined UTXOs.
   - *Note:* `Coinbase Maturity Depth: TBD` (Maturity rules will be enforced at the consensus layer, not hardcoded into Task 04 primitives).

---

## 11. Error Model

UTXO operations return strongly-typed domain errors:

```rust
pub enum UtxoError {
    MissingUtxo(OutPoint),
    AlreadySpent(OutPoint),
    ValueDeficit { total_in: u64, total_out: u64 },
    ArithmeticOverflow,
    InvalidCoinbasePlacement,
    ImmatureCoinbaseSpend { height: u64, required: u64 },
}
```

---

## 12. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 04 must satisfy the following test suites:

### Unit Tests:
- `test_outpoint_uniqueness`: Prove that differing indices or hashes produce distinct OutPoints.
- `test_utxo_creation_and_insertion`: Prove outputs convert to valid UTXO map entries.
- `test_utxo_successful_consumption`: Prove spending an output removes it from the unspent map.
- `test_integer_quanta_preservation`: Prove amounts remain strictly typed `u64` quanta.

### Invariant & Negative Tests:
- `test_reject_double_spend`: Prove attempting to consume the same `OutPoint` twice fails immediately.
- `test_reject_missing_outpoint`: Prove spending a non-existent `OutPoint` returns `MissingUtxo`.
- `test_reject_value_deficit`: Prove transactions where $\sum \text{Out} > \sum \text{In}$ are rejected.
- `test_atomic_transition_failure`: Prove that if one input in a batch is invalid, zero UTXOs are modified.

### Integration Tests (with Task 03):
- Build a full `Transaction`, resolve its inputs against an active `UtxoSet`, assert value conservation, consume inputs, and insert resulting outputs.

---

## 13. Acceptance Criteria Checklist

Task 04 can only be marked as **VERIFIED** when:

- [x] `UtxoEntry` and in-memory `UtxoSet` domain structures are implemented in `scytale-core`.
- [x] `OutPoint` primary key identity, equality, and hashing are verified.
- [x] UTXO creation from transaction outputs functions deterministically.
- [x] Single-spend enforcement and double-spend rejection are mathematically verified.
- [x] Value conservation ($\sum \text{In} \ge \sum \text{Out}$) is enforced across transitions.
- [x] All monetary calculations operate strictly in integer `quanta` (`u64`).
- [x] Atomic state transition semantics are established (zero partial state on failure).
- [x] Genesis and Coinbase outputs are fully supported.
- [x] Storage abstraction boundary is preserved (no `redb` dependencies in `scytale-core`).
- [x] 100% of unit and integration test suites pass.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 14. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Implementation underway in `scytale-core`.
     │
     ├── If structural conflict arises ──> [ BLOCKED ]
     │                                          │
     │ <────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit/invariant tests pass quality gates.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 05 (Authorization).
```

- **Current Status:** **`COMPLETED / PRODUCTION-READY`**

---

## 15. Dependency for Downstream Tasks

- **Task 05 (Authorization Model):** Verifies cryptographic unlocking proofs against the `locking_condition` held in the resolved `UtxoEntry`.

---

## 16. Agent Operating Rules

1. Treat `docs/work/04-utxo.md` as the authoritative work runbook.
2. Re-use primitives from Task 03; never create duplicate types.
3. Keep `scytale-core` free of database dependencies (`redb` belongs to Task 14).
4. Enforce strict integer quanta math with zero floating-point operations.
5. Adhere strictly to the definition of done and quality gates.

---

## 17. Cross-Specification References

- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: Master UTXO specification.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction specification.
- **[`docs/LEDGER-SPEC.md`](../LEDGER-SPEC.md)**: Master ledger model.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)**: Lineage tracking.
- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Storage partition architecture.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)**: Parameter registry.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Threat model.
