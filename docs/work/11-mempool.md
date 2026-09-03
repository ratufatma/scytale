# Task 11 — Mempool

This document is the permanent **Task Execution Runbook** for Task 11: Mempool (Transaction Pool). It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's local pending-transaction state machine, admission validation pipeline, conflict detection, fee ranking metadata, and eviction handling.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 11
Task Name   : Mempool
Phase       : Runtime / Transaction Processing
Level       : MEDIUM → HEAVY
Status      : COMPLETED / PRODUCTION-READY
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Integer quanta fee arithmetic.
- **Task 02 — Genesis Allocation:** Genesis accounting boundary.
- **Task 03 — Transaction:** Transaction structural models.
- **Task 04 — UTXO:** Active `UTXO_SET` resolution and OutPoint checking.
- **Task 05 — Authorization:** Input unlocking verification.
- **Task 06 — Hashing / Serialization:** 32-byte `TxID` derivation.
- **Task 07 — Block:** Block transactions vector.
- **Task 08 — Proof-of-Work:** PoW validation boundary.
- **Task 09 — Difficulty:** Retargeting rules.
- **Task 10 — Chain Selection / Reorganization:** Canonical tip updates and reorg signals.

### Core Reference Specifications:
- [`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)
- [`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)

---

## 2. Objective & Architectural Role

> **Task Goal:** *Implement the local pending-transaction state machine (`Mempool`) in `scytale-mempool`, providing high-performance transaction admission, deduplication, conflict detection, parent-child dependency tracking, and fee-rate ranking for block template construction without ever conferring canonical ledger authority to unconfirmed transactions.*

### Mempool vs. Canonical State Separation:
```text
┌───────────────────────────────────────┐
│        Local Pending Domain           │
│  - Ephemeral unconfirmed pool         │
│  - Node-local admission policies      │
│  - Mempool state != Canonical State   │
└──────────────────┬────────────────────┘
                   │
                   ▼ (Block Confirmation)
┌───────────────────────────────────────┐
│       Canonical Ledger Domain         │
│  - redb committed storage             │
│  - Active UTXO_SET & Canonical Chain  │
│  - Sole source of truth for balances  │
└───────────────────────────────────────┘
```

- **Non-Consensus Principle:** Two honest nodes may maintain differing mempool contents due to network latency without causing a consensus fork.

---

## 3. Scope & Non-Goals

### In Scope:
- Transaction admission pipeline (stateless validation $\rightarrow$ UTXO lookup $\rightarrow$ authorization $\rightarrow$ conflict check).
- `TxID` index mapping and deduplication.
- In-flight double-spend detection across pending transactions.
- In-memory parent-child transaction dependency resolution.
- Fee calculation and metadata tracking (`fee`, `fee_rate`, `size`).
- Transaction removal upon block confirmation or conflict invalidation.
- Unit, double-spend, dependency, block arrival, and reorg reconciliation test suites.

### Out of Scope / Non-Goals:
- Implementing the continuous background mining search loop (deferred to Task 12 / Mining).
- Implementing P2P wire framing, inventory gossip, or socket management (deferred to Task 15 / P2P).
- Implementing persistent disk table serialization in `redb` (deferred to Task 14 / Storage).
- Designing Passbook balance presentation or signing interfaces.

---

## 4. Work Items

### W1 — Inspect Existing Primitives
- Inspect `crates/scytale-mempool/src/lib.rs` and workspace types.
- Re-use `Transaction`, `TxId`, `Hash`, `OutPoint`, and `UtxoEntry` from prior tasks.

### W2 — Implement Mempool Entry Structure
- Structure pending transaction metadata:
  ```rust
  pub struct MempoolEntry {
      pub transaction: Transaction,
      pub txid: Hash,
      pub fee: u64,
      pub fee_rate: u64,
      pub size_bytes: usize,
      pub added_time: u64,
  }
  ```

### W3 — Transaction Admission Pipeline
- Implement the sequential verification pipeline:
  1. Stateless structural validation (`tx.validate_structure()`).
  2. Duplicate check: Assert `!mempool.contains(&txid)`.
  3. Resolve input `OutPoints` against active `UTXO_SET` + pending mempool outputs.
  4. Verify cryptographic unlocking proofs (`tx.verify_authorization()`).
  5. Enforce integer value conservation ($\sum \text{In} \ge \sum \text{Out}$).
  6. Assert no input `OutPoint` is consumed by another pending transaction.
  7. Insert into `Mempool` map.

### W4 — Pending Double-Spend & Conflict Rejection
- Maintain a reverse lookup index: `spent_outpoints: HashMap<OutPoint, Hash>`.
- If an incoming transaction references an `OutPoint` already claimed by a pending transaction:
  - *Default Rule:* Reject the incoming transaction immediately.
  - *Note on Status:* `Replacement Policy (RBF): TBD`.

### W5 — In-Memory Parent-Child Dependency Tracking
- Allow Transaction B to spend an unconfirmed output of pending Transaction A.
- Maintain dependency links (`parents: HashSet<Hash>`, `children: HashSet<Hash>`).

