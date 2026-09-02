# Task 17 — Value Provenance & Lineage Tracing

This document is the permanent **Task Execution Runbook** for Task 17: Value Provenance & Lineage Tracing. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's deterministic value lineage engine, ancestral DAG backward traversal, issuance classification, cycle-safe graph queries, and Passbook auditability views.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 17
Task Name   : Value Provenance
Phase       : Ledger / Auditability
Level       : MEDIUM → HEAVY
Status      : PLANNED
```

### Primary Dependencies:
- **Task 02 — Genesis Allocation:** Genesis Block 0 root allocations.
- **Task 03 — Transaction:** Transaction structural models and OutPoint generation.
- **Task 04 — UTXO:** Active `UTXO_SET` resolution and input consumption lineage.
- **Task 06 — Hashing / Serialization:** 32-byte `TxID` and `BlockID` identifiers.
- **Task 07 — Block:** Block structure and coinbase reward outputs.
- **Task 10 — Chain Selection / Reorganization:** Canonical vs. non-canonical branch provenance.
- **Task 14 — Storage:** Historical transaction and block record lookups in `redb`.
- **Task 15 — Node Lifecycle:** Unified query interfaces.
- **Task 16 — Passbook:** User-facing presentation projections.

### Core Reference Specifications:
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/LEDGER-SPEC.md`](../LEDGER-SPEC.md)
- [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](../HASHING-AND-SERIALIZATION-SPEC.md)
- [`docs/GENESIS-SPEC.md`](../GENESIS-SPEC.md)
- [`docs/GENESIS-ALLOCATION.md`](../GENESIS-ALLOCATION.md)
- [`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)
- [`docs/PASSBOOK-CONCEPT.md`](../PASSBOOK-CONCEPT.md)
- [`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)

---

## 2. Objective & Core Provenance Question

> **Task Goal:** *Implement the deterministic Value Provenance query engine in `scytale-consensus` / `scytale-node`, enabling validating nodes, auditors, and users to deterministically answer the foundational question: **"Where did the value in this spendable UTXO originate from?"** by traversing backward through the immutable ledger DAG to its canonical genesis allocation or mining coinbase issuance.*

### Core Lineage Model:
$$\text{Current OutPoint} \longrightarrow \text{Creating Tx} \longrightarrow \text{Inputs} \longrightarrow \text{Parent UTXOs} \longrightarrow \dots \longrightarrow \text{Issuance Origin}$$

```text
Spendable UTXO (e.g. 500,000 quanta)
                │
                ▼
[ Creating Transaction TxID ] ──> Included in Block Height H
                │
                ▼ (Input Consumption Links)
[ Parent Spent OutPoints ] ─────> Parent TxIDs ─────> Block Height H_prev
                │
                ▼ (Iterative Backward Traversal)
┌──────────────────────────────────────────────────────────────┐
│                  Issuance Origin Classification              │
│                                                              │
│  ├── Genesis Block 0  ──> Founder / Treasury / Ecosystem     │
│  └── Block Coinbase   ──> Proof-of-Work Mining Subsidy       │
└──────────────────────────────────────────────────────────────┘
```

---

## 3. Core Principles & Architectural Invariants

- **Official Asset Identity:**
  ```text
  Project / Protocol : Scytale
  Native Coin Name   : Scytale Coin
  Ticker / Symbol    : SCY
  Smallest Unit      : quanta
  Conversion         : 1 SCY = 100,000,000 quanta (10^8 quanta)
  ```
- **Object-Level Lineage (No Record-Per-Quanta):** Provenance tracks value flow through ledger objects (`UTXO`, `Transaction`, `BlockHeader`). Scytale **never** creates individual database rows per quanta ($1\text{M quanta} = 1\text{ UTXO record}$, not $1\text{M records}$).
- **Deterministic Backward Traversal:** Given identical historical blocks in storage, traversing backward from any `OutPoint` always yields the exact same ancestral DAG.
- **No Arbitrary Value Creation:** Every valid UTXO strictly originates from either Genesis Block 0 or a valid Coinbase transaction satisfying the monetary emission schedule.
- **Zero Duplicate Storage:** Provenance traverses existing transaction/block records in `scytale-storage`; it does not build a secondary duplicate graph database.

