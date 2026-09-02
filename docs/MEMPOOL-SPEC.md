# Scytale Mempool Specification

This document defines the formal specification for the **Mempool (Memory Pool)** in Scytale. It establishes the rules for preliminary transaction admission, pending-state tracking, double-spend conflict resolution, fee prioritization signals, and lifecycle synchronization with blockchain blocks.

---

## 1. Purpose & Role of the Mempool

In Scytale, the **Mempool** is a node-local staging area for unconfirmed transactions:

```text
Incoming Transaction (P2P / Local RPC)
                ↓
    Preliminary Admission Validation
                ↓
       Mempool (Pending State)
                ↓
 Miner Block Template Selection
                ↓
    Candidate Block Assembly
                ↓
  Proof-of-Work & Consensus Validation
                ↓
 Canonical Block Committed to Ledger
```

### Core Functions:
1. **Transaction Staging:** Holds valid, unconfirmed transactions awaiting inclusion in a candidate block.
2. **Relay Filtering:** Acts as a network shield, verifying basic cryptographic validity and solvency before propagating transactions to peers.
3. **Miner Market Signal:** Provides a prioritized catalog of pending fee-density opportunities for block assembly.

---

## 2. Mempool is Local Pending State, NOT Canonical State

Scytale strictly distinguishes the mempool from the blockchain ledger:

> **Fundamental Principle:** *The mempool is a local node resource and is NOT part of the canonical blockchain state.*

```text
Node A Mempool  ≠  Node B Mempool  ──>  NORMAL (Local network latency / policies)
Node A Tip State == Node B Tip State ──>  MANDATORY (Consensus state determinism)
```

- **No Consensus Divergence:** Differences in pending transaction sets between nodes do not constitute a consensus fork.
- **Dynamic & Ephemeral:** Mempool contents may fluctuate based on local arrival times, network topology, and node eviction policies.

---

## 3. Transaction Admission Criteria

Before admitting any transaction into the mempool, a node executes a standardized preliminary verification pipeline:

```text
Raw Transaction Payload
           ↓
[ 1. Structure & Canonical Byte Check ]
           ↓
[ 2. Cryptographic Authorization Validation ]
           ↓
[ 3. UTXO Existence & Solvency in Current State ]
           ↓
[ 4. Value Conservation (Inputs >= Outputs) ]
           ↓
[ 5. Positive / Valid Fee Calculation ]
           ↓
[ 6. Local Policy & Spam Threshold Compliance ]
           ↓
   Admitted to Mempool (Pending Status)
```

### Rejection Conditions:
A candidate transaction is rejected immediately if:
- The binary payload violates canonical serialization schemas.
- Cryptographic authorization proofs fail verification.
- Any referenced input `OutPoint` does not exist in the active `UTXO_SET`.
- The transaction attempts to spend an already-spent UTXO (double-spend against confirmed state).
- Total output value exceeds total input value ($\sum \text{Outputs} > \sum \text{Inputs}$).
- The calculated fee is invalid or insufficient under local node policy.

---

## 4. Transaction Identity & Deduplication

Transactions within the mempool are indexed exclusively by their 32-byte cryptographic identifier:

$$\text{TxID} = \text{BLAKE3}(\text{Serialize}_{\text{canonical}}(\text{Transaction}))$$

- **Strict Deduplication:** A mempool instance never stores multiple copies of the same `TxID`.
- **Idempotent Admission:** Receiving a known `TxID` is treated as a redundant broadcast and dropped without error or state mutation.
- Cross-References: [`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md) and [`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md).

---

## 5. Pending Double-Spend Conflict Detection

The mempool actively tracks all `OutPoints` consumed by currently pending transactions to detect conflicting spends:

```text
                            Confirmed UTXO A
                                   │
                   ┌───────────────┴───────────────┐
                   ▼                               ▼
       Transaction X (Pending)          Transaction Y (Incoming)
        (Consumes UTXO A)                (Attempts to Consume UTXO A)
```

- **Conflict Invariant:** The mempool will never simultaneously hold two conflicting transactions attempting to consume the same input `OutPoint`.
- **Specification Status:** `Conflicting Pending Spend Policy: TBD` (e.g., First-Seen policy vs. Replace-By-Fee prioritization rules to be formalized in a dedicated policy milestone).

---

## 6. Transaction Fees & Fee Density

Transaction fees represent the implicit difference between consumed inputs and generated outputs:

$$\text{Fee} = \sum \text{Input Values} - \sum \text{Output Values} \quad (\text{in integer quanta})$$

```text
Conversion: 1 SCY = 100,000,000 quanta
```

### Fee Rate Metric:
Nodes compute a normalized fee density metric to guide mining selection:

