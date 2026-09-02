# Scytale Chain Selection & Reorganization Specification

This document defines the formal specification for **Canonical Chain Selection, Fork Handling, and Chain Reorganization** in Scytale. It establishes the mathematical and procedural rules by which validating nodes independently and deterministically select the primary consensus branch from a Directed Acyclic Graph (DAG) of competing blocks.

---

## 1. Purpose & Core Objectives

In a decentralized peer-to-peer network, latency and concurrent block discoveries naturally generate competing branches (forks). The Chain Selection mechanism ensures:

- **Deterministic Convergence:** Every compliant node independently converges on the exact same canonical sequence of blocks given the same set of valid candidate headers.
- **Objective Fork Resolution:** Competing branches are evaluated using provable, unforgeable cumulative computational work rather than subjective metrics (such as arrival timestamps or block height).
- **Atomic Reorganization:** Re-routing the active ledger to a heavier valid branch is executed with strict all-or-nothing database atomicity.
- **UTXO & Mempool Coherence:** Unconfirmed transactions, confirmed inputs, and orphaned coinbase payouts are systematically synchronized during branch transitions.

> **Fundamental Axiom:** *The canonical chain is defined exclusively as the fully valid branch exhibiting the greatest cumulative Proof-of-Work.*

---

## 2. The Canonical Chain Invariant

Scytale evaluates chain authority based on accumulated thermodynamic work:

$$\text{Canonical Chain} = \arg\max_{C \in \mathcal{V}} \left( \sum_{B \in C} \text{Work}(B) \right)$$

where $\mathcal{V}$ is the set of all branches originating from the Scytale Genesis block whose blocks strictly satisfy 100% of consensus validation rules.

```text
Branch A (3 Blocks):
Genesis ──> A1 (Work: 10) ──> A2 (Work: 10) ──> A3 (Work: 10)
Total Cumulative Work: 30

Branch B (2 Blocks, Higher Difficulty Epoch):
Genesis ──> A1 (Work: 10) ──> B2 (Work: 25)
Total Cumulative Work: 35  ──> [ CANONICAL TIP ]
```

### Critical Distinction:
- **Heaviest Work vs. Longest Chain:** Canonical selection is **NOT** governed by simple block count. A shorter branch with higher cumulative work definitively supersedes a longer branch composed of lower-difficulty blocks.

---

## 3. Cumulative Chain Work Calculation

Cumulative chain work represents the scalar sum of expected Proof-of-Work hashes required to discover every block in that branch:

$$\text{ChainWork}(C) = \sum_{i=0}^{H} \text{Work}(\text{Block}_i)$$

$$\text{Work}(\text{Block}_i) \approx \frac{2^{256}}{\text{difficulty\_target}_i + 1}$$

- **Monotonic Progression:** Adding a valid block strictly increases a branch's cumulative work.
- **Specification Status:** `Chain Work Calculation: TBD` (Fixed-point integer representation and multi-precision `u256` scalar summation rules).

---

## 4. Absolute Validity Precedes Work Comparison

A branch cannot be considered for canonical selection unless **every individual block and state transition in that branch is completely valid**:

```text
                  Incoming Candidate Branch
                              ↓
              [ 1. Validate Header Schemas ]
                              ↓
              [ 2. Validate Parent Hash Linkages ]
                              ↓
              [ 3. Validate Difficulty Targets & Retargets ]
                              ↓
              [ 4. Verify Proof-of-Work Hashes <= Targets ]
                              ↓
              [ 5. Validate Transaction Authorization & Fees ]
                              ↓
              [ 6. Validate UTXO State Transitions & Lineage ]
                              ↓
                       Is Branch Valid?
             ├── NO  ──> REJECTED (Work is completely ignored)
             └── YES ──> Compare Cumulative Work against Active Tip
```

> **Consensus Invariant:** *An invalid block or state transition immediately disqualifies its entire descendant branch, regardless of the quantity of Proof-of-Work accumulated.*

---

## 5. Genesis Anchor Requirement

Every valid Scytale chain must have the official **Scytale Genesis Block (Height 0)** as its immutable root:

```text
Scytale Genesis (Block 0) ──> Block 1 ──> Block 2 ──> ... ──> Active Tip
```

