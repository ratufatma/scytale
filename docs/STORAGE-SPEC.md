# Scytale Storage Architecture Specification

This document defines the formal specification for the **Storage Architecture** in Scytale. It establishes the conceptual data model, persistence categories, atomic state transition invariants, and lifecycle operations using **`redb`** as the embedded storage engine.

---

## 1. Storage Purpose & Architectural Role

The Scytale storage layer is designed to fulfill the following core requirements:

- **Persistence:** Durable, crash-consistent on-disk storage of the canonical ledger and historical blockchain data.
- **Determinism:** Guarantees bit-for-bit identical state representation across all node instances executing identical consensus blocks.
- **High-Performance UTXO Lookups:** Provides rapid, indexed key-value resolution for validating transaction inputs.
- **Historical & Provenance Retrieval:** Enables efficient retrieval of blocks, transactions, and Value Provenance ancestry DAG paths.
- **Strict Atomicity:** Guarantees all-or-nothing database transactions across UTXO mutations and chain state updates.
- **Architectural Boundary:** Storage serves exclusively as the persistence substrate for protocol state; it never dictates or alters consensus rules.

> **Core Principle:** *Storage is the persistence layer for protocol state, not the source of consensus rules.*

---

## 2. Storage Engine: `redb`

Scytale standardizes on **`redb`** as its primary embedded storage engine:

| Property | Rationale |
| :--- | :--- |
| **Embedded Architecture** | Runs within the node process without requiring external database servers or network socket overhead. |
| **ACID Compliance** | Provides strict atomicity, consistency, isolation, and durability with full crash safety. |
| **MVCC Concurrency** | Supports non-blocking concurrent read transactions while preserving single-writer serializability. |
| **Type-Safe Table Interfaces** | Native Rust zero-copy serialization interfaces for structured keys and values. |

---

## 3. Storage Data Categories

To preserve modularity and enable scalable state management, Scytale partitions stored data into three distinct architectural categories:

```text
Scytale Storage Architecture
├── 1. Canonical State (Active Consensus Truth)
│   ├── UTXO Set (Spendable Outputs)
│   └── Chain State (Active Tip, Height, Cumulative Work)
│
├── 2. Historical Data (Immutable Chain History)
│   ├── Block Payloads
│   ├── Block Headers & Metadata
│   └── Confirmed Transactions
│
└── 3. Optional Derived Indexes (Query Acceleration)
    ├── Transaction-to-Block Lookups
    ├── Address/Locking-Condition Indexes (Passbook Acceleration)
    └── Ancestral Provenance Lineage Caches
```

### 3.1 Canonical State
The minimal, mandatory dataset required for a validating node to execute consensus state transitions. It reflects the exact spendable UTXO set and active chain tip at the latest confirmed block height.

### 3.2 Historical Data
The append-only record of historical blocks and transactions. Used for chain synchronization, reorganizations, auditing, and deep provenance traversals.

### 3.3 Optional Derived Indexes
Secondary lookup structures generated purely for query acceleration (such as Passbook balance aggregations or explorer lookups). **Derived indexes are never a source of truth** and can be deterministically rebuilt from historical data at any time.

---

## 4. Conceptual Table Layout

Scytale structures its underlying `redb` database into five primary conceptual tables:

```text
redb Database File (.redb)
├── UTXO_SET        : OutPoint -> TxOut + Metadata
├── BLOCKS          : BlockID -> Raw Canonical Block
├── BLOCK_INDEX     : BlockID -> Block Header & Consensus Metadata
├── TRANSACTIONS    : TxID -> Raw Canonical Transaction
└── CHAIN_STATE     : Key (Protocol Singleton) -> Active Tip State
```

*Note: Table names represent logical ownership and functionality; exact binary table naming conventions are implementation details.*

---

## 5. UTXO Set Storage (`UTXO_SET`)

The `UTXO_SET` stores the active, spendable outputs of the ledger:

```text
Key   : OutPoint (TxID : 32 bytes, OutputIndex : u32)
Value : TxOut (Value : u64 quanta, LockingCondition : Vec<u8>) + Provenance Metadata
```

### Invariants:
1. **Current Spendable View:** A record exists in `UTXO_SET` if and only if the output is currently unspent.
2. **Atomic Deletion upon Spending:** When a transaction spends an output, its corresponding `OutPoint` record is permanently removed from `UTXO_SET`.
3. **Atomic Insertion upon Creation:** Newly created outputs are inserted into `UTXO_SET` within the same database transaction.
4. **Aggregate Integer Value:** Storage records the aggregate integer `quanta` value per UTXO; it **never creates individual records per quantum**.

---