$$\text{Fee Rate} = \frac{\text{Transaction Fee (quanta)}}{\text{Transaction Size / Weight}}$$

- **Signaling Role:** Fee rate serves as an economic prioritization signal for miners, **not as an immutable consensus rule**.
- **Specification Status:** `Fee Rate Unit: TBD` (quanta per serialized byte vs. abstract weight units).

---

## 7. Miner Selection Autonomy

Mining nodes possess complete autonomy in choosing how to select and pack transactions from the mempool into candidate blocks:

```text
                  Mempool Candidate Pool
                            ↓
               [ Miner Selection Strategy ]
         ├── Highest Fee Rate First (Economic Maximization)
         ├── Highest Absolute Fee First
         ├── First-Seen Arrival Order
         └── Custom Local Policy Filters
                            ↓
               Candidate Block Assembly
```

- **Consensus Independence:** Consensus rules do not enforce a specific transaction ordering or fee sorting algorithm within a valid block.
- **Validity Bound:** As long as the assembled block satisfies structural limits, PoW target, and coinbase limits ($\le \text{Subsidy} + \sum \text{Fees}$), the block is valid regardless of how transactions were ordered.

---

## 8. Intra-Mempool Transaction Dependencies

A transaction in the mempool may depend on outputs created by another unconfirmed transaction:

```text
Transaction Parent (Tx_P) ──> Creates Pending Output (Tx_P : 0)
                                      │
                                      ▼
Transaction Child (Tx_C)  ──> Spends Pending Output (Tx_P : 0)
```

- **Topological Sorting:** If a miner includes a child transaction, it must also include the parent transaction prior to the child within the same block.
- **Specification Status:** `Package / Dependency Policy: TBD` (Package relay limits, maximum ancestor/descendant depth, and package fee calculation).

---

## 9. Block Arrival & State Synchronization

When a new, valid block is accepted into the canonical blockchain, the mempool synchronizes its state:

```text
                     New Canonical Block Accepted
                                  ↓
      [ Identify Confirmed Transactions Included in Block ]
                                  ↓
       [ Remove Included Transactions (TxIDs) from Mempool ]
                                  ↓
    [ Identify Conflicts with Newly Spent On-Chain OutPoints ]
                                  ↓
       [ Evict Conflicting / Invalidated Pending Transactions ]
                                  ↓
          [ Re-evaluate Remaining Pending Transactions ]
```

- **Eviction of Invalidated Spends:** Any pending transaction attempting to spend an `OutPoint` consumed by the new block is purged immediately.

---

## 10. Chain Reorganization Behavior

When a heavier proof-of-work chain causes a chain reorganization:

```text
                     Chain Reorganization Detected
                                  ↓
         [ Disconnect Stale Blocks from Previous Tip ]
                                  ↓
      [ Re-admit Disconnected Transactions to Mempool ]
                                  ↓
           [ Connect New Canonical Branch Blocks ]
                                  ↓
        [ Evict Transactions Confirmed in New Branch ]
                                  ↓
         [ Validate Surviving Pending Mempool Set ]
```

- **Specification Status:** `Reorganization Re-admission Policy: TBD` (Re-validation rules and timeout filters for resurrected transactions).

---

## 11. Resource Management, Capacity & Eviction

To prevent memory exhaustion and denial-of-service vulnerabilities, each node enforces local mempool resource boundaries:

| Resource Constraint | Specification Status | Purpose |
| :--- | :--- | :--- |
| **`Maximum Mempool Size`** | `TBD` | Maximum memory (RAM) allocation for pending transaction storage. |
| **`Minimum Accepted Fee Rate`** | `TBD` | Dynamic or static threshold below which low-fee transactions are rejected. |
| **`Eviction Policy`** | `TBD` | Algorithm for purging lowest-fee-density transactions when memory capacity is saturated. |
| **`Transaction Expiration`** | `TBD` | Maximum duration a pending transaction may reside in memory without block confirmation. |

---

## 12. Persistence & Storage Boundary

- **Optional Persistence:** Mempool persistence across node restarts is an optional local operational convenience, not a consensus requirement.
- **Separation from `redb` Canonical State:**
  - `Mempool` $\ne$ `UTXO_SET` (Mempool holds ephemeral pending candidates; UTXO set holds canonical unspent truth).
  - `Mempool` $\ne$ `BLOCKS` / `TRANSACTIONS` (Mempool does not store permanent historical chain records).
- **Specification Status:** `Mempool Persistence Strategy: TBD`.

---

## 13. Relationship with Automatic Mining

The mempool serves as the dynamic transaction pipeline for Scytale's autonomous mining subsystem:

```text
               Autonomous Mining Lifecycle
                            │
               Reads Mempool Transaction Set
                            │
              Constructs Block Template & Merkle Root
                            │
              Computes Dynamic Coinbase (Subsidy + Fees)
                            │
           Iterates PoW Nonce against Difficulty Target
```

- **Dynamic Re-template:** Significant changes in mempool high-fee inventory trigger automated template updates during miner idling or interval ticks.

---

## 14. Relationship with Scytale Passbook

Scytale Passbook communicates with the mempool to reflect real-time payment states:

| Lifecycle State | Ledger Context | Passbook Presentation | Balance Impact |
| :--- | :--- | :--- | :--- |
| **`Pending`** | Resides in local node mempool. | Rendered as *"Pending / Unconfirmed"*. | **Not Spendable** (Excluded from confirmed balance). |
| **`Confirmed`** | Committed to a connected block. | Rendered as *"Confirmed"*. | **Spendable** (Aggregated into balance). |
| **`Rejected`** | Evicted / Conflicted out. | Rendered as *"Rejected / Invalid"*. | No balance impact. |

- Cross-Reference: [`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md).

---

## 15. Value Provenance Context

Pending mempool transactions do not create canonical Value Provenance:
- A mempool entry represents a **potential future state transition**.
- Value Provenance becomes mathematically anchored only when the transaction is permanently committed into an accepted block header and connected to the UTXO ledger.
- Cross-Reference: [`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md).

---

## 16. Consensus Rules vs. Local Mempool Policy

| Dimension | Consensus Rules (Mandatory & Universal) | Local Mempool Policy (Node Operator Discretion) |
| :--- | :--- | :--- |
| **Authority** | Enforced uniformly by every node on the network. | Configured locally by individual node operators and miners. |
| **Scope** | - Transaction structural validity.<br>- Cryptographic authorization validation.<br>- Strict value conservation ($\sum \text{In} \ge \sum \text{Out}$).<br>- Active UTXO existence.<br>- Block size and Proof-of-Work threshold. | - Maximum mempool memory capacity.<br>- Transaction fee ranking and sorting.<br>- Transaction eviction and expiration criteria.<br>- Minimum relay fee rate thresholds.<br>- Replacement / Replace-By-Fee policies. |

---

## 17. Complete Mempool Transaction Lifecycle

```text
                     Transaction Broadcast Received
                                   ↓
                         Admission Validation
                        ├── INVALID  ──> Dropped
                        └── VALID    ──> Admitted
                                   ↓
                     Pending in Memory Pool
                                   │
       ┌───────────────────────────┼───────────────────────────┐
       ▼                           ▼                           ▼
Included in Block           Conflicts with Block          Expires / Evicted
       ↓                           ↓                           ↓
Confirmed in Ledger         Removed from Mempool        Purged from Memory
```

---

## 18. Open Questions & Pending Parameters

The following parameters and policies are designated as **TBD**:

| Parameter / Policy | Status | Scope |
| :--- | :--- | :--- |
| **Maximum Mempool Size** | `TBD` | Default memory cap in megabytes. |
| **Minimum Relay Fee Policy** | `TBD` | Baseline quanta per unit size for network relay. |
| **Fee Rate Unit Definition** | `TBD` | Standard unit (e.g., `quanta / byte`). |
| **Eviction Algorithm** | `TBD` | Strategy for shedding low-priority transactions under memory pressure. |
| **Expiration Duration** | `TBD` | Time-to-live (TTL) for unconfirmed transactions in memory. |
| **Transaction Replacement Policy** | `TBD` | Explicit rules for handling conflicting replacement transactions. |
| **Package Relay & Dependencies** | `TBD` | Maximum unconfirmed ancestor/descendant limits. |
| **Local Disk Persistence** | `TBD` | Schema and format for optional mempool state dumping across restarts. |

---

## 19. Cross-Specification References

- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Transaction format, inputs, outputs, and validity checks.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint definitions, lifecycle, and double-spend rules.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block transaction vectors and coinbase positioning.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work verification.
- **[`docs/DIFFICULTY-SPEC.md`](DIFFICULTY-SPEC.md)**: Difficulty adjustment and 60s target block interval.
- **[`docs/STORAGE-SPEC.md`](STORAGE-SPEC.md)**: Canonical state vs. ephemeral pending storage.
- **[`docs/VALUE-PROVENANCE-SPEC.md`](VALUE-PROVENANCE-SPEC.md)**: Value lineage and DAG traversal.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 42,000,000 SCY cap, fee mechanisms, and integer quanta accounting.
- **[`docs/ECONOMIC-MODEL.md`](ECONOMIC-MODEL.md)**: Miner fee market dynamics and block space scarcity.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: Presentation of pending and confirmed transaction statuses.
