# Task 14 — Persistent Storage Engine

This document is the permanent **Task Execution Runbook** for Task 14: Storage with `redb`. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's transactional persistence layer, key-value table definitions, all-or-nothing atomic block commits, UTXO mutations, and crash-resilient restart recovery.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 14
Task Name   : Storage
Phase       : Runtime / Persistence
Level       : MEDIUM → HEAVY
Status      : VERIFIED
```

### Primary Dependencies:
- **Task 03 — Transaction:** Transaction byte layout and OutPoint keys.
- **Task 04 — UTXO:** In-memory `UtxoEntry` and primary OutPoint mappings.
- **Task 06 — Hashing / Serialization:** 32-byte `Hash` digests for TxID and BlockID primary keys.
- **Task 07 — Block:** `Block` and `BlockHeader` structures.
- **Task 10 — Chain Selection / Reorganization:** Canonical tip updates and reorg rollback vectors.
- **Task 11 — Mempool:** State query interfaces.
- **Task 12 — Mining Lifecycle:** Block template assembly queries.
- **Task 13 — P2P Network:** Historical block storage for Initial Block Sync (IBD).

### Core Reference Specifications:
- [`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)
- [`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective & Architectural Role

> **Task Goal:** *Implement the embedded, zero-overhead transactional persistence engine in `scytale-storage` using `redb`, providing ACID transactions, deterministic primary key lookups, atomic multi-table block commits, and durable restart recovery while ensuring domain crates (`scytale-core`) remain 100% decoupled from database engine dependencies.*

### Storage vs. Consensus Boundary:
```text
┌───────────────────────────────────────┐
│        scytale-consensus              │
│  - Evaluates mathematical rules       │
│  - Decides state transitions          │
│  - Pure, stateless domain logic       │
└──────────────────┬────────────────────┘
                   │ Validated State Transition
                   ▼
┌───────────────────────────────────────┐
│         scytale-storage (redb)        │
│  - Persists committed canonical state │
│  - Stores immutable historical blocks │
│  - Zero consensus authority           │
└───────────────────────────────────────┘
```

- **Fundamental Invariant:** *Storage persists validated state; it never decides monetary policy, transaction validity, or fork selection.*

---

## 3. Scope & Non-Goals

### In Scope:
- Initializing embedded `redb` database instances at designated file paths.
- Defining strongly-typed table schemas for Blocks, Transactions, UTXOs, Block Index, and Chain State.
- All-or-nothing atomic block commits (`WriteTransaction` encompassing block insertion + UTXO consumption + creation + tip update).
- Historical transaction and block lookups by 32-byte hash.
- Fast OutPoint key resolution for UTXO state validation.
- Unit, atomicity, crash recovery, and restart test suites.

### Out of Scope / Non-Goals:
- Implementing historical data pruning or blockchain state trimming (pruning is out of scope).
- Generating genesis allocations automatically upon database creation (node bootstrap responsibility).
- Implementing network RPC daemons or remote SQL/gRPC query proxies.

---

## 4. Work Items

### W1 — Inspect Existing Storage Workspace
- Inspect `crates/scytale-storage/src/lib.rs` and workspace dependencies.
- Confirm `redb` dependency alignment without arbitrary version bumping.

### W2 — Implement Storage Table Definitions
- Define canonical tables in `scytale-storage`:
  ```rust
  // Conceptual Table Definitions
  pub const BLOCKS: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("blocks");
  pub const TRANSACTIONS: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("transactions");
  pub const UTXOS: TableDefinition<&[u8; 36], &[u8]> = TableDefinition::new("utxos"); // 32-byte TxID + 4-byte Index
  pub const BLOCK_INDEX: TableDefinition<&[u8; 32], &[u8]> = TableDefinition::new("block_index");
  pub const CHAIN_STATE: TableDefinition<&str, &[u8]> = TableDefinition::new("chain_state");
  ```

### W3 — Block & Transaction CRUD Operations
- Implement CRUD methods:
  - `put_block(&mut WriteTransaction, block: &Block) -> Result<()>`
  - `get_block(&ReadTransaction, block_id: &Hash) -> Result<Option<Block>>`
  - `put_transaction(&mut WriteTransaction, tx: &Transaction) -> Result<()>`
  - `get_transaction(&ReadTransaction, txid: &Hash) -> Result<Option<Transaction>>`

### W4 — UTXO Set Storage Operations
- Implement fast OutPoint key indexing:
  - `insert_utxo(&mut WriteTransaction, outpoint: &OutPoint, entry: &UtxoEntry) -> Result<()>`
  - `spend_utxo(&mut WriteTransaction, outpoint: &OutPoint) -> Result<()>`
  - `get_utxo(&ReadTransaction, outpoint: &OutPoint) -> Result<Option<UtxoEntry>>`

### W5 — Atomic Block Commit Pipeline (Critical Invariant)
- Execute the state commit as a single atomic `redb::WriteTransaction`:
  1. Write full serialized `Block` to `BLOCKS`.
  2. For each transaction $\rightarrow$ write serialized bytes to `TRANSACTIONS`.
  3. For each transaction input $\rightarrow$ delete spent `OutPoint` from `UTXOS`.
  4. For each transaction output $\rightarrow$ insert new `OutPoint` and `UtxoEntry` into `UTXOS`.
  5. Update `BLOCK_INDEX` (height, cumulative work, parent linkage).
  6. Atomically update `CHAIN_STATE` (canonical tip hash and active height).
  7. Commit `WriteTransaction`.

### W6 — Chain Reorganization Rollback Support
- Provide atomic rollback primitives allowing `scytale-consensus` (Task 10) to disconnect a side branch, restore spent ancestor UTXOs, and apply the candidate branch within a single `WriteTransaction`.

---

## 5. Storage Invariants & Policies (`TBD`)

The following storage configuration parameters remain designated as **TBD**:

| Storage Parameter | Status | Description |
| :--- | :---: | :--- |
| **`BINARY_KEY_CODEC`** | `TBD` | Canonical binary key serialization (reusing Task 06 codecs). |
| **`SCHEMA_VERSION_IDENTIFIER`** | `TBD` | Database schema version tag for migration discovery. |
| **`STORAGE_CACHE_CAPACITY`** | `TBD` | In-memory LRU cache size for active UTXOs. |
| **`PRUNING_POLICY`** | `TBD` | Historical transaction pruning strategy (deferred). |

---

## 6. Error & Failure Model

Storage operations return strongly-typed domain errors:

```rust
pub enum StorageError {
    DatabaseCorrupted(String),
    TransactionAborted(String),
    KeyNotFound(String),
    CodecSerializationFailure(String),
    InconsistentState { tip: Hash, height: u64 },
    RedbError(String),
}
```

- **Fail-Closed Principle:** If a `WriteTransaction` encounters any disk I/O or serialization error, the transaction is immediately aborted by `redb`, leaving the existing canonical ledger 100% intact.

---

## 7. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 14 must fulfill the following test suites:

### Unit Tests:
- `test_database_open_and_table_init`: Verify database file creation and table discovery.
- `test_block_and_transaction_roundtrip`: Insert block and verify byte-exact retrieval.
- `test_utxo_insert_lookup_spend`: Insert UTXO, verify lookup returns `Some`, delete, assert lookup returns `None`.

### Atomicity & Rollback Tests:
- `test_atomic_block_commit_success`: Verify all tables (Blocks, Txs, UTXOs, Tip) update in unison.
- `test_aborted_transaction_leaves_zero_state`: Simulate error during block commit; assert zero partial rows exist.
- `test_reorg_atomic_utxo_mutation`: Rollback 2 blocks and apply 3 new blocks; verify UTXO state matches new canonical branch.

### Restart & Crash Resilience Tests:
- `test_persistence_across_process_restart`: Write canonical state, close database instance, reopen, and assert tip, height, and UTXOs are identical.

---

## 8. Acceptance Criteria Checklist

Task 14 can only be marked as **VERIFIED** when:

- [x] `redb` storage engine is integrated into `scytale-storage`.
- [x] Domain crates (`scytale-core`) maintain zero direct dependencies on `redb`.
- [x] Table schemas for Blocks, Transactions, UTXOs, Index, and Chain State are implemented.
- [x] All-or-nothing atomic block commits are verified across multi-table writes.
- [x] Failed write transactions leave zero partial or corrupted canonical state.
- [x] Fast UTXO insertions, lookups, and deletions are operational.
- [x] Complete state durability across process restarts is proven.
- [x] Monetary accounting values are persisted strictly as integer `quanta`.
- [x] 100% of unit, atomicity, restart, and persistence test suites pass.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 9. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Storage engine and table schemas underway.
     │
     ├── If binary key codec or storage schema is blocked ──> [ BLOCKED ]
     │                                                              │
     │ <────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, atomicity, and restart tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 15 (Node Lifecycle).
```

- **Current Status:** **`VERIFIED`**

---

## 10. Dependency for Downstream Tasks

- **Task 15 (Node Lifecycle):** Opens the persistent storage database upon startup, executes state recovery, and orchestrates orderly shutdown commits.

---

## 11. Agent Operating Rules

1. Treat `docs/work/14-storage.md` as the authoritative work runbook.
2. Re-use domain primitives from Tasks 03–07; do not create duplicate block or transaction types.
3. Storage is strictly a persistence layer; never inject consensus or monetary decisions into storage code.
4. Guarantee all-or-nothing atomicity for all block state transitions using `redb::WriteTransaction`.
5. Adhere strictly to the definition of done and quality gates.

---

## 12. Cross-Specification References

- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Master storage specification.
- **[`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)**: Block data structures.
- **[`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)**: Transaction models.
- **[`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)**: UTXO state transitions.
- **[`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)**: Reorg atomic rollback requirements.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](../VALUE-PROVENANCE-SPEC.md)**: Provenance lineage tracking.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Storage threat model.