---

## 4. Scope & Non-Goals

### In Scope:
- Backward DAG traversal from any active `OutPoint` to its creating transaction and block.
- Recursive multi-input ancestral tracing.
- Handling value splits and multi-input aggregations.
- Classifying terminal origins (`Genesis Allocation`, `Mining Reward`, `Transferred Value`).
- Cycle detection to protect against corrupt or cyclic malformed states.
- Unit, multi-hop lineage, coinbase origin, reorg awareness, and Passbook integration test suites.

### Out of Scope / Non-Goals:
- Implementing an external graph database engine (Neo4j, etc.).
- Tracking individual coin serial numbers or quanta IDs.
- Altering consensus validity rules based on provenance classifications (audit/query layer only).

---

## 5. Work Items

### W1 — Inspect Existing Primitives
- Inspect `crates/` and `apps/` for existing `OutPoint`, `Transaction`, `TxId`, `Block`, and `UtxoEntry` types.
- Re-use primitives directly without duplicate definitions.

### W2 — Implement Provenance Trace Data Models
- Define query result models:
  ```rust
  pub struct ProvenanceTrace {
      pub target_outpoint: OutPoint,
      pub value_quanta: u64,
      pub creating_txid: Hash,
      pub creating_block_height: u64,
      pub origin_classification: IssuanceOrigin,
      pub ancestral_inputs: Vec<AncestorNode>,
  }

  pub struct AncestorNode {
      pub outpoint: OutPoint,
      pub txid: Hash,
      pub value_quanta: u64,
      pub depth: usize,
      pub parent_inputs: Vec<AncestorNode>,
  }

  pub enum IssuanceOrigin {
      GenesisAllocation { category: GenesisCategory },
      MiningReward { block_height: u64, coinbase_txid: Hash },
      TransferredValue,
  }

  pub enum GenesisCategory {
      FounderAllocation,
      TreasuryAllocation,
      EcosystemAllocation,
  }
  ```

### W3 — Single-Hop Creation Resolution
- Given `OutPoint(txid, index)`:
  1. Lookup `txid` in `TRANSACTIONS` table $\rightarrow$ extract output value and locking condition.
  2. Lookup `txid` in `BLOCK_INDEX` $\rightarrow$ identify containing block hash and height.

### W4 — Multi-Hop Recursive Ancestor Traversal
- Implement cycle-safe recursive traversal:
  - For each input in the creating transaction:
    - Recursively resolve the parent `OutPoint`.
    - Terminate recursion when reaching a Coinbase transaction (`is_coinbase() == true`) or Genesis Block 0.

### W5 — Cycle Detection & Resource Throttling
- Maintain a visited set of `HashSet<OutPoint>` during traversal.
- If a visited `OutPoint` is encountered again $\rightarrow$ immediately abort with `ProvenanceError::CyclicLineageDetected`.
- Enforce strict depth limits (`MAX_PROVENANCE_DEPTH`).

### W6 — Value Conservation Verification
- For every non-coinbase transaction in the traversal path, assert:
  $$\sum \text{Input Values} = \sum \text{Output Values} + \text{Fee}$$

---

## 6. Provenance Operational Parameters (`TBD`)

The following query parameters remain designated as **TBD**:

| Operational Parameter | Status | Description |
| :--- | :---: | :--- |
| **`MAX_PROVENANCE_DEPTH`** | `TBD` | Maximum ancestral hop depth allowed in a single query (e.g. 100 hops). |
| **`PROVENANCE_QUERY_TIMEOUT`** | `TBD` | Maximum wall-clock execution time for deep DAG traversals. |
| **`PRUNED_PROVENANCE_HANDLING`**| `TBD` | Policy for handling historical branches if pruning is enabled in the future. |

---

## 7. Error Model

Provenance queries return strongly-typed domain errors:

```rust
pub enum ProvenanceError {
    OutPointNotFound(OutPoint),
    CreatingTransactionMissing(Hash),
    ContainingBlockMissing(Hash),
    CyclicLineageDetected(OutPoint),
    MaxDepthExceeded(usize),
    ValueConservationViolated { in_sum: u64, out_sum: u64 },
    StorageError(String),
}
```

---

## 8. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 17 must fulfill the following test suites:

### Unit & Lineage Tests:
- `test_direct_coinbase_provenance`: Assert UTXO from Coinbase resolves to `IssuanceOrigin::MiningReward`.
- `test_direct_genesis_provenance`: Assert UTXO from Genesis Block 0 resolves to `IssuanceOrigin::GenesisAllocation`.
- `test_multi_hop_transfer_lineage`: Create Chain: Genesis $\rightarrow$ Tx1 $\rightarrow$ Tx2 $\rightarrow$ Tx3; assert Tx3 UTXO traces back 3 hops to Genesis.
- `test_multi_input_lineage_resolution`: Spend 2 distinct UTXOs in 1 transaction; assert provenance captures both ancestral lineages.

### Safety & Invariant Tests:
- `test_cycle_detection_aborts_cleanly`: Inject synthetic cyclic mock DAG ($A \rightarrow B \rightarrow A$); verify `CyclicLineageDetected` error is returned without hanging.
- `test_no_record_per_quanta_verification`: Assert storage records remain $O(\text{UTXOs})$ rather than $O(\text{quanta})$.

### Reality & Passbook Integration Tests:
- Query real canonical ledger state via Passbook interface; assert provenance view matches exact transaction graph.

---

## 9. Acceptance Criteria Checklist

Task 17 can only be marked as **VERIFIED** when:

- [ ] OutPoint resolution to creating transaction and block is implemented.
- [ ] Multi-hop backward DAG traversal to Genesis or Coinbase is operational.
- [ ] Issuance origin classification (`GenesisAllocation`, `MiningReward`, `TransferredValue`) works accurately.
- [ ] Multi-input and value-split lineage tracking is supported.
- [ ] Object-level lineage is preserved (zero record-per-quanta design).
- [ ] Cycle detection safeguards prevent infinite loops on malformed histories.
- [ ] Strict integer quanta arithmetic is maintained across all provenance traces.
- [ ] Passbook provenance views display identical traces to canonical ledger queries.
- [ ] 100% of unit, lineage, safety, and integration test suites pass.
- [ ] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 10. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Traversal engine and classification underway.
     │
     ├── If query interface or depth limits are blocked ──> [ BLOCKED ]
     │                                                            │
     │ <──────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, lineage, and Passbook integration tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off. Roadmap tasks 01–17 fully defined.
```

- **Current Status:** **`PLANNED`**

---

## 11. Final Roadmap Completion (Tasks 01–17)

Task 17 represents the **final execution task** in the Scytale foundational roadmap:

```text
┌────────────────────────────────────────────────────────────────────────┐
│               SCYTALE FOUNDATIONAL ROADMAP (TASKS 01–17)               │
│                                                                        │
│   01. Monetary Policy                 10. Chain Selection / Reorg      │
│   02. Genesis Allocation              11. Mempool                      │
│   03. Transaction                     12. Mining Lifecycle             │
│   04. UTXO                            13. P2P Network (Go)             │
│   05. Authorization                   14. Storage (redb)               │
│   06. Hashing / Serialization         15. Node Lifecycle               │
│   07. Block                           16. Passbook                     │
│   08. Proof-of-Work                   17. Value Provenance             │
│   09. Difficulty Adjustment                                            │
└────────────────────────────────────────────────────────────────────────┘
```

- All 17 foundational tasks now possess explicit, permanent execution runbooks in `docs/work/`.

---

## 12. Agent Operating Rules

1. Treat `docs/work/17-value-provenance.md` as the authoritative work runbook.
2. Re-use existing storage records; never build a secondary graph database.
3. Provenance is strictly a read-only audit and query layer; never mutate consensus state from provenance code.
4. Enforce cycle detection and traversal depth bounds on all recursive lookups.
5. Adhere strictly to the definition of done and quality gates.

---

## 13. Cross-Specification References

- **[`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)**: Master value provenance specification.
- **[`docs/GENESIS-SPEC.md`](../GENESIS-SPEC.md)**: Genesis allocation origin rules.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction models.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: UTXO state transitions.
- **[`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)**: Block and Coinbase rules.
- **[`docs/PASSBOOK-CONCEPT.md`](../PASSBOOK-CONCEPT.md)**: Passbook presentation view.
- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Storage layout.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
