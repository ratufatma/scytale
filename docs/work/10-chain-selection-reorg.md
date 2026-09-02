# Task 10 — Chain Selection & Reorganization

This document is the permanent **Task Execution Runbook** for Task 10: Chain Selection, Fork Handling, and Chain Reorganization. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's cumulative chain work evaluation, common ancestor resolution, atomic UTXO rollback/reapply transitions, and mempool reconciliation.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 10
Task Name   : Chain Selection / Reorganization
Phase       : Consensus
Level       : HEAVY
Status      : PLANNED
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Maximum supply and subsidy rules.
- **Task 02 — Genesis Allocation:** Genesis Block 0 anchor.
- **Task 03 — Transaction:** Transaction model and fee arithmetic.
- **Task 04 — UTXO:** In-memory state mutation and rollback primitives.
- **Task 05 — Authorization:** Input unlocking verification.
- **Task 06 — Hashing / Serialization:** 32-byte BLAKE3 digests and TxIDs.
- **Task 07 — Block:** `Block`, `BlockHeader`, and coinbase isolation.
- **Task 08 — Proof-of-Work:** Header validation ($\text{Hash} \le \text{Target}$).
- **Task 09 — Difficulty:** Dynamic retargeting and historical target validation.

### Core Reference Specifications:
- [`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/POW-SPEC.md`](../POW-SPEC.md)
- [`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)
- [`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)

---

## 2. Objective

> **Task Goal:** *Implement the deterministic Chain Selection and Reorganization engine in `scytale-consensus`, establishing the mathematical rules that evaluate competing valid branches, select the canonical chain exhibiting the greatest cumulative Proof-of-Work, and execute all-or-nothing UTXO rollbacks and mempool reconciliations.*

### Core Canonical Selection Rule:
$$\text{Canonical Chain} = \arg\max_{C \in \mathcal{V}} \left( \sum_{B \in C} \text{Work}(B) \right)$$

```text
Incoming Competing Branch
            ↓
[ 1. Validate Complete Branch Validity (Structure, PoW, UTXO) ]
            ↓ (Only Valid Branches Considered)
[ 2. Calculate Cumulative Chain Work ]
            ↓
Compare: Candidate Cumulative Work > Active Canonical Tip Work?
 ├── No  ──> Retain Branch in Storage as Non-Canonical Side Branch
 └── Yes ──> Trigger Atomic Chain Reorganization:
             ├── Find Common Ancestor Block
             ├── Disconnect Old Canonical Branch (Rollback UTXOs)
             ├── Connect New Candidate Branch (Apply New UTXOs)
             ├── Reconcile Disconnected Transactions with Mempool
             └── Atomically Update Canonical Tip
```

---

## 3. Core Principles & Selection Invariants

1. **Validity Precedes Work:** Cumulative computational work is **never** evaluated on invalid branches. A block with invalid signatures or double-spends is rejected immediately regardless of claimed work.
2. **Cumulative Work Metric (Not Block Height):** Selection is governed strictly by total accumulated computational work ($\sum 2^{256}/\text{Target}$), **not by simple block height**.
3. **Atomic State Coherence:** The committed state must maintain 100% coherence across four dimensions:
   $$\text{Canonical Tip} \iff \text{Canonical Height} \iff \text{Cumulative Work} \iff \text{Active UTXO Set}$$
4. **Zero Peer Trust:** Peer-provided cumulative work headers or branch claims are treated as untrusted hints until fully validated locally.

---

## 4. Subsystem Responsibility Boundaries

