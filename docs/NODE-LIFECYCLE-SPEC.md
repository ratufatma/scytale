# Scytale Node Lifecycle Specification

This document defines the formal specification for the **Node Lifecycle and Runtime Orchestration** in Scytale. It establishes the sequential initialization pipeline, subsystem dependency graph, readiness criteria, event-driven runtime coordination, failure handling, graceful shutdown, and crash recovery procedures for a full validating Scytale node.

---

## 1. Purpose & Orchestration Role

The **Node Lifecycle** acts as the supreme runtime orchestrator of the Scytale daemon. It unifies all independent crates and subsystems into a coherent, thread-safe, deterministic execution environment:

```text
                                SCYTALE NODE
                                     │
                    ┌────────────────┼────────────────┐
                    │                │                │
                 Storage         Consensus           P2P
                 (redb)        (PoW / Chain)         (Go)
                    │                │                │
                    └────────────────┼────────────────┘
                                     │
                                  Mempool
                                     │
                               Autonomous Miner
                                     │
                             Passbook Services
```

### Core Orchestrator Responsibilities:
- **Sequential Dependency Bootstrapping:** Ensures low-level storage engines and consensus states are verified before high-level networking or mining workers initialize.
- **Unified Event Coordination:** Routes network blocks, peer transactions, local mempool mutations, and miner events without duplicating canonical state authority.
- **Deterministic State Isolation:** Shields the canonical ledger from unverified network inputs and transient mining templates.
- **Graceful Teardown & Recovery:** Guarantees that node termination leaves on-disk `redb` state 100% consistent and crash-resilient.

> **Foundational Axiom:** *The Node Lifecycle coordinates and executes subsystems; it never creates or overrides consensus rules.*

---

## 2. Architectural Data Ownership & Single Source of Truth

To prevent synchronization drift and race conditions, Scytale enforces strict ownership boundaries:

| Subsystem | Primary Responsibility | State Authority |
| :--- | :--- | :--- |
| **`Storage Engine (redb)`** | Durably persists canonical chain state, UTXO set, blocks, and indexes. | **Source of Truth** for committed state. |
| **`Consensus Engine`** | Validates headers, PoW, transactions, and state transitions. | **Rule Authority** for validity checks. |
| **`Mempool`** | Manages local, ephemeral unconfirmed transaction queues. | **Pending State** only (Non-canonical). |
| **`P2P Subsystem (Go)`** | Discovers peers, manages sockets, and transports wire messages. | **Transport Conduit** only. |
| **`Autonomous Miner`** | Iterates nonce search over candidate block templates. | **Transient Worker** (Non-canonical). |
| **`Node Orchestrator`** | Manages lifecycles, event flows, and subsystem coordination. | **Coordinator** (Zero consensus authority). |

> **Invariance Rule:** *No subsystem may maintain an independent copy of ledger state that claims canonical standing over the verified storage engine.*

---

## 3. High-Level Node State Machine

The node operates across seven conceptual runtime states:

```text
              [ STARTING ]
                   │
                   ▼ (Storage Loaded & Subsystems Initialized)
              [ READY ]
                   │
                   ▼ (Sync Completed / Mining Enabled)
              [ RUNNING ] <────────────────────────┐
                   │                               │
                   ├── Network Partition / I/O lag │ Reconnected
                   ▼                               │
              [ DEGRADED ] ────────────────────────┘
                   │
                   │ Fatal Storage Corruption / Panic
                   ▼
              [ FAILED ]
                   │
                   ▼ Shutdown Signal Received
              [ STOPPING ]
                   │
                   ▼ Subsystems Cleanly Flushed & Closed
              [ STOPPED ]
```

---

## 4. Sequential Startup Pipeline

To guarantee deterministic bootstrapping, subsystems initialize in a strict, linear dependency order:

```text
                             Node Start Invocation
                                       ↓
1. [ Configuration Loader ]    ──> Parse CLI flags & runtime config files
                                       ↓
2. [ Storage Subsystem ]       ──> Open redb environment & verify table handles
                                       ↓
3. [ Canonical State Loader ]  ──> Read CHAIN_STATE tip, height, and cumulative work
                                       ↓
4. [ Consensus Validator ]     ──> Initialize target verification & difficulty tables
                                       ↓
5. [ UTXO State Loader ]       ──> Verify UTXO_SET consistency against tip block
                                       ↓
6. [ Mempool Subsystem ]       ──> Initialize pending queue (or re-admit persisted txs)
                                       ↓
7. [ P2P Networking Daemon ]   ──> Spawn Go networking layer & bind listening ports
                                       ↓
8. [ Initial Chain Sync (IBD)] ──> Discover peer tips, download & apply missing blocks
                                       ↓
9. [ Autonomous Miner ]        ──> (If enabled) Construct first candidate block
                                       ↓
10. [ Transition to READY ]    ──> Open external RPC / Passbook query endpoints
```

### Mining Guard Invariant:
> **Strict Startup Rule:** *The autonomous miner must NEVER begin Proof-of-Work hashing against an uninitialized, unverified, or unsynchronized chain state.*

---

## 5. Subsystem Initialization Contracts

### 5.1 Storage Initialization (`docs/STORAGE-SPEC.md`)
- Opens database environment at configured `data_dir`.
- Validates binary file headers and checks for write-ahead journal recovery.
- If database corruption is unrecoverable, the node halts immediately and transitions to `FAILED`.

### 5.2 Consensus Initialization (`docs/BLOCK-SPEC.md`, `docs/POW-SPEC.md`, `docs/DIFFICULTY-SPEC.md`)
- Loads the active canonical tip hash and height.
- Computes active difficulty target and epoch boundaries.
- Rejects any configuration attempting to override consensus parameters from unvalidated remote sources.

### 5.3 Mempool Initialization (`docs/MEMPOOL-SPEC.md`)
- Instantiates ephemeral pending transaction cache.
- If persistence is enabled, reads candidate records from disk, re-validates each against the current `UTXO_SET`, and drops any conflicting or obsolete spends.

### 5.4 P2P Initialization (`docs/P2P-NETWORK-SPEC.md`)
- Launches Go P2P subsystem over the IPC runtime boundary.
- Initiates peer discovery via configured seeds/bootstrap nodes and executes network handshakes.

---

## 6. Initial Chain Synchronization (IBD) Workflow

For newly joined nodes or nodes restarting after extended downtime:

```text
                     Node Enters Synchronization Phase
                                    ↓
       [ Query Connected Peers for Highest Declared Cumulative Work ]
                                    ↓
            [ Formulate Logarithmic Chain Locator from Local Tip ]
                                    ↓
            [ Download & Verify Sequential Headers from Heaviest Peer ]
                                    ↓
                 [ Stream & Apply Missing Block Payloads ]
                                    ↓
      [ Execute Atomic State Transitions (UTXO Deletions & Insertions) ]
                                    ↓
           [ Local Tip Matches Heaviest Valid Network Tip ]
                                    ↓
                      [ Transition to READY State ]
```

---

## 7. Node Readiness Criteria

A Scytale node transitions to the `READY` state when all foundational criteria are satisfied:
1. The `redb` storage layer is active and verified consistent.
2. Canonical `CHAIN_STATE` and `UTXO_SET` match the latest local block.
3. Consensus parameters are initialized and bound to the verified tip.
4. Peer networking is operational (or explicitly bypassed in standalone/devnet mode).
5. Initial chain synchronization has caught up with active network work.
- `Readiness Policy: TBD` (Configurable strict sync thresholds vs. devnet instant-ready rules).

---

## 8. Running State & Event-Driven Orchestration

Once `READY`, the node enters its continuous `RUNNING` event loop:

```text
                               RUNNING EVENT BUS
                                       │
         ┌──────────────────┬──────────┴──────────┬──────────────────┐
         ▼                  ▼                     ▼                  ▼
  [ New Tx Ingress ] [ New Block Ingress ] [ Competing Fork ] [ Miner Solution ]
         │                  │                     │                  │
         ▼                  ▼                     ▼                  ▼
Validate & Admit    Validate Consensus &  Execute Atomic     Pre-Validate Locally,
 to Mempool Queue     Apply Block Diff      Reorganization     Commit & Broadcast
```

