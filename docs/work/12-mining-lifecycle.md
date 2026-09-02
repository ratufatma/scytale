# Task 12 — Automatic Mining Lifecycle

This document is the permanent **Task Execution Runbook** for Task 12: Automatic Mining Lifecycle. It defines the technical instructions for agents and engineers to design, implement, test, and verify Scytale's continuous background mining loop, candidate block template construction, stale work cancellation, pre-broadcast local validation, and zero-balance user bootstrapping.

---

## 1. Task Metadata & Positioning

```text
Task ID     : 12
Task Name   : Mining Lifecycle
Phase       : Runtime / Consensus Integration
Level       : HEAVY
Status      : VERIFIED
```

### Primary Dependencies:
- **Task 01 — Monetary Policy:** Emission schedule and integer quanta block subsidies.
- **Task 02 — Genesis Allocation:** Macro supply boundaries.
- **Task 03 — Transaction:** Transaction data structures and fee calculation.
- **Task 04 — UTXO:** Solvency checks and coinbase output creation.
- **Task 05 — Authorization:** Input unlocking verification.
- **Task 06 — Hashing / Serialization:** 32-byte BLAKE3 digests.
- **Task 07 — Block:** `Block`, `BlockHeader`, and coinbase isolation.
- **Task 08 — Proof-of-Work:** Computational target satisfaction ($\text{Hash} \le \text{Target}$).
- **Task 09 — Difficulty:** Active consensus difficulty target.
- **Task 10 — Chain Selection / Reorganization:** Canonical tip updates and reorg notifications.
- **Task 11 — Mempool:** Transaction selection and fee prioritization.

### Core Reference Specifications:
- [`docs/MINING-LIFECYCLE-SPEC.md`](../MINING-LIFECYCLE-SPEC.md)
- [`docs/POW-SPEC.md`](../POW-SPEC.md)
- [`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)
- [`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)
- [`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)
- [`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)
- [`docs/STORAGE-SPEC.md`](../STORAGE-SPEC.md)
- [`docs/TRANSACTION-SPEC.md`](../TRANSACTION-SPEC.md)
- [`docs/UTXO-SPEC.md`](../UTXO-SPEC.md)
- [`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)
- [`docs/CONSENSUS-SPEC.md`](../CONSENSUS-SPEC.md)
- [`docs/PROTOCOL-CONSTANTS.md`](../PROTOCOL-CONSTANTS.md)
- [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)
- [`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)

---

## 2. Objective & Architectural Role

> **Task Goal:** *Implement the continuous, autonomous background mining engine in `scytale-consensus` / `scytale-node`, enabling validating nodes to assemble candidate block templates from the active canonical tip and mempool, execute Proof-of-Work searches, instantly cancel stale templates upon state changes, and locally validate solutions before network broadcast.*

### Continuous Background Lifecycle Model:
```text
Node Reaches READY State
           │
           ▼
[ Start Background Mining Worker ]
           │
           ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        Continuous Mining Loop                          │
│                                                                        │
│   1. Read Canonical Tip + Difficulty Target + Mempool Transactions     │
│   2. Calculate Block Subsidy R(H) + Aggregate Included Fees            │
│   3. Construct Coinbase Transaction at index 0                         │
│   4. Assemble Candidate BlockHeader                                    │
│   5. Iterate Nonce & Hash with BLAKE3                                  │
│                                                                        │
│   ├── Solution Found ──> Local Pre-Validation ──> Submit/Broadcast     │
│   └── Tip Changed    ──> Cancel Work ──> Refresh ──> Rebuild Candidate │
└────────────────────────────────────────────────────────────────────────┘
```

- **Not a Manual Command:** Mining runs as a continuous background daemon, automatically transitioning to the next block height without manual user prompts.

---

## 3. Zero-Balance Permissionless Onboarding

Scytale strictly preserves the permissionless user bootstrap invariant:

$$\text{New User Initial Balance} = 0\text{ SCY}$$

```text
[ Fresh Node Downloaded ] ──> Balance = 0 SCY
                                    │
                                    ▼
                      [ Node Starts & Synchronizes ]
                                    │
                                    ▼
                     [ Automatic Mining Enabled ]
                                    │
                                    ▼
                     [ Miner Solves Block Height H ]
                                    │
                                    ▼
                [ Coinbase Commits 10 SCY to Miner UTXO ]
                                    │
                                    ▼
              [ Passbook Displays First Positive Balance ]
```