## 6. Transaction Storage (`TRANSACTIONS`)

The `TRANSACTIONS` table provides historical access to confirmed transactions:

```text
Key   : TxID (32-byte BLAKE3 Hash)
Value : Canonical Serialized Transaction Record
```

- **Role:** Preserves full transaction payloads (inputs, authorization proofs, outputs) for historical auditability and provenance resolution.
- **Separation from UTXO Set:** While the `UTXO_SET` mutates dynamically, `TRANSACTIONS` represents immutable historical data.
- Cross-References: [`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md) and [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md).

---

## 7. Block Storage (`BLOCKS`)

The `BLOCKS` table stores full canonical block payloads:

```text
Key   : BlockID (32-byte BLAKE3 Header Digest)
Value : Raw Canonical Serialized Block (Header + Transactions[])
```

- Provides raw block retrieval for P2P block relay, full node verification, and historical replay.
- Cross-Reference: [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md).

---

## 8. Block Index Storage (`BLOCK_INDEX`)

The `BLOCK_INDEX` table stores lightweight block header metadata for rapid chain traversal:

```text
Key   : BlockID (32 bytes)
Value : BlockHeaderRecord
        ├── Height (u64)
        ├── PreviousBlockHash (32 bytes)
        ├── Timestamp (u64)
        ├── DifficultyTarget (Consensus Target)
        ├── CumulativeChainWork (u256 / Work Metric)
        └── ValidationStatus (Flags)
```

- **Functions:** Fast ancestor traversal, fork comparison (heaviest chain evaluation), and block header synchronization without loading full block transaction vectors.

---

## 9. Chain State Storage (`CHAIN_STATE`)

The `CHAIN_STATE` table maintains the active singleton state of the canonical tip:

```text
Key   : TIP_KEY (Fixed Singleton Identifier)
Value : ActiveChainState
        ├── BestBlockID (32-byte Hash of Current Tip)
        ├── BestHeight (u64)
        ├── BestCumulativeWork (Cumulative Chain Work Metric)
        └── ConsensusEpochMetadata (Current Retarget State)
```

- **Single Source of Current Tip:** Represents the definitive point from which candidate blocks and new transactions are validated.

---

## 10. Atomic Block Commit & State Transition

Block execution executes an all-or-nothing atomic database transaction in `redb`:

```text
                   Incoming Candidate Block
                              ↓
              Consensus & Cryptographic Validation
                              ↓
             Begin redb Write Transaction (Tx_db)
                              ↓
           [ Delete Consumed OutPoints from UTXO_SET ]
           [ Insert Newly Created Outputs into UTXO_SET ]
           [ Insert Block Payload into BLOCKS ]
           [ Insert Header Record into BLOCK_INDEX ]
           [ Insert Transactions into TRANSACTIONS ]
           [ Update Active Tip in CHAIN_STATE ]
                              ↓
                       Commit Tx_db
                              │
          ┌───────────────────┴───────────────────┐
          ▼                                       ▼
       SUCCESS                                 FAILURE
(All tables updated)                      (Tx_db Rollback)
                                        (Zero Partial State)
```

### Atomicity Guarantees:
- **No Partial State:** The node will never persist a block payload without updating the UTXO set, nor mutate UTXOs without updating the chain tip.
- **Rollback on Error:** Any disk error or validation failure triggers an immediate transaction rollback, leaving the preceding canonical state completely pristine.

---

## 11. Provenance Storage Requirements

As specified in [`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md), storage must support deterministic backward traversal:

```text
OutPoint ──> TRANSACTIONS (Creating Tx) ──> BLOCK_INDEX (Block) ──> Ancestral OutPoints ...
```

- **Graph Reconstruction:** Lineage is traversed on-demand across existing primary tables without requiring dedicated graph databases or per-quantum tracking.
- **Integer Quanta Conservation:** Every stored amount is represented as an unsigned 64-bit integer (`u64`) in quanta ($1\text{ SCY} = 100,000,000\text{ quanta}$).

---

## 12. Genesis Allocation Storage Model

Genesis allocations defined in [`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md) and [`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md) are stored as standard initial UTXO outputs in Block 0:

- **Founder Allocation:** 15% / 6,300,000 SCY ($630,000,000,000,000\text{ quanta}$)
- **Development / Treasury:** 5% / 2,100,000 SCY ($210,000,000,000,000\text{ quanta}$)
- **Ecosystem / Community:** 5% / 2,100,000 SCY ($210,000,000,000,000\text{ quanta}$)
- **Mining Emission Reserve:** 75% / 31,500,000 SCY ($3,150,000,000,000,000\text{ quanta}$)