- `Runtime Coordination Model: TBD` (Actor model, async channels, or event multiplexer).

---

## 9. Transaction & Block Ingress Workflows

### 9.1 Transaction Ingress:
```text
P2P Announcement ──> Request Tx ──> Validate Auth & UTXO ──> Admit to Mempool ──> Notify Miner
```
- Transactions admitted to the mempool **do not alter the canonical UTXO set**.

### 9.2 Block Ingress:
```text
P2P Block Arrival ──> Consensus Pre-Checks ──> Cumulative Work Evaluation ──> Canonical Tip?
                                                                                   │
                               ┌───────────────────────────────────────────────────┤
                               ▼                                                   ▼
                       [ NO: Side Branch ]                                [ YES: Extend Tip ]
                     Store in BLOCKS Table                               Commit Atomic State Diff
                                                                         Update UTXO_SET & Tip
                                                                         Evict Mempool Confirmed Txs
                                                                         Invalidate Stale Miner
                                                                         Spawn Next Block Template
```

---

## 10. Reorganization Orchestration

When a competing valid branch surpasses the cumulative Proof-of-Work of the active chain:

```text
Heavier Competing Branch Detected
                ↓
Traverse Backward to Locate Common Ancestor
                ↓
Execute Atomic redb Reorganization Transaction:
├── Rollback UTXO mutations on disconnected branch
├── Apply UTXO mutations from new heavy branch
└── Advance CHAIN_STATE tip to new branch header
                ↓
Re-evaluate Disconnected Transactions for Mempool Re-admission
                ↓
Cancel Stale Miner Worker & Construct Candidate Block at (New Tip + 1)
                ↓
Resume Autonomous Mining Loop
```