- Zero initial deposit, zero registration tokens, and zero prior balance are required to participate in mining.

---

## 4. Scope & Non-Goals

### In Scope:
- Candidate block template construction (`Tip` + `Target` + `Mempool` + `Coinbase`).
- Coinbase transaction generation with exact subsidy $R(h)$ and aggregated fees.
- Asynchronous mining worker loop with non-blocking cancellation signals.
- Invalidation and rebuilding of stale candidate templates upon new block ingress or reorg.
- Pre-broadcast local consensus validation.
- Unit, stale-work cancellation, restart, and zero-balance bootstrap test suites.

### Out of Scope / Non-Goals:
- Implementing hardware-specific ASIC/GPU mining drivers or multi-machine mining pool protocols (Stratum).
- Implementing low-level P2P socket multiplexers (deferred to Task 13 / P2P).
- Implementing persistent `redb` block storage (deferred to Task 14 / Storage).
- Designing user-facing wallet key management or Passbook UI components.

---

## 5. Work Items

### W1 — Inspect Existing Primitives
- Inspect `crates/scytale-consensus` and `crates/scytale-core`.
- Re-use `Block`, `BlockHeader`, `Transaction`, `TxIn`, `TxOut`, `OutPoint`, `Hash`, and `Target` directly.

### W2 — Implement Miner State Machine
- Manage miner lifecycle states:
  ```rust
  pub enum MinerState {
      Stopped,
      Starting,
      Ready,
      Mining { height: u64, candidate_hash: Hash },
      Refreshing,
      Stopping,
      Failed(String),
  }
  ```

### W3 — Candidate Block Construction
- Assemble block candidate templates:
  1. Fetch active canonical tip hash and height ($H+1$).
  2. Fetch expected difficulty target for height $H+1$.
  3. Select priority transactions from `Mempool`.
  4. Calculate block subsidy: $R(H+1) = \text{Initial Reward} / 2^{\lfloor (H+1) / 2,100,000 \rfloor}$.
  5. Sum transaction fees: $\sum \text{Fees} = \sum (\text{Inputs} - \text{Outputs})$.
  6. Create coinbase output: $\text{Value} \le R(H+1) + \sum \text{Fees}$.
  7. Compute transaction commitment root and assemble `BlockHeader`.

### W4 — Proof-of-Work Nonce Search Loop
- Iterate `header.nonce` across a designated range:
  $$\text{assert}(\text{BLAKE3}(\text{canonical\_bytes}(\text{header})) \le \text{difficulty\_target})$$
- Support non-blocking cancellation token polling on every iteration.

### W5 — Stale Candidate Cancellation (Critical Invariant)
- When a valid block arrives from the P2P network or a reorg is executed:
  1. Trigger immediate cancellation token on the running mining worker.
  2. Abandon the uncompleted candidate template (carries zero state).
  3. Reload new canonical tip state and rebuild a fresh candidate template for height $H+2$.
  4. Resume mining immediately.

### W6 — Pre-Broadcast Local Consensus Validation
- When a valid PoW nonce is discovered:
  - Run full local consensus validation (all 13 rules in `BLOCK-SPEC.md`) before broadcasting to the network or committing to storage.

---

## 6. Operational Policies (`TBD`)

The following mining configuration parameters remain designated as **TBD**:

| Operational Parameter | Status | Description |
| :--- | :---: | :--- |
| **`MINING_CONCURRENCY_WORKERS`** | `TBD` | Number of parallel worker threads allocated to hashing. |
| **`MINING_INTENSITY_THROTTLE`** | `TBD` | CPU throttling policy for background desktop usage. |
| **`CANDIDATE_REFRESH_INTERVAL`**| `TBD` | Frequency of rebuilding templates to capture high-fee mempool txs. |
| **`COINBASE_PAYOUT_ADDRESS`** | `TBD` | Format for specifying local miner payout destination. |

---

## 7. Error Model

Mining lifecycle operations return strongly-typed domain errors:

```rust
pub enum MiningError {
    PrerequisitesNotReady(String),
    StaleCandidateAborted { height: u64 },
    LocalValidationFailed(ConsensusError),
    MempoolUnavailable,
    StorageUnavailable,
    WorkerPanic(String),
}
```

