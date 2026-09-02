# Task 15 — Node Lifecycle & Subsystem Orchestration

This document is the permanent **Task Execution Runbook** for Task 15: Node Lifecycle & Subsystem Orchestration. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's node runtime coordinator, strict startup/shutdown sequencing, state recovery, initial chain sync orchestration, and continuous background subsystem management.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 15
Task Name   : Node Lifecycle
Phase       : Runtime / Orchestration
Level       : HEAVY
Status      : VERIFIED
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Integer quanta supply rules.
- **Task 02 — Genesis Allocation:** Genesis Block 0 initial anchor.
- **Task 03 — Transaction:** Transaction validation pipelines.
- **Task 04 — UTXO:** State tracking and OutPoint resolution.
- **Task 05 — Authorization:** Input unlocking proofs.
- **Task 06 — Hashing / Serialization:** 32-byte BLAKE3 digests and codecs.
- **Task 07 — Block:** BlockHeader and Block structural invariants.
- **Task 08 — Proof-of-Work:** PoW verification.
- **Task 09 — Difficulty:** Dynamic retargeting calculation.
- **Task 10 — Chain Selection / Reorganization:** Heaviest chain fork choice and atomic reorg.
- **Task 11 — Mempool:** Transaction pool admission and eviction hooks.
- **Task 12 — Mining Lifecycle:** Autonomous mining daemon loop.
- **Task 13 — P2P Network:** Go transport daemon and message exchange.
- **Task 14 — Storage:** Embedded `redb` persistence and atomic commit transactions.

