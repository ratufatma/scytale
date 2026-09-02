# Task 16 — Passbook Presentation Layer

This document is the permanent **Task Execution Runbook** for Task 16: Passbook Presentation Layer. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's human-readable financial history projection, confirmed/pending balance derivations, sequential entry numbering, and provenance lineage views.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 16
Task Name   : Passbook
Phase       : User Experience / Ledger Presentation
Level       : MEDIUM
Status      : PLANNED
```

### Primary Dependencies:
- **Task 03 — Transaction:** Transaction structural models and fee arithmetic.
- **Task 04 — UTXO:** Active `UTXO_SET` resolution and OutPoint status.
- **Task 05 — Authorization:** Output locking condition representations.
- **Task 06 — Hashing / Serialization:** 32-byte `TxID` and `BlockID` display strings.
- **Task 07 — Block:** Historical block timestamps and confirmations.
- **Task 10 — Chain Selection / Reorganization:** Canonical tip updates and reorg re-projections.
- **Task 11 — Mempool:** Pending unconfirmed transaction queries.
- **Task 14 — Storage:** Ledger history persistence.
- **Task 15 — Node Lifecycle:** Unified node query interface (`READY` state).

### Core Reference Specifications:
- [`docs/PASSBOOK-CONCEPT.md`](../PASSBOOK-CONCEPT.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)
- [`docs/NODE-LIFECYCLE-SPEC.md`](../NODE-LIFECYCLE-SPEC.md)
- [`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective & Architectural Role

> **Task Goal:** *Implement the read-only Passbook financial presentation layer in `scytale-node`, projecting complex UTXO graphs into a clean, familiar bank passbook experience (Buku Tabungan) with sequential entry numbers, clear confirmed/pending statuses, precise integer quanta calculations, and unbroken provenance lineage links without ever altering or creating ledger state.*

### Passbook vs. Ledger Boundary:
```text
┌───────────────────────────────────────────────────────────────┐
│                      CANONICAL LEDGER                         │
│  - redb committed storage & Active UTXO_SET                   │
│  - Mathematical source of truth for all balances              │
└──────────────────────────────┬────────────────────────────────┘
                               │ Node Query Interface
                               ▼
┌───────────────────────────────────────────────────────────────┐
│                  PASSBOOK PRESENTATION LAYER                  │
│  - Derives Confirmed Balance: Σ Spendable Canonical UTXOs     │
│  - Human-friendly Passbook Entry Numbers (#000001, #000002)   │
│  - Displays Provenance Lineage (OutPoint ──> TxID ──> Block)  │
│  - Zero Consensus Authority (Cannot mint, burn, or transfer)  │
└───────────────────────────────────────────────────────────────┘
```

- **Core Invariant:** *Passbook displays ledger state; it never defines, stores, or mutates ledger state.*

---

## 3. Zero-Balance Initial State & Units

```text
New User Initial State : 0 SCY (0 quanta)
Monetary Precision     : 1 SCY = 100,000,000 quanta
Internal Accounting    : Strict Integer Arithmetic (Zero floating-point)
```

- **Bootstrap Flow:** Creating a new Passbook instance starts with exactly **0 SCY**. Only when the node receives funds or mines a canonical block does the derived balance become $> 0$.

---

## 4. Scope & Non-Goals

### In Scope:
- Deriving confirmed balance by summing active spendable canonical UTXOs.
- Tracking and displaying pending mempool transactions separately from confirmed balances.
- Generating sequential, human-friendly Passbook Entry Numbers (`#000123`).
- Classifying entries by transaction type (`Received`, `Sent`, `Mining Reward`, `Fee`, `Change`).
- Presenting provenance lineage metadata (TxID, Output Index, Block Height, Origin).
- Unit, reorg projection, mining reward display, and restart test suites.

### Out of Scope / Non-Goals:
- Managing private keys, mnemonic seed phrases, or transaction signing (wallet domain).
- Reading raw database tables in `redb` directly (queries pass through Node interface).
- Creating artificial balances or synthetic test tokens.
- Designing graphical desktop/web UI widgets (presentation domain logic only).

---

## 5. Work Items

### W1 — Inspect Existing Workspace Structure
- Inspect `crates/` and `apps/` for query interfaces and domain representations.
- Re-use `Transaction`, `TxId`, `OutPoint`, `Hash`, and `Quanta` types directly.

### W2 — Implement Passbook Domain Models
- Define read-only projection models:
  ```rust
  pub struct PassbookView {
      pub confirmed_balance_quanta: u64,
      pub pending_balance_quanta: i64,
      pub total_entries: usize,
      pub entries: Vec<PassbookEntry>,
  }

  pub struct PassbookEntry {
      pub entry_number: u64,
      pub timestamp: u64,
      pub entry_type: EntryType,
      pub amount_quanta: u64,
      pub fee_quanta: u64,
      pub status: EntryStatus,
      pub txid: Hash,
      pub outpoint: Option<OutPoint>,
      pub block_height: Option<u64>,
  }

  pub enum EntryType {
      Received,
      Sent,
      MiningReward,
      Change,
  }

  pub enum EntryStatus {
      Confirmed { confirmations: u64 },
      Pending,
      Reorganized,
  }
  ```