---

## 8. Test Strategy & Verification Plan

In accordance with [`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md), implementation of Task 12 must fulfill the following test suites:

### Unit & Template Tests:
- `test_candidate_block_assembly`: Verify coinbase subsidy + fee aggregation in candidate.
- `test_coinbase_output_exactness`: Verify coinbase output equals exact block reward + fees.
- `test_zero_balance_mining_bootstrap`: Prove a node with zero spendable UTXOs constructs a valid candidate.

### Stale Work Cancellation Tests:
- `test_cancel_mining_on_new_block`: Simulate solving block concurrently with external block arrival; assert worker cancels immediately.
- `test_cancel_mining_on_reorg`: Simulate fork reorg; assert miner resets template to new canonical tip.

### End-to-End Test (Controlled Difficulty):
- Launch test miner on low difficulty ($2^{255}-1$), solve block, verify local pre-validation passes, and assert coinbase UTXO is recognized in canonical state.

---

## 9. Acceptance Criteria Checklist

Task 12 can only be marked as **VERIFIED** when:

- [x] Autonomous background mining lifecycle is implemented.
- [x] Candidate block templates are constructed from tip, difficulty target, and mempool.
- [x] Coinbase subsidy and fee calculations operate strictly in integer `quanta`.
- [x] Non-blocking cancellation tokens cleanly abort stale mining workers.
- [x] Mining automatically advances to next height upon block acceptance without manual prompts.
- [x] Pre-broadcast local consensus validation is enforced.
- [x] Zero-balance user bootstrapping is proven and tested.
- [x] Mining carries zero authoritative state of its own (decoupled from consensus truth).
- [x] 100% of unit, template, cancellation, and controlled-difficulty tests pass.
- [x] Workspace quality gates pass (`cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`).

---

## 10. Definition of Done & Task Status

```text
[ PLANNED ]     ──> Runbook defined and committed.
     │
     ▼
[ IN PROGRESS ] ──> Background worker and template assembly underway.
     │
     ├── If concurrency runtime or cancellation API is blocked ──> [ BLOCKED ]
     │                                                                   │
     │ <─────────────────────────────────────────────────────────────────┘
     ▼
[ VERIFIED ]    ──> All unit, cancellation, and lifecycle tests pass.
     │
     ▼
[ COMPLETE ]    ──> Signed off and ready for Task 13 (P2P Network).
```

- **Current Status:** **`VERIFIED`**

---

## 11. Dependency for Downstream Tasks

- **Task 13 (P2P Network):** Receives locally mined and validated blocks from Task 12 for broadcast to network peers.

---

## 12. Agent Operating Rules

1. Treat `docs/work/12-mining-lifecycle.md` as the authoritative work runbook.
2. Re-use primitives from Tasks 01–11; do not create duplicate block or transaction types.
3. Mining produces candidates; consensus decides validity; storage persists state.
4. Ensure zero-balance mining is fully supported with zero artificial balance requirements.
5. Adhere strictly to the definition of done and quality gates.

---

## 13. Cross-Specification References

- **[`docs/MINING-LIFECYCLE-SPEC.md`](../MINING-LIFECYCLE-SPEC.md)**: Master mining lifecycle specification.
- **[`docs/POW-SPEC.md`](../POW-SPEC.md)**: Proof-of-Work rules.
- **[`docs/BLOCK-SPEC.md`](../BLOCK-SPEC.md)**: Block and Coinbase specification.
- **[`docs/DIFFICULTY-SPEC.md`](../DIFFICULTY-SPEC.md)**: Difficulty target retargeting.
- **[`docs/MEMPOOL-SPEC.md`](../MEMPOOL-SPEC.md)**: Transaction pool selection.
- **[`docs/CHAIN-SELECTION-SPEC.md`](../CHAIN-SELECTION-SPEC.md)**: Heaviest chain rules.
- **[`docs/MONETARY-POLICY.md`](../MONETARY-POLICY.md)**: Monetary policy and emission curves.
- **[`docs/TESTING-STRATEGY.md`](../TESTING-STRATEGY.md)**: Testing strategy.
- **[`docs/SECURITY-THREAT-MODEL.md`](../SECURITY-THREAT-MODEL.md)**: Security threat model.