All genesis outputs materialize as verifiable records in `UTXO_SET` and `TRANSACTIONS` upon Block 0 initialization.

---

## 13. Current State vs. Historical Data vs. Derived Indexes

| Property | Canonical State (`UTXO_SET`, `CHAIN_STATE`) | Historical Data (`BLOCKS`, `TRANSACTIONS`) | Derived Indexes (Optional Caches) |
| :--- | :--- | :--- | :--- |
| **Role** | Current spendable truth. | Full ledger history. | Query acceleration. |
| **Mutability** | Mutable (deletions and insertions per block). | Append-only (immutable once confirmed). | Ephemeral / Rebuildable. |
| **Pruning Sensitivity** | Cannot be pruned. | Prunable in non-archival modes. | Recomputed from historical data. |
| **Source of Truth** | **Yes** (Consensus critical). | **Yes** (Historical truth). | **No** (Derived cache). |

---

## 14. Passbook Support Queries

The storage layer supports the read queries required by [`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md):
- **Balance Calculation:** Filter and aggregate active records in `UTXO_SET` matching user keys ($\sum \text{UTXO Values}$).
- **Journal History:** Read chronological transaction records from `TRANSACTIONS` and `BLOCK_INDEX`.
- **Provenance Inspector:** Follow `OutPoint` ancestry links backward to issuance blocks.
- **Decoupled Architecture:** Passbook balances are never stored as static records; they are computed dynamically on-demand from verified storage.

---

## 15. Crash Recovery & Consistency

Scytale storage guarantees crash-safe recovery upon node restart:

1. **Clean Startup:** The node opens `redb`, verifies database integrity, and reads the canonical tip from `CHAIN_STATE`.
2. **Uncommitted Transaction Rollback:** Any write transaction interrupted by an abrupt process termination or power loss is discarded by `redb`'s write-ahead journal.
3. **Corruption Detection:** If database corruption is detected, the node halts gracefully and provides deterministic reindex paths rather than operating on compromised data.

---

## 16. Concurrency Strategy

- **Single-Writer Mutex:** State transitions and block applications are serialized through a single writer lock.
- **Concurrent Readers:** Read-only queries (Passbook balance lookups, RPC calls, mempool validation checks) utilize concurrent, non-blocking `redb` read transactions.
- `Concurrent Writers Strategy: TBD` (Future parallel validation optimizations).
- `Read Concurrency Strategy: TBD` (Worker pool tuning).

---

## 17. Storage Lifecycle Operations

```text
Node Launch:
  1. Open redb storage environment at data directory path.
  2. Load and verify CHAIN_STATE singleton.
  3. Verify consistency between CHAIN_STATE tip and UTXO_SET.
  4. Signal storage readiness to Consensus and Mempool engines.

Node Graceful Shutdown:
  1. Stop accepting new state transitions.
  2. Complete or rollback active in-flight database transactions.
  3. Flush pending file buffers to durable disk storage.
  4. Safely close redb environment handle.
```

---

## 18. Open Questions & Pending Specifications

The following implementation parameters remain designated as **TBD**:

| Parameter | Status | Scope |
| :--- | :--- | :--- |
| **Exact `redb` Table Definitions** | `TBD` | Rust table types and typed key/value definitions in `scytale-storage`. |
| **Binary Key/Value Encodings** | `TBD` | Compact serialization schemas for OutPoints, TxOuts, and Headers. |
| **BlockID Derivation Formula** | `TBD` | Finalization of domain-separated BLAKE3 header digest layout. |
| **Secondary Index Suite** | `TBD` | Exact set of optional indexes deployed for RPC/Passbook acceleration. |
| **Pruning Policy Specification** | `TBD` | Formal rules for discarding ancient block payloads on pruned nodes. |
| **Reindex & Repair Procedures** | `TBD` | Standalone CLI tools for database recovery and index rebuilding. |
| **Storage Migration Strategy** | `TBD` | Schema versioning and migration mechanics for future protocol upgrades. |

---

## 19. Cross-Specification References

- **[`docs/ARCHITECTURE.md`](ARCHITECTURE.md)**: Modular workspace structure and crate dependency graph.
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: Master UTXO ledger model and state transitions.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Canonical transaction format and TxID calculation.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint indexing and UTXO lifecycle.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md)**: Value provenance lineage and DAG traversal.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block structure, coinbase positioning, and state transitions.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work threshold verification.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: Dynamic difficulty adjustment.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 42,000,000 SCY cap, 60s block target, and quanta accounting.
- **[`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md)**: 15/5/5/75 supply distribution breakdown.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block specification and zero-balance onboarding.
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 digests and canonical byte serialization.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: Presentation layer and dynamic balance derivation.