- Cross-References: [`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md) and [`docs/MINING-LIFECYCLE-SPEC.md`](MINING-LIFECYCLE-SPEC.md).

---

## 11. Zero-Balance Onboarding Experience

Scytale guarantees zero economic barrier to entry for new node operators:

$$\text{New User Initial Balance} = 0\text{ SCY} \quad (0\text{ quanta})$$

```text
1. User installs and launches Scytale Node.
2. Initial wallet / passbook balance reads 0 SCY.
3. Node synchronizes historical blocks seamlessly without deposit or stake.
4. Autonomous mining activates permissionlessly (0 SCY required).
5. First mined block solves PoW and commits coinbase subsidy (10 SCY) to local UTXO set.
6. Passbook instantly renders positive spendable balance with full Value Provenance.
```

- Cross-References: [`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md), [`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md), and [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md).

---

## 12. Graceful Shutdown Lifecycle

When a shutdown signal (`SIGINT`, `SIGTERM`, or CLI stop) is received:

```text
                         Shutdown Signal Received
                                    ↓
1. [ Transition to STOPPING State ] ──> Reject new RPC and network ingress
                                    ↓
2. [ Miner Worker Teardown ]        ──> Signal miner loop to halt; abandon in-flight template
                                    ↓
3. [ Mempool Flush ]                ──> (Optional) Persist pending transaction queue to disk
                                    ↓
4. [ P2P Teardown ]                 ──> Broadcast disconnect notices & close network sockets
                                    ↓
5. [ Storage Flush & Close ]        ──> Flush dirty buffers to disk & close redb handles
                                    ↓
6. [ Transition to STOPPED State ]  ──> Process exits with code 0
```

- `Graceful Shutdown Timeout: TBD` (Maximum duration allowed before forced thread termination).

---

## 13. Node Restart & Recovery

Upon process restart following a clean shutdown or abrupt termination:
1. The node opens `redb` and verifies the last committed atomic transaction.
2. If uncommitted partial writes occurred due to power loss, `redb` rolls back to the previous durable snapshot.
3. The node verifies that `CHAIN_STATE` matches the `UTXO_SET` tip.
4. Subsystems initialize sequentially, synchronize missing peer blocks, and resume active operation without data loss.

---

## 14. Degraded & Failure States

- **`DEGRADED` State:** Triggered when external network peers drop or remote services become unavailable. The node remains fully operational locally, preserving read/write access to confirmed ledger state and allowing local Passbook queries.
- **`FAILED` State:** Triggered by irrecoverable storage disk corruption, out-of-memory crashes, or invalid consensus states. The node terminates runtime loops immediately to prevent writing corrupted state to disk.
- `Degraded Mode Rules: TBD`, `Offline Mining Policy: TBD`.

---

## 15. Relationship with Scytale Passbook

- **Read-Only Consumer:** Passbook interfaces interact with the node via high-level query APIs.
- **Decoupled Architecture:** Passbook never acts as the node orchestrator and never mutates ledger state directly; it displays deterministic views derived from active `UTXO_SET` and historical transaction tables.

---

## 16. Observability & Runtime Status Signals

A running node exposes unified status telemetry:
- **`Node State`:** `Starting`, `Ready`, `Running`, `Degraded`, `Stopping`, `Stopped`, or `Failed`.
- **`Chain Telemetry`:** Best Block Hash, Best Height, Cumulative Work Metric, Target Difficulty.
- **`Network Telemetry`:** Connected Inbound/Outbound Peer Count, Sync Status, Bandwidth Rates.
- **`Mempool Telemetry`:** Pending Transaction Count, Total Fee Density, Memory Usage.
- **`Mining Telemetry`:** Miner Active Flag, Current Candidate Height, Local Hash Rate.
- **`Storage Telemetry`:** Disk Size, Table Record Counts, Commit Latency.

---

## 17. Security & Untrusted Input Boundary

Every external byte arriving via P2P sockets, RPC calls, or file imports is treated as **untrusted**:

```text
External Input ──> Binary Deserializer ──> Structural Check ──> Cryptographic Validation ──> Ledger State
```

- Under no circumstances does raw, unvalidated network data bypass consensus validation to mutate local storage.

---

## 18. Open Questions & Pending Specifications

The following implementation parameters remain designated as **TBD**:

| Parameter / Policy | Status | Scope |
| :--- | :--- | :--- |
| **Readiness Policy** | `TBD` | Synchronization threshold defining when node moves from `STARTING` to `READY`. |
| **Runtime Coordination Model** | `TBD` | Concurrency pattern (Tokio tasks, actor channels, or OS thread workers). |
| **Configuration File Format** | `TBD` | Format for runtime settings (TOML, YAML, or CLI flags). |
| **Graceful Shutdown Timeout** | `TBD` | Maximum duration before forced worker termination during shutdown. |
| **Failure Recovery Policy** | `TBD` | Automatic repair procedures upon detecting corrupted local secondary indexes. |
| **Degraded Mode Operation Rules** | `TBD` | Functional capabilities permitted during network partitions. |
| **Offline Mining Policy** | `TBD` | Consensus rules for local developer mining without active peers. |
| **Control Interface / Admin API** | `TBD` | RPC or socket interface for querying node telemetry and triggering shutdown. |

---

## 19. Cross-Specification References

- **[`docs/ARCHITECTURE.md`](ARCHITECTURE.md)**: System-level crate and workspace hierarchy.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: `redb` database lifecycle and atomic transaction guarantees.
- **[`docs/P2P-NETWORK-SPEC.md`](P2P-NETWORK-SPEC.md)**: Go networking subsystem and IPC message boundary.
- **[`docs/MEMPOOL-SPEC.md`](MEMPOOL-SPEC.md)**: Memory pool admission and eviction rules.
- **[`docs/MINING-LIFECYCLE-SPEC.md`](MINING-LIFECYCLE-SPEC.md)**: Autonomous miner worker lifecycle and candidate refreshing.
- **[`docs/CHAIN-SELECTION-SPEC.md`](CHAIN-SELECTION-SPEC.md)**: Canonical chain determination and reorg pipeline.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: 13 consensus validation invariants.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work threshold evaluation.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: Target difficulty adjustment.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Transaction structure and authorization rules.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint lifecycle and solvency verification.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md)**: Value lineage and DAG traversal.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: User-facing financial presentation layer.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block root anchor and zero-balance onboarding.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Fixed maximum supply and emission schedule.
