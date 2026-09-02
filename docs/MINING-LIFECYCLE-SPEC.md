# Scytale Automatic Mining Lifecycle Specification

This document defines the formal specification for the **Automatic Mining Lifecycle** in Scytale. It establishes how continuous Proof-of-Work mining operates as an autonomous background subsystem managed by the node lifecycle, detailing candidate block construction, state synchronization, stale work invalidation, local pre-validation, and error recovery.

---

## 1. Core Architectural Principle

In Scytale, mining is structured as an **autonomous, continuous node lifecycle service**, rather than a manual, per-block invocation:

```text
               Scytale Node Launch
                        ↓
            Initialize Node Subsystems
                        ↓
         [ Automatic Mining Loop Started ]
                        ↓
        ┌────────────────────────────────┐
        │  1. Read Canonical Tip State   │
        │  2. Assemble Candidate Block   │
        │  3. Execute Proof-of-Work Loop │
        │  4. Solve Header Hash <= Target│
        │  5. Pre-Validate Locally       │
        │  6. Broadcast & Commit Block   │
        │  7. Advance to Next Height     │
        └───────────────┬────────────────┘
                        │
                        ▼
      [ Loop Recurs for Succeeding Blocks ]
```

### Key Lifecycle Principles:
- **Zero Manual Intervention:** Once enabled, the miner iterates blocks indefinitely as long as the node is active.
- **Dynamic State Tracking:** The miner continuously monitors the canonical blockchain tip and mempool inventory, adapting candidate templates in real time.
- **Pre-Broadcast Verification:** Solved candidates must pass full consensus verification locally before network propagation.

---

## 2. Miner Subsystem vs. Canonical State Boundary

The mining subsystem possesses **no independent canonical ledger state**:

> **Architectural Boundary:** *The miner is a computational worker that consumes verified state from Storage, Consensus, and Mempool to assemble speculative candidate blocks. It does not dictate canonical truth.*

```text
+-------------------------------------------------------------------------+
|                       Node Canonical Infrastructure                     |
|  - Storage Engine (redb) : UTXO Set & Chain State (Tip Height & Hash)   |
|  - Consensus Engine      : Target Calculation & Validation Invariants   |
|  - Mempool               : Valid Pending Transaction Queue              |
+------------------------------------+------------------------------------+
                                     │
                             (Read-Only State)
                                     │
                                     ▼
+-------------------------------------------------------------------------+
|                        Autonomous Mining Worker                         |
|  - Constructs Candidate Block Template                                  |
|  - Formulates Coinbase Output (Subsidy + Transaction Fees)              |
|  - Evaluates BLAKE3 Hashes against Target Threshold                     |
+-------------------------------------------------------------------------+
```

---

## 3. Node Startup & Initialization Pipeline

Upon launching the node, subsystems initialize sequentially:

```text
Node Launch Invocation
          ↓
[ 1. Open redb Storage Environment ]
          ↓
[ 2. Load Active Chain State & Verify UTXO Integrity ]
          ↓
[ 3. Initialize P2P Network Engine & Handshake with Peers ]
          ↓
[ 4. Initialize Local Mempool Subsystem ]
          ↓
[ 5. Initialize Autonomous Miner Controller ]
          ↓
[ 6. Construct Initial Candidate Block at Height (Tip + 1) ]
          ↓
[ 7. Spawn Mining Worker Loop ]
```

### Configuration Status:
- `Mining Enabled By Default: TBD` (Configurable via node startup flags / config file).
- `Mining Configuration Interface: TBD` (CLI arguments vs. configuration file directives).
- `Mining Startup Policy: TBD` (Immediate startup vs. post-sync startup).

---

## 4. Candidate Block Construction

To assemble a candidate block, the miner queries active node state:

```text
                      Canonical Chain Tip (Height H, Hash)
                                      +
                      Active Difficulty Target (T)
                                      +
                      Mempool Transaction Prioritization
                                      +
                      Computed Coinbase Transaction
                                      ↓
                         Candidate Block Template
             ├── Header
             │   ├── version                : Active Protocol Version
             │   ├── previous_block_hash    : Current Tip BlockID
             │   ├── transaction_commitment : Root of Selected Transactions
             │   ├── timestamp              : Current Unix Epoch Timestamp
             │   ├── difficulty_target      : Active Target (T)
             │   └── nonce                  : Initial Exploration Offset (0)
             │
             └── Transactions[]
                 ├── [0]  : Coinbase (Subsidy R(H+1) + Sum of Fees)
                 └── [1..]: Prioritized Mempool Transactions
```

- Cross-References: [`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md), [`docs/MEMPOOL-SPEC.md`](MEMPOOL-SPEC.md), and [`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md).

---

## 5. Coinbase Transaction Construction

The miner formulates the coinbase transaction at `index 0` according to strict consensus rules:

$$\text{Coinbase Output Value} \le R_{\text{quanta}}(\text{height}) + \sum_{k=1}^{N} \text{Fee}_{\text{quanta}}(Tx_k)$$

- **New Issuance:** Subsidy component $R(H+1)$ follows the deterministic halving curve ($10\text{ SCY} \rightarrow 5\text{ SCY} \dots$).
- **Fee Collection:** Consolidates fees from all included transactions into the miner's designated payout address.
- **Accounting Units:** Computed strictly in atomic integer `quanta` ($1\text{ SCY} = 100,000,000\text{ quanta}$).

---

## 6. Proof-of-Work Search Loop

Once a candidate header is constructed, the mining loop searches for a valid solution:

```text
               Candidate Block Header Template
                              ↓
              [ Modify Nonce / Mining Field ]
                              ↓
             Canonical Binary Serialization
                              ↓
                    BLAKE3 Header Hash
                              ↓
           Numeric(BLAKE3(Header)) <= Target?
             ├── NO  ──> Increment Nonce & Repeat Search Loop
             └── YES ──> Candidate Solved! Proceed to Pre-Validation
```

- **Hashing Primitive:** Pure **BLAKE3** 32-byte digest evaluation as defined in [`docs/POW-SPEC.md`](POW-SPEC.md).
- **Execution Model:** `Mining Worker Architecture: TBD` (CPU worker pools, SIMD acceleration, and core thread scheduling).

---

## 7. New Block Arrival & Stale Work Invalidation

When a competing block is broadcast across the P2P network and accepted as the new canonical tip:

```text
                   Peer Broadcasts New Valid Block B
                                   ↓
                  Node Consensus Validates & Accepts B
                                   ↓
                   Canonical Tip Advances to Height H+1
                                   │
                   ┌───────────────┴───────────────┐
                   ▼                               ▼
       [ Active Mining Candidate ]       [ Storage / Mempool ]
      Candidate is now STALE/OBSOLETE     State Updated to Height H+1
                   ↓                               ↓
       [ Cancel Mining Worker ]                    │
                   ↓                               │
       [ Refresh Canonical State ] <───────────────┘
                   ↓
   [ Construct Fresh Candidate at Height H+2 ]
                   ↓
       [ Resume PoW Search Loop ]
```

### Stale Work Invariant:
- **Zero Wasted Cycles on Stale State:** A miner must never continue computing Proof-of-Work over an obsolete `previous_block_hash`.
- `Cancellation Mechanism: TBD` (Atomic interruption flags / channel signaling between consensus and miner).

---

## 8. Mempool Updates & Template Refresh

When significant new transactions enter the mempool (e.g., transactions offering high fee density):

```text
High-Fee Transactions Enter Mempool
                 ↓
Node Evaluates Fee Delta against Active Candidate
                 ↓
Template Refresh Triggered?
  ├── NO  ──> Continue searching current candidate nonce space
  └── YES ──> Update transaction set, re-compute Coinbase Fee sum,
              update transaction commitment, and reset nonce search
```

- `Candidate Refresh Policy: TBD` (Interval-based polling vs. event-driven fee threshold recalculations).

---

## 9. Block Found & Pre-Broadcast Local Validation

When the mining loop discovers a hash satisfying the difficulty target:

```text
                     Proof-of-Work Solved!
                               ↓
                 Assemble Complete Block Struct
                               ↓
         [ Comprehensive Local Consensus Validation Pipeline ]
         ├── 1. Header schema and BLAKE3 hash check
         ├── 2. Proof-of-Work <= difficulty_target verification
         ├── 3. Parent block link matches active tip
         ├── 4. Transaction commitment matches transaction vector
         ├── 5. Coinbase transaction positioned at index 0
         ├── 6. Coinbase value <= Subsidy + Total Fees
         ├── 7. All transactions valid against current UTXO_SET
         └── 8. No intra-block double spends
                               ↓
                         All Passed?
             ├── NO  ──> Discard Candidate, Log Error, Re-template
             └── YES ──> Proceed to Local Commit & Network Broadcast
```

> **Mandatory Rule:** *A node must never broadcast a mined block to the P2P network without first executing complete, local consensus validation.*

---

## 10. Successful Block Acceptance & Next Cycle

Following successful pre-validation:

```text
                      Locally Valid Block
                               │
               ┌───────────────┴───────────────┐
               ▼                               ▼
      [ Commit to Storage ]           [ Broadcast to P2P ]
    - Atomically apply UTXO diffs    - Transmit block to peers
    - Update CHAIN_STATE tip         - Advance network state
    - Evict confirmed mempool txs
               │                               │
               └───────────────┬───────────────┘
                               ▼
            Advance Miner to Height (H + 2)
                               ↓
             Assemble Next Candidate Block
                               ↓
               Resume Autonomous Mining Loop
```

---

## 11. Competing Block Race Conditions

In the event that the local miner solves a block concurrently with the arrival of a peer block at the same height:

- **Resolution Mechanism:** `Competing Block Handling: Governed by heaviest chain consensus rules`.
- The node retains both valid candidate branches in storage, continuing mining on the active tip until the tie is resolved by the next cumulative work extension.

---

## 12. Miner State Machine

```text
           ┌──────────┐
           │ Stopped  │
           └────┬─────┘
                │ Node Launch / Start Command
                ▼
           ┌──────────┐
           │ Starting │
           └────┬─────┘
                │ State Loaded & Candidate Built
                ▼
    ┌───>  ┌──────────┐
    │      │  Mining  │ <─────────────────────┐
    │      └────┬─────┘                       │
    │           │                             │
    │           ├── Stale Tip / Mempool Delta │
    │           ▼                             │
    │      ┌────────────┐                     │
    │      │ Refreshing │ ────────────────────┘
    │      └────────────┘
    │           │
    │           │ Node Shutdown Requested
    │           ▼
    │      ┌──────────┐
    └───── │ Stopping │
           └────┬─────┘
                │ Workers Terminated
                ▼
           ┌──────────┐
           │ Stopped  │
           └──────────┘
```

---

## 13. Node Shutdown & Restart Mechanics

### 13.1 Graceful Shutdown:
```text
Shutdown Signal (SIGINT / SIGTERM / CLI)
                ↓
Signal Mining Loop to Halt Search
                ↓
Abandon In-Flight Unsolved Candidate Template
(Unsolved candidates carry zero state and are dropped safely)
                ↓
Flush Storage & Safely Close redb Environment
                ↓
Node Exits Cleanly
```

### 13.2 Node Restart:
- Upon restart, the node loads canonical state from `redb`, re-populates the mempool, constructs a brand-new candidate block, and resumes mining.
- `Mining Candidate Persistence: Not required` (Candidate templates are transient).

---

## 14. Zero-Balance User Integration

In accordance with [`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md):

$$\text{New User Initial Balance} = 0\text{ SCY} \quad (0\text{ quanta})$$

```text
Fresh Installation (0 SCY)
           ↓
Launch Scytale Node with Mining Enabled
           ↓
Autonomous Miner Operates Permissionlessly (Zero Fees / Zero Deposit Required)
           ↓
First Solved Block Connected to Ledger
           ↓
Coinbase Output (10 SCY) Recorded in Active UTXO Set
           ↓
Passbook Dynamically Renders First Non-Zero Balance
```

- Mining requires **zero prior token ownership**, eliminating economic barriers to network validation.

---

## 15. Operational Resource Controls

To ensure mining does not starve host system resources:

| Control Parameter | Status | Scope |
| :--- | :--- | :--- |
| **`CPU Resource Policy`** | `TBD` | Maximum CPU thread allocation / core affinity. |
| **`Mining Limit`** | `TBD` | Optional throttling for developer / low-power nodes. |
| **`Operational Pause Policy`** | `TBD` | Temporary pause during initial historical block download (IBD). |
| **`Block Publication Policy`** | `TBD` | Propagation strategies across connected P2P peers. |

---

## 16. Observability & Node Status Signals

A compliant node should expose standard status metrics for local monitoring:
- **`Mining Status`:** `Active`, `Refreshing`, `Idle`, or `Disabled`.
- **`Target Height`:** Block height of the candidate currently being mined.
- **`Difficulty Target`:** Current Proof-of-Work threshold.
- **`Hash Rate Metric`:** Estimated local hashes per second (BLAKE3).
- **`Mined Statistics`:** Total blocks solved, accepted, and rejected.

---

## 17. Open Questions & Pending Specifications

The following parameters remain designated as **TBD**:

| Parameter / Policy | Status | Scope |
| :--- | :--- | :--- |
| **Mining Enabled By Default** | `TBD` | Default operational state on fresh node initialization. |
| **Cancellation Signaling Primitive** | `TBD` | Atomic boolean flags vs. async broadcast channels. |
| **Candidate Refresh Frequency** | `TBD` | Maximum time interval before updating candidate block header timestamps. |
| **CPU Worker Architecture** | `TBD` | Thread allocation and parallel noncing strategies. |
| **Initial Block Download (IBD) Pause** | `TBD` | Automatic suppression of miner worker during historical chain synchronization. |

---

## 18. Cross-Specification References

- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block header schema, coinbase constraints, and 13 consensus checks.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: BLAKE3 target evaluation and difficulty mechanics.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: 60-second block cadence retargeting.
- **[`docs/MEMPOOL-SPEC.md`](MEMPOOL-SPEC.md)**: Pending transaction selection and fee signals.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: Atomic commit of mined blocks to `redb`.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md)**: Coinbase output lineage and auditability.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Zero-balance user bootstrapping through mining.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Emission curve, block subsidies, and quanta accounting.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: Dynamic display of confirmed mining payouts.