### W6 — Block Inclusion & Eviction Hook
- When a new canonical block is accepted:
  1. For each transaction in the block $\rightarrow$ remove corresponding `TxID` from mempool.
  2. For remaining pending transactions $\rightarrow$ re-evaluate input validity against the mutated `UTXO_SET`.
  3. Evict any pending transactions whose inputs were spent by other transactions in the accepted block.

### W7 — Reorganization Re-admission Handling
- When a chain reorganization occurs:
  1. Receive vector of disconnected transactions from Task 10.
  2. Re-run admission pipeline for each disconnected transaction against the new canonical state.
  3. Re-admit valid transactions back into the mempool.

---

## 5. Local Resource Policies (`TBD`)

The following operational limits are node-local policies (not universal consensus rules):

| Policy Parameter | Status | Description |
| :--- | :---: | :--- |
| **`MINIMUM_RELAY_FEE_RATE`** | `TBD` | Minimum fee-density required for admission. |
| **`MAX_MEMPOOL_CAPACITY_BYTES`** | `TBD` | Maximum in-memory RAM allocated to the pool. |
| **`MAX_TRANSACTION_SIZE_BYTES`** | `TBD` | Maximum byte size for a single transaction. |
| **`TRANSACTION_EXPIRATION_TIME`** | `TBD` | Maximum duration a transaction remains pending. |
| **`MEMPOOL_EVICTION_STRATEGY`** | `TBD` | Eviction criteria when RAM limit is reached (e.g. lowest fee-rate first). |

---

## 6. Error Model

Mempool admission returns strongly-typed domain errors:

```rust
pub enum MempoolError {
    DuplicateTx(Hash),
    MissingInputUtxo(OutPoint),
    ConflictDoubleSpend { outpoint: OutPoint, conflicting_tx: Hash },
    AuthorizationFailed(AuthorizationError),
    ValueDeficit { total_in: u64, total_out: u64 },
    BelowMinimumFeeRate { actual: u64, minimum: u64 },
    OversizedTransaction { size: usize, max: usize },
    MempoolFull,
    InvalidTransactionStructure(BlockError),
}
```

---

## 7. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 11 must fulfill the following test suites:

### Unit Tests:
- `test_admit_valid_transaction`: Verify successful admission and query by `TxID`.
- `test_reject_duplicate_txid`: Verify rejection on duplicate insertion.
- `test_fee_metadata_calculation`: Verify exact `fee = In - Out` and fee-rate calculation.

### Conflict & Dependency Tests:
- `test_reject_pending_double_spend`: Assert second transaction spending the same `OutPoint` is rejected.
- `test_parent_child_admission`: Assert child transaction spending pending parent output is admitted.
- `test_parent_removal_evicts_child`: Assert removing parent evicts dependent unconfirmed children.

### Block Arrival & Reorg Tests:
- `test_block_inclusion_removes_transactions`: Verify confirmed transactions disappear from mempool.
- `test_block_inclusion_evicts_conflicting_pending_tx`: Verify conflicting transactions are dropped.
- `test_reorg_readmission`: Verify valid disconnected transactions return to the pending pool.

---

## 8. Acceptance Criteria Checklist

Task 11 can only be marked as **VERIFIED** when:

- [x] `Mempool` data structure and entry models are implemented in `scytale-mempool`.
- [x] `TxID` is enforced as unique primary key for pending entries.
- [x] Full admission pipeline (stateless $\rightarrow$ UTXO $\rightarrow$ auth $\rightarrow$ conflict) is implemented.
- [x] In-flight double-spend conflict detection is verified.
- [x] Parent-child dependency resolution is supported.
- [x] Confirmed block inclusion cleanly removes transactions from the pool.
- [x] Conflicting pending transactions are evicted upon block confirmation.
- [x] Mempool state remains strictly decoupled from canonical ledger state.
- [x] Zero arbitrary minting or fee inflation is permitted.
- [x] 100% of unit, conflict, dependency, and block arrival test suites pass.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 9. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Pool structure and admission pipeline underway.
     │
     ├── If replacement policy or mempool bounds require protocol lock ──> [ BLOCKED ]
     │                                                                           │
     │ <─────────────────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, conflict, and block arrival tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 12 (Mining Lifecycle).
```

- **Current Status:** **`COMPLETED / PRODUCTION-READY`**

---

## 10. Dependency for Downstream Tasks

- **Task 12 (Mining Lifecycle):** Queries `Mempool` to select highest-fee pending transactions and assemble candidate block templates.

---

## 11. Agent Operating Rules

1. Treat `docs/work/11-mempool.md` as the authoritative work runbook.
2. Re-use primitives from Tasks 03–10; do not create duplicate transaction or hash types.
3. Mempool is strictly local state; never mutate canonical state from mempool operations.
4. Do not invent replacement (RBF) or package relay policies without formal specification.
5. Adhere strictly to the definition of done and quality gates.

---

## 12. Cross-Specification References

- **[`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)**: Master mempool specification.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction specification.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: UTXO state transitions.
- **[`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)**: Reorg re-admission rules.
- **[`docs/MINING-LIFECYCLE-SPEC.md`](../MINING-LIFECYCLE-SPEC.md)**: Block template selection.
- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Storage separation.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