```text
┌─────────────────────────────────────────────────────────────────┐
│                 `scytale-consensus` (Task 10)                   │
│  - Fork branch tree evaluation & common ancestor discovery      │
│  - Cumulative work summation and canonical tip selection        │
│  - Reorganization orchestration (atomic rollback / re-apply)   │
└────────────────────────────────┬────────────────────────────────┘
                                 │
                                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                 Interfacing Subsystems                          │
│  - Storage Engine (`redb` atomicity)           ──> Task 14     │
│  - Mempool Re-admission Pipeline               ──> Task 11     │
│  - P2P Chain Sync & Wire Transport             ──> Task 15     │
│  - Autonomous Miner Template Reset             ──> Task 12     │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Work Items

### W1 — Inspect Existing Primitives
- Inspect `scytale-core` and `scytale-consensus` structs from Tasks 01–09.
- Re-use `Block`, `BlockHeader`, `Hash`, `OutPoint`, and `UtxoEntry` directly.

### W2 — Implement Cumulative Chain Work Accounting
- Implement checked 256-bit arithmetic for cumulative work:
  $$\text{CumulativeWork}(H+1) = \text{CumulativeWork}(H) + \text{BlockWork}(H+1)$$
- *Note on Status:* Exact integer representation formula for $\text{BlockWork} = 2^{256}/(\text{Target} + 1)$.

### W3 — Maintain Canonical Chain Tip State
- Track the active canonical state:
  ```rust
  pub struct ChainState {
      pub canonical_tip_hash: Hash,
      pub canonical_height: u64,
      pub cumulative_work: u256,
  }
  ```

### W4 — Competing Branch Management
- Store valid blocks that build upon ancestor blocks but currently possess less cumulative work than the active tip as `SideBranch` blocks.

### W5 — Common Ancestor Resolution
- Implement backward DAG traversal:
  $$\text{find\_common\_ancestor}(\text{active\_tip}: \&\text{Hash}, \text{candidate\_tip}: \&\text{Hash}) \rightarrow \text{Result}<\text{Hash}, \text{ChainError}>$$

### W6 — Reorganization Engine Orchestration
- Implement the atomic reorganization transition:
  1. Calculate disconnection vector: $[\text{Active Tip} \dots \text{Common Ancestor}]$.
  2. Calculate connection vector: $[\text{Common Ancestor} \dots \text{Candidate Tip}]$.
  3. Validate full state transition of all blocks in the connection vector against the rollback state.
  4. Atomically commit the new UTXO set and update the active tip.

### W7 — Mempool Transaction Reconciliation
- Collect all transactions included in the disconnected branch (excluding coinbase).
- Re-validate each transaction against the new canonical `UTXO_SET`:
  - If still valid and unspent $\rightarrow$ re-admit to mempool.
  - If conflicting or invalid $\rightarrow$ drop from mempool.

### W8 — Disconnected Coinbase Invalidation
- Ensure coinbase outputs from disconnected blocks are completely removed from the spendable UTXO set.

---

## 6. Equal-Work Tie-Breaking Policy

> [!WARNING]
> ### Equal-Work Tie-Break Status: `TBD`
> 
> If two competing valid branches exhibit mathematically identical cumulative Proof-of-Work:
> - **Operating Rule:** The node retains the first-received valid branch as its canonical tip until the tie is broken by the next valid block extension.
> - **Blocking Rule:** If the consensus specification requires a deterministic tie-breaker (e.g. lowest block hash value), mark status as `BLOCKED` until formally resolved.

---

## 7. Error Model

Chain selection and reorg operations return strongly-typed domain errors:

```rust
pub enum ChainError {
    InvalidBranchBlock { hash: Hash, reason: String },
    CommonAncestorNotFound { tip_a: Hash, tip_b: Hash },
    ReorgValidationFailed { block_hash: Hash, error: String },
    InsufficientWork { candidate: u256, active: u256 },
    CorruptedChainLinkage(Hash),
    StorageError(String),
}
```

---

## 8. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 10 must fulfill the following test suites:

### Unit & Fork Tests:
- `test_cumulative_work_accumulation`: Verify cumulative work increases monotonically with each connected block.
- `test_competing_fork_selection`: Construct fork with Branch A (Work 100) and Branch B (Work 150); assert Branch B becomes canonical.
- `test_reject_invalid_branch_even_with_high_work`: Assert a branch containing an invalid transaction is rejected regardless of claimed work.

### Reorganization Tests:
- `test_linear_reorg_execution`: Simulate 3-block rollback and 4-block re-application; verify canonical tip and height advance accurately.
- `test_utxo_rollback_reapply_integrity`: Verify UTXOs spent in disconnected blocks are restored, and outputs created in new blocks become spendable.
- `test_mempool_reconciliation_after_reorg`: Verify valid disconnected transactions re-enter the mempool.
- `test_coinbase_cleanup_on_reorg`: Verify disconnected coinbase outputs cannot be spent.

### Determinism Tests:
- Assert that feeding the same set of valid forks in differing network arrival orders deterministically converges on the exact same canonical tip.

---

## 9. Acceptance Criteria Checklist

Task 10 can only be marked as **VERIFIED** when:

- [ ] Cumulative Proof-of-Work calculation is implemented deterministically.
- [ ] Validity-before-work rule is strictly enforced.
- [ ] Common ancestor discovery algorithm is implemented and tested.
- [ ] Atomic chain reorganization engine is implemented.
- [ ] UTXO state rollback and re-application are verified.
- [ ] Disconnected non-coinbase transactions are reconciled with the mempool.
- [ ] Disconnected coinbase outputs are invalidated.
- [ ] Canonical state coherence (Tip, Height, Work, UTXO) is guaranteed.
- [ ] Zero peer-supplied metadata is trusted without local verification.
- [ ] 100% of unit, fork, and reorg test suites pass.
- [ ] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 10. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Reorg engine and fork tree implementation underway.
     │
     ├── If tie-break or chain work formula is TBD ──> [ BLOCKED ]
     │                                                       │
     │ <─────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All fork, reorg, and UTXO rollback tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 11 (Mempool).
```

- **Current Status:** **`PLANNED`**

---

## 11. Dependency for Downstream Tasks

- **Task 11 (Mempool):** Queries the active canonical `UTXO_SET` established by Task 10 to validate pending unconfirmed transactions and handles re-admission signals following reorgs.

---

## 12. Agent Operating Rules

1. Treat `docs/work/10-chain-selection-reorg.md` as the authoritative work runbook.
2. Re-use primitives from Tasks 01–09; do not create duplicate block or state types.
3. Guarantee atomic state transitions; partial reorg state is strictly prohibited.
4. If chain work formula or equal-work rules require protocol specification, set status to `BLOCKED`.
5. Adhere strictly to the definition of done and quality gates.

---

## 13. Cross-Specification References

- **[`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)**: Master chain selection specification.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Consensus invariants.
- **[`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)**: BlockHeader specification.
- **[`docs/POW-SPEC.md`](../POW-SPEC.md)**: Proof-of-Work rules.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: UTXO state transitions.
- **[`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)**: Mempool re-admission rules.
- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Atomic commit architecture.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)**: Lineage tracking.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