### W3 — Balance Derivation Engine
- Query active canonical `UTXO_SET` via Node query interface:
  $$\text{Confirmed Balance} = \sum_{u \in \text{UserUTXOs}} u.\text{value}$$
- Calculate pending delta:
  $$\text{Pending Delta} = \sum \text{Pending Inflows} - \sum \text{Pending Outflows}$$

### W4 — Transaction History Projection
- Project confirmed transactions into ordered chronological entries.
- Assign local sequential `entry_number` integers ($1, 2, 3, \dots$) for easy human reference.

### W5 — Provenance Lineage View
- For each entry, expose the complete value provenance trace:
  $$\text{Current OutPoint} \longrightarrow \text{Creating TxID} \longrightarrow \text{Block Height} \longrightarrow \text{Coinbase / Genesis}$$

### W6 — Reorganization Projection Synchronization
- When a chain reorganization occurs (Task 10):
  - Passbook automatically re-queries the updated canonical state.
  - Transactions dropped from canonical status are marked as `Reorganized` or `Pending`.
  - Confirmed balance immediately reflects the new canonical branch.

---

## 6. Passbook Operational Policies (`TBD`)

The following presentation parameters remain designated as **TBD**:

| Parameter | Status | Description |
| :--- | :---: | :--- |
| **`CONFIRMATION_THRESHOLD_FINAL`** | `TBD` | Number of confirmations considered high-assurance. |
| **`PASSBOOK_CACHE_POLICY`** | `TBD` | In-memory cache invalidation strategy for historical entries. |
| **`CHANGE_IDENTIFICATION_RULES`** | `TBD` | Heuristics for separating change outputs from recipient outputs. |
| **`UI_FRAMEWORK_BINDINGS`** | `TBD` | Target GUI/CLI integration layer. |

---

## 7. Error Model

Passbook projection queries return strongly-typed domain errors:

```rust
pub enum PassbookError {
    NodeNotReady,
    UtxoLookupFailed(String),
    TransactionNotFound(Hash),
    ProvenanceLineageBroken(OutPoint),
    StaleLedgerState,
}
```

---

## 8. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 16 must fulfill the following test suites:

### Unit Tests:
- `test_zero_balance_initialization`: Verify fresh passbook returns 0 quanta.
- `test_balance_summation_multiple_utxos`: Insert 3 UTXOs (100, 200, 300 quanta); assert confirmed balance = 600 quanta.
- `test_passbook_entry_numbering`: Assert sequential assignment of entry numbers (#1, #2, #3).
- `test_confirmed_vs_pending_separation`: Assert pending transaction does not inflate confirmed balance.

### Integration & Reorg Tests:
- `test_mining_reward_reflection`: Mine a block in test harness; verify Coinbase UTXO creates a `MiningReward` entry.
- `test_reorganization_updates_passbook`: Simulate 2-block rollback; assert passbook entries update to reflect new canonical branch.
- `test_restart_preserves_passbook_integrity`: Restart node; verify passbook projection reproduces identical balance and history.

---

## 9. Acceptance Criteria Checklist

Task 16 can only be marked as **VERIFIED** when:

- [ ] Passbook projection domain model is implemented in `scytale-node`.
- [ ] Initial balance is strictly 0 SCY with zero synthetic tokens.
- [ ] Confirmed balance is derived dynamically from canonical `UTXO_SET`.
- [ ] Pending unconfirmed transactions are clearly distinguished from confirmed balance.
- [ ] Sequential Passbook Entry Numbers are assigned for human readability.
- [ ] Transaction types (`Received`, `Sent`, `MiningReward`, `Change`) are classified accurately.
- [ ] Value provenance lineage is inspectable from UTXO back to origin.
- [ ] Chain reorganizations update passbook projections automatically.
- [ ] Passbook maintains zero consensus or monetary authority.
- [ ] Zero private key or signing logic exists in this presentation layer.
- [ ] 100% of unit, reorg, and integration test suites pass.
- [ ] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 10. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Passbook models and derivation queries underway.
     │
     ├── If Node query interface or provenance schema is blocked ──> [ BLOCKED ]
     │                                                                      │
     │ <────────────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, reorg, and balance derivation tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 17 (Value Provenance).
```

- **Current Status:** **`PLANNED`**

---

## 11. Dependency for Downstream Tasks

- **Task 17 (Value Provenance Deep Validation):** Provides specialized backward graph traversal primitives to enrich Passbook lineage visualizations.

---

## 12. Agent Operating Rules

1. Treat `docs/work/16-passbook.md` as the authoritative work runbook.
2. Passbook is strictly a presentation layer; never store an independent balance ledger.
3. Keep wallet key management and signing strictly out of scope.
4. Guarantee zero-balance initial state with zero artificial deposits.
5. Adhere strictly to the definition of done and quality gates.

---

## 13. Cross-Specification References

- **[`docs/PASSBOOK-CONCEPT.md`](../PASSBOOK-CONCEPT.md)**: Master Passbook design concept.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)**: Provenance lineage specification.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction models.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: UTXO state transitions.
- **[`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)**: Reorganization updates.
- **[`docs/NODE-LIFECYCLE-SPEC.md`](../NODE-LIFECYCLE-SPEC.md)**: Node query interfaces.
- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Storage layout.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