- Any candidate chain referencing an alternative root hash or foreign genesis state belongs to an incompatible network and is rejected immediately.
- Cross-Reference: [`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md).

---

## 6. Canonical Chain Extension (Fast Path)

When an incoming valid block directly references the current canonical tip:

```text
[ Current Canonical Tip (Height H) ]  +  [ Valid New Block (Height H + 1) ]
                                  ↓
                  Verify Block & PoW against Active State
                                  ↓
               Apply Atomic UTXO Mutation in Storage
                                  ↓
                Advance Canonical Tip to (Height H + 1)
                                  ↓
                  Evict Confirmed Txs from Mempool
```

- This represents the standard, zero-reorganization progression of the ledger.

---

## 7. Competing Blocks & Fork Topologies

When two or more valid blocks share the same parent block, a fork topology is created:

```text
                            [ Common Parent P ]
                                     │
                     ┌───────────────┴───────────────┐
                     ▼                               ▼
               [ Block A ]                     [ Block B ]
          (Broadcast by Node 1)           (Broadcast by Node 2)
```

### Resolution Rules:
1. **Branch Tracking:** The node validates and persists both blocks in `BLOCKS` and `BLOCK_INDEX`.
2. **Active Selection:** The node maintains the active tip on the branch with the highest cumulative work.
3. **Equal Work Tie-Break:** If $\text{Work}(A) == \text{Work}(B)$, the node retains the branch that arrived first locally as the active tip, pending future block discovery.
4. **Specification Status:** `Equal Work Tie-Break: TBD`.

---

## 8. Chain Reorganization (Reorg) Mechanics

A **Chain Reorganization** occurs when an alternative valid branch accumulates strictly greater Proof-of-Work than the currently active branch:

```text
Step 1: Identify Fork Topology
                       [ Common Ancestor C ]
                                 │
                 ┌───────────────┴───────────────┐
                 ▼                               ▼
          [ Stale Branch A ]              [ New Heavy Branch B ]
           Block A1 (Work: 10)             Block B1 (Work: 20)
           Block A2 (Work: 10)             Block B2 (Work: 20)
         Total Work on A: 20             Total Work on B: 40
                                                 │
                                                 ▼
                                        TRIGGER REORGANIZATION
```

---

## 9. Reorganization Execution Pipeline

The reorganization pipeline transitions ledger state from the stale branch to the new heavy branch through an atomic multi-step procedure:

```text
               Reorganization Triggered (Work_B > Work_A)
                                  ↓
             [ 1. Locate Most Recent Common Ancestor C ]
                                  ↓
           [ 2. Begin redb Atomic Database Transaction ]
                                  ↓
         [ 3. Rollback Disconnected Branch A (A2 -> A1) ]
         ├── Undo UTXO state diffs back to Ancestor C
         └── Collect disconnected transactions for mempool re-evaluation
                                  ↓
          [ 4. Connect New Canonical Branch B (B1 -> B2) ]
         ├── Sequentially validate and apply UTXO state diffs
         └── Assert value conservation and no double-spends
                                  ↓
             [ 5. Update CHAIN_STATE Tip to Block B2 ]
                                  ↓
                      [ 6. Commit Database Tx ]
                                  ↓
      [ 7. Re-evaluate Mempool & Invalidate Stale Mining Worker ]
```

---

## 10. Common Ancestor Resolution

The node traverses block header references backward from both branch tips until discovering their latest shared intersection:

$$\text{CommonAncestor}(A_{\text{tip}}, B_{\text{tip}}) = \arg\max_{H} \{ B \in A_{\text{branch}} \cap B_{\text{branch}} \}$$

- All mutations are evaluated as state differentials relative to this common snapshot.

---

## 11. UTXO Set Rollback Invariants

During a reorganization:
1. **Revocation of Stale Outputs:** All UTXOs generated by transactions in the disconnected branch ($A_1, A_2$) are removed from the active `UTXO_SET`.
2. **Restoration of Consumed Inputs:** All UTXOs consumed by transactions in the disconnected branch are restored to unspent status in `UTXO_SET`.
3. **Application of New Outputs:** All outputs generated by transactions in the new branch ($B_1, B_2$) are inserted into `UTXO_SET`.
4. **Spendability Transition:** Only outputs created on the newly committed canonical branch ($B$) are spendable post-reorg.

---

## 12. Transaction Re-Admission to Mempool

Transactions confirmed in the disconnected branch that do not appear in the new canonical branch are re-evaluated for mempool inclusion:

```text
                      Disconnected Transaction Tx_k
                                    ↓
                 Is Tx_k already confirmed in Branch B?
                   ├── YES ──> Drop (Already confirmed)
                   └── NO  ──> Re-validate against new UTXO_SET
                                    ↓
                           Valid under New State?
                             ├── NO  ──> Discard / Expire
                             └── YES ──> Re-admit to Mempool (Pending)
```

- **Specification Status:** `Transaction Re-admission Policy: TBD`.

---

## 13. Coinbase Rollback & Monetary Integrity

- **Stale Subsidy Invalidation:** Coinbase transactions in the disconnected branch are permanently invalidated; their unspent outputs are pruned from `UTXO_SET`.
- **New Subsidy Activation:** Coinbase outputs from the new canonical branch are instantiated into `UTXO_SET`, subject to protocol emission limits ($10\text{ SCY} \dots$) and maturity delays.
- **Zero Supply Leakage:** Reorganizations cannot create unbacked supply or violate the 42,000,000 SCY maximum cap.

---

## 14. Orphan & Unconnected Block Handling

When a node receives a valid block whose parent block is currently unknown:

```text
Incoming Block (Parent Hash: 0x9e2a...)
                 ↓
      Parent Exists in Storage?
        ├── YES ──> Connect & Process Immediately
        └── NO  ──> Store in Ephemeral Unconnected Pool
                         ↓
             Request Missing Parent from Peer
                         ↓
             Parent Arrives & Validated
                         ↓
             Connect Orphaned Child Block
```

- **Specification Status:** `Orphan Retention Policy: TBD`, `Orphan Storage Limit: TBD`.

---

## 15. Valid vs. Canonical Distinction

Scytale strictly isolates validity from canonical standing:

| Status | Definition | Storage Representation |
| :--- | :--- | :--- |
| **`Valid`** | Conforms completely to header, PoW, and UTXO rules. | Persisted in `BLOCKS` and `BLOCK_INDEX`. |
| **`Canonical`** | Belongs to the valid branch with the greatest cumulative work. | Actively reflected in `UTXO_SET` and `CHAIN_STATE`. |
| **`Stale / Side Branch`** | Valid, but superseded by a heavier branch. | Retained in historical storage; excluded from active `UTXO_SET`. |

---

## 16. Interaction with Mining Subsystem

When a reorganization or chain extension advances the canonical tip:

```text
                  Canonical Tip Changes (Height H -> H')
                                    │
                                    ▼
                 Signal Autonomous Mining Controller
                                    │
                  Cancel Active PoW Worker on Stale Tip
                                    │
               Re-template Candidate Block on New Canonical Tip
                                    │
                    Resume Mining Loop at (H' + 1)
```

- Cross-Reference: [`docs/MINING-LIFECYCLE-SPEC.md`](MINING-LIFECYCLE-SPEC.md).

---

## 17. Interaction with Scytale Passbook

- **State Fidelity:** Passbook displays balances and transaction confirmations derived strictly from the active canonical tip.
- **Reorg UI Reflection:** If a transaction is displaced during a reorg, Passbook updates its status from `Confirmed` back to `Pending` (if re-admitted to mempool) or `Rejected` (if conflicted), adjusting the derived balance accordingly.
- Cross-Reference: [`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md).

---

## 18. Settlement Finality Model

- Scytale relies on **probabilistic Proof-of-Work settlement finality**.
- As additional valid blocks accumulate on top of a confirmed transaction, the cumulative work required for an adversarial branch to displace that transaction grows exponentially.
- **Specification Status:** `Finality Model: TBD` (Standard recommended confirmation depth for commercial settlement).

---

## 19. All-or-Nothing Reorganization Atomicity

A reorganization must execute as a single atomic transaction in `redb`:
- The node will **never** commit a new tip in `CHAIN_STATE` without completely applying the corresponding UTXO mutations in `UTXO_SET`.
- If an I/O or validation error occurs while applying the new branch, the database rolls back to the prior active tip cleanly without state corruption.

---

## 20. Open Questions & Pending Specifications

The following parameters and policies are designated as **TBD**:

| Parameter / Policy | Status | Scope |
| :--- | :--- | :--- |
| **Chain Work Mathematical Formula** | `TBD` | Exact multi-precision arithmetic representation for cumulative work. |
| **Equal Work Tie-Break Rule** | `TBD` | Formal tie-break criteria when two competing valid branches have identical work. |
| **Orphan Block Retention Policy** | `TBD` | Maximum memory/disk limits and timeout thresholds for unconnected blocks. |
| **Maximum Reorganization Depth Limit** | `TBD` | Optional consensus or node policy limits on deep historical reorganizations. |
| **Transaction Re-admission Criteria** | `TBD` | Mempool re-acceptance filtering algorithms post-reorg. |
| **Recommended Confirmation Depth** | `TBD` | UI/Passbook guidelines for high-value settlement finality. |

---

## 21. Cross-Specification References

- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block header structure, parent linkage, and 13 consensus validation checks.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: BLAKE3 Proof-of-Work evaluation and difficulty targets.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: Dynamic difficulty adjustment across epochs.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: `redb` atomic commit and table partition architecture.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint lifecycle and atomic state transitions.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Transaction validity and authorization rules.
- **[`docs/MEMPOOL-SPEC.md`](MEMPOOL-SPEC.md)**: Mempool re-admission and conflict resolution.
- **[`docs/MINING-LIFECYCLE-SPEC.md`](MINING-LIFECYCLE-SPEC.md)**: Autonomous miner invalidation upon tip advancement.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md)**: Ancestral value lineage across canonical blocks.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block root anchor.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: Maximum supply ceiling and coinbase emission invariants.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: Financial journal rendering and confirmation state tracking.