### Core Reference Specifications:
- [`docs/NODE-LIFECYCLE-SPEC.md`](../NODE-LIFECYCLE-SPEC.md)
- [`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)
- [`docs/MINING-LIFECYCLE-SPEC.md`](../MINING-LIFECYCLE-SPEC.md)
- [`docs/P2P-NETWORK-SPEC.md`](../P2P-NETWORK-SPEC.md)
- [`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)
- [`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective & Architectural Role

> **Task Goal:** *Implement the runtime orchestrator (`Node`) in `scytale-node`, coordinating the lifecycle of all modular subsystems (Storage, Consensus, UTXO, Mempool, P2P, and Mining) through deterministic state transitions (Startup $\rightarrow$ Recovery $\rightarrow$ Sync $\rightarrow$ Ready $\rightarrow$ Running $\rightarrow$ Shutdown) without ever altering underlying consensus rules.*

### Subsystem Ownership Matrix:
```text
┌─────────────────────────────────────────────────────────────────┐
│                    SCYTALE NODE RUNTIME                         │
│  - Configuration & Resource Ownership                           │
│  - Orderly Startup & Graceful Shutdown Orchestration           │
└────────────────────────────────┬────────────────────────────────┘
                                 │
        ┌──────────────┬─────────┴────┬──────────────┬────────────┐
        ▼              ▼              ▼              ▼            ▼
┌──────────────┐┌──────────────┐┌──────────────┐┌──────────┐┌──────────┐
│   Storage    ││  Consensus   ││   Mempool    ││ Go P2P   ││  Mining  │
│ (Persists)   ││ (Validates)  ││ (Local Pool) ││(Transport││ (PoW)    │
└──────────────┘└──────────────┘└──────────────┘└──────────┘└──────────┘
```

- **Fundamental Invariant:** *Node coordinates subsystems; consensus defines protocol validity.* The node runtime does not invent consensus rules, create artificial balances, or bypass validation boundaries.

---

## 3. Node State Machine & Lifecycle Transitions

```text
                  [ STARTUP COMMAND ]
                           │
                           ▼
                      STARTING
                           │
                           ▼
                     INITIALIZING (Config, Storage open)
                           │
                           ▼
                      RECOVERING (Integrity check, UTXO verification)
                           │
                           ▼
                       SYNCING (P2P connect, IBD catch-up)
                           │
                           ▼
                         READY (All subsystems healthy)
                           │
                           ▼
                        RUNNING (Event loop, Mining if enabled)
                           │
            ┌──────────────┴──────────────┐
            ▼                             ▼
       [ SIGINT/SIGTERM ]           [ Fatal Error ]
            │                             │
            ▼                             ▼
        STOPPING                        FAILED
            │                             │
            ▼                             ▼
         STOPPED                    [ Aborted ]
```

---

## 4. Deterministic Startup & Shutdown Sequencing

### 12-Step Startup Sequence:
1. Load runtime configuration (`data_dir`, network ID, mining flag, resource policies).
2. Open embedded `redb` storage instance (`scytale-storage`).
3. Load persisted canonical state (tip hash, height, cumulative work).
4. Verify local database integrity and recover active `UTXO_SET`.
5. Initialize consensus verification engine with active difficulty target.
6. Initialize in-memory `Mempool` instance.
7. Launch and bind Go P2P transport daemon.
8. Perform Initial Block Synchronization (IBD) with network peers.
9. Assert complete chain synchronization and reach `READY` state.
10. If mining enabled $\rightarrow$ initialize and launch background mining worker daemon.
11. Enter `RUNNING` steady state.
12. Expose local control and status inspection interfaces.

### Orderly Graceful Shutdown Sequence:
1. Receive shutdown signal (`SIGINT`, `SIGTERM`, or control RPC).
2. Transition state to `STOPPING` and stop accepting new external transactions.
3. Signal cancellation token to background mining worker and await loop exit.
4. Finish or abort in-flight block processing.
5. Persist final canonical state and flush in-memory mempool if configured.
6. Terminate Go P2P peer sessions and close network sockets.
7. Close `redb` database handles cleanly.
8. Transition to `STOPPED` state and exit process with code 0.

---

## 5. Subsystem Event Orchestration

During steady `RUNNING` operation, the node runtime orchestrates cross-subsystem event pipelines:

### 1. Incoming Transaction Pipeline:
```text
P2P Network ──> Decode ──> Mempool Admission ──> Notify Miner Template (if active)
```

### 2. Incoming Block Pipeline:
```text
P2P Network ──> Consensus Rule Validation ──> Chain Selection:
                 ├── Non-Canonical ──> Persist Side-Branch Block
                 └── Canonical Tip ──> Atomic Block Commit (Storage) ──>
                                       Update Active UTXO Set ──>
                                       Reconcile Mempool ──>
                                       Cancel Stale Miner Candidate ──>
                                       Rebuild Fresh Mining Template
```

### 3. Locally Solved Block Pipeline:
```text
Mining Worker ──> Solution Found ──> Full Local Pre-Validation ──>
                  Atomic Block Commit (Storage) ──> Broadcast via Go P2P
```

---

## 6. Zero-Balance Permissionless Verification

Scytale guarantees zero-balance bootstrapping:

$$\text{Initial Balance} = 0\text{ SCY}$$

- When a new user starts a fresh node, the node synchronizes from Genesis Block 0, enters `READY` with zero spendable UTXOs, and automatically begins mining if enabled.
- The node **never** injects synthetic tokens or requires prior balance to mine.

---

## 7. Node Configuration Parameters (`TBD`)

The following runtime configuration parameters remain designated as **TBD**:

| Configuration Parameter | Status | Description |
| :--- | :---: | :--- |
| **`CONFIG_FILE_FORMAT`** | `TBD` | Configuration file syntax (e.g. TOML vs. YAML). |
| **`SHUTDOWN_TIMEOUT_SECONDS`**| `TBD` | Maximum duration to await background worker termination. |
| **`DEGRADED_MODE_POLICY`** | `TBD` | Operational behavior when P2P is completely disconnected. |
| **`CONTROL_IPC_TRANSPORT`** | `TBD` | Administrative CLI control interface (e.g. Local RPC). |

---

## 8. Error Model & Degraded States

```rust
pub enum NodeError {
    StorageInitializationFailed(StorageError),
    ConsensusIntegrityViolation(String),
    P2PStartupFailed(String),
    ChainSyncTimeout,
    MempoolCorrupted,
    MiningWorkerPanic(String),
    ShutdownTimeoutExceeded,
}
```

- **Fault Isolation:** A network disconnection transitions the node to `DEGRADED` mode without corrupting the local canonical database or aborting local validation.

---

## 9. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 15 must fulfill the following test suites:

### Unit & Lifecycle Tests:
- `test_node_state_transitions`: Verify sequential progression from `STARTING` to `RUNNING` to `STOPPED`.
- `test_startup_sequence_order`: Assert storage opens before consensus and P2P initializes before sync.
- `test_graceful_shutdown_order`: Assert miner cancels before storage closes.

### Multi-Subsystem Integration Tests:
- `test_node_full_pipeline_block_arrival`: Inject external block via P2P; assert consensus accepts, storage commits, and mempool clears.
- `test_node_reorg_orchestration`: Trigger multi-block reorg; assert UTXO rollback commits and miner template resets.
- `test_zero_balance_mining_bootstrap`: Start fresh node with 0 SCY; assert background miner starts and earns first coinbase UTXO.

### Restart & Crash Recovery Reality Tests:
- `test_restart_preserves_canonical_continuity`: Run node, commit blocks, kill process, restart, and assert node resumes seamlessly at exact previous height and tip.

---

## 10. Acceptance Criteria Checklist

Task 15 can only be marked as **VERIFIED** when:

- [x] `Node` runtime orchestrator is implemented in `scytale-node`.
- [x] Strict startup sequence is enforced and tested.
- [x] Orderly graceful shutdown sequence with timeout safeguards is operational.
- [x] Storage, Consensus, UTXO, Mempool, P2P, and Mining are fully orchestrated.
- [x] Incoming transaction, block, and reorg event flows operate atomically.
- [x] Zero-balance node mining and coinbase balance recognition are proven.
- [x] Fault isolation prevents network failures from corrupting storage.
- [x] Deterministic restart recovery restores exact canonical ledger state.
- [x] Node runtime introduces zero artificial consensus or monetary rules.
- [x] 100% of lifecycle, integration, and restart test suites pass.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 11. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Node coordinator and event pipelines underway.
     │
     ├── If config schema or control IPC transport is blocked ──> [ BLOCKED ]
     │                                                                  │
     │ <────────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All lifecycle, integration, and reality tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 16 (Passbook Interface).
```

- **Current Status:** **`VERIFIED`**

---

## 12. Dependency for Downstream Tasks

- **Task 16 (Passbook / Wallet Interface):** Queries the `READY` node runtime to retrieve confirmed UTXO balances and submits authorized transactions.

---

## 13. Agent Operating Rules

1. Treat `docs/work/15-node-lifecycle.md` as the authoritative work runbook.
2. Coordinate subsystems via explicit interfaces; do not create monolithic cross-dependencies.
3. Node coordinates lifecycle; consensus defines validity; storage persists state.
4. Guarantee zero-balance user bootstrapping with zero artificial balances.
5. Adhere strictly to the definition of done and quality gates.

---

## 14. Cross-Specification References

- **[`docs/NODE-LIFECYCLE-SPEC.md`](../NODE-LIFECYCLE-SPEC.md)**: Master node lifecycle specification.
- **[`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)**: Storage layout.
- **[`docs/MINING-LIFECYCLE-SPEC.md`](../MINING-LIFECYCLE-SPEC.md)**: Mining daemon loop.
- **[`docs/P2P-NETWORK-SPEC.md`](../P2P-NETWORK-SPEC.md)**: P2P network protocol.
- **[`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)**: Mempool admission.
- **[`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)**: Chain selection and reorg.
- **[`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)**: Master consensus rules.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
