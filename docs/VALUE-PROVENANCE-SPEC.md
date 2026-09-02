# Scytale Value Provenance Specification

This document defines the formal specification for **Value Provenance** in the Scytale blockchain engine. It establishes how native asset value (denominated in `quanta`) is deterministically traced to its issuance origin across the ledger without requiring individual database records or serial numbers for each atomic quantum.

---

## 1. Purpose & Objectives

Value Provenance is a foundational consensus invariant in Scytale designed to ensure:

- **Complete Auditability:** Every spendable quantum on the ledger can be traced back through an unbroken historical lineage to a valid protocol-defined issuance origin.
- **Macro Supply Transparency:** Guarantees that no unbacked, synthetic, or off-ledger coins can enter the circulating supply.
- **User Transparency:** Enables users and the Passbook layer to visually audit the origin and transaction lineage of their funds.
- **Decoupled Presentation:** Provides a verifiable provenance graph from canonical ledger records without requiring the presentation layer (Passbook) to act as a source of truth.

> **Core Axiom:** *Every spendable value within Scytale must possess a mathematically verifiable, deterministic provenance path.*

---

## 2. Denomination & Accounting Foundation

Scytale defines a single native coin, **Scytale Coin** (`SCY`), with two standardized denomination tiers:

```text
Project / Protocol : Scytale
Native Coin        : Scytale Coin
Asset Symbol       : SCY
Smallest Unit      : quanta
Conversion         : 1 SCY = 100,000,000 quanta (10^8 quanta)
```

- **Denomination Equivalence:** `SCY` (presentation tier) and `quanta` (accounting tier) represent the exact same native coin (**Scytale Coin**).
- **Zero Floating-Point Precision:** All provenance tracking, balance calculations, fees, and transfers operate strictly in **unsigned 64-bit integer quanta** (`u64`).

---

## 3. Core Provenance Principles

1. **No Arbitrary Value Creation:** Value cannot be minted outside deterministic consensus block subsidies or explicitly locked genesis allocations.
2. **No Untraceable Value:** A transaction input is invalid if it references an output whose parent transaction history is unresolvable.
3. **Consensus-Grounded Veracity:** Provenance is derived exclusively from verified, consensus-valid ledger state transitions, never from client-side caches or UI assertions.

---

## 4. UTXO-Based Provenance Model

Provenance in Scytale is structured upon the **Unspent Transaction Output (UTXO)** architecture:

```text
Transaction Output (TxOut)
           ↓
Unspent Transaction Output (UTXO)
           ↓
OutPoint Primary Key (TxID : OutputIndex)
           ↓
Creating Transaction
           ↓
Ancestral Origin (Coinbase Block Subsidy or Genesis Allocation)
```

### OutPoint Reference:
$$\text{OutPoint} = (\text{TxID}, \text{OutputIndex})$$
- **`TxID`:** 32-byte BLAKE3 cryptographic hash of the creating transaction.
- **`OutputIndex`:** 0-based integer position (`u32`) of the output within the transaction's `outputs` vector.

Cross-References: [`docs/UTXO-SPEC.md`](UTXO-SPEC.md), [`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md), and [`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md).

---

## 5. Provenance $\ne$ Individual Record per Quantum

A crucial architectural principle of Scytale is that **Value Provenance does NOT create individual database records or serial numbers for each quantum**:

```text
INCORRECT Mental Model (Per-Quantum Tracking):
1 quanta  ──> Database Record
1 quanta  ──> Database Record
1 quanta  ──> Database Record  (Massive, unscalable storage bloat)

CORRECT Scytale Architecture (UTXO Lineage Tracking):
UTXO A: (TxID: 0x4f3a..., Index: 0, Value: 1,000,000 quanta)
──> Single compact database record representing the full aggregate integer value.
```

- **Lineage over Serialization:** Provenance is established by traversing the Directed Acyclic Graph (DAG) of **UTXOs and Transactions**, not by assigning discrete IDs to individual quanta.
- **Storage Efficiency:** Enables lightweight, scalable database storage while preserving 100% mathematical auditability.

---

## 6. Value Flow & Conservation

When a UTXO is spent, value flows deterministically from inputs to newly created outputs and miner fees:

```text
                   Input UTXO A
                1,000,000 quanta
                       │
                       ▼
             Spending Transaction (Tx_k)
                       │
         ┌─────────────┴─────────────┬────────────────────────┐
         ▼                           ▼                        ▼
     Output B                    Output C             Transaction Fee
  600,000 quanta              399,000 quanta            1,000 quanta
  (New OutPoint: Tx_k:0)      (New OutPoint: Tx_k:1)    (To Miner Coinbase)
```

$$\sum \text{Input Values} = \sum \text{Output Values} + \text{Transaction Fee}$$
$$1,000,000\text{ quanta} = 600,000\text{ quanta} + 399,000\text{ quanta} + 1,000\text{ quanta}$$

- **Fee Provenance:** Transaction fees do not destroy or create value; they transfer existing quanta from the transaction creator to the miner's coinbase output.

---

## 7. Quanta-Level Traceability

Scytale deterministically resolves the ancestral origin of any quantum value held in an active UTXO:

```text
Current Spendable UTXO (X quanta)
               ↓
Referenced Creating Transaction (TxID_n)
               ↓
Consumed Input OutPoints (TxID_{n-1} : Index)
               ↓
Preceding Ancestral Transactions ...
               ↓
Protocol-Defined Issuance Origin (Genesis Block or Coinbase Mined Subsidy)
```

Every quantum in an output inherits the collective lineage of the inputs consumed to construct that transaction.

---

## 8. The Provenance Directed Acyclic Graph (DAG)

Ledger state transitions form a deterministic graph of value lineage:

```text
              [ Genesis Block / Block 0 ]  ──  [ Coinbase Mined Block ]
                           │                               │
                           ▼                               ▼
                      Transaction A                  Transaction B
                           │                               │
                    ┌──────┴──────┐                 ┌──────┴──────┐
                    ▼             ▼                 ▼             ▼
                UTXO A:0       UTXO A:1         UTXO B:0       UTXO B:1
                600k q         400k q           500k q         500k q
                    │                               │
                    └───────────────┬───────────────┘
                                    ▼
                              Transaction C
                                    │
                             ┌──────┴──────┐
                             ▼             ▼
                         UTXO C:0       UTXO C:1
                          800k q         299k q  (Fee: 1k q)
```

---

## 9. Coinbase Mining Provenance

Newly minted Proof-of-Work block subsidies enter the provenance graph through the Coinbase transaction:

```text
Mined Proof-of-Work Block (Height H)
                 ↓
      Coinbase Transaction (index 0)
                 ↓
           Coinbase TxID
                 ↓
     Coinbase OutPoint (TxID : 0)
                 ↓
      Newly Created Mined UTXO
```

- **Subsidy Origin:** The block subsidy ($R(h)$) represents verified new issuance authorized by Proof-of-Work consensus.
- **Fee Incorporation:** Included fees represent the transfer and consolidation of existing quanta from confirmed transactions.

---

## 10. Genesis Allocation Provenance

Genesis allocations distributed at network launch originate strictly from Block 0 transaction outputs:

```text
Genesis Block (Height 0)
           ↓
Genesis Bootstrap Transaction
           ↓
      Genesis TxID
           ↓
Genesis OutPoints (TxID : Index)
           │
           ├── Founder Allocation OutPoint          (15% /  6,300,000 SCY)
           ├── Development / Treasury OutPoint      ( 5% /  2,100,000 SCY)
           └── Ecosystem / Community OutPoint       ( 5% /  2,100,000 SCY)
```

- **Equal Provenance Grounding:** Founder, Treasury, and Ecosystem allocations possess identical on-chain auditability to mined coins. There are zero privileged, off-ledger, or hidden issuance paths.

---

## 11. Founder Allocation Provenance Model

The locked founder parameters are bound to the public ledger:

- **Share:** `15%` ($6,300,000\text{ SCY} = 630,000,000,000,000\text{ quanta}$).
- **Nature:** One-time issuance executed at Block 0.
- **Audit Rule:** Any movement of founder funds creates standard spending transactions on the ledger, preserving full public visibility.
- **No Ongoing Mint:** Founder keys possess zero future issuance rights.

---

## 12. Macro Supply Reconciliation

At any point in chain history, total ledger value reconciles with mathematical exactness:

$$\text{Maximum Supply} = \text{Genesis Allocation} + \text{Issued Mining Rewards} + \text{Unissued Reserve}$$

$$4,200,000,000,000,000\text{ quanta} = 1,050,000,000,000,000\text{ quanta} + \sum_{i=1}^{H} R(i) + \text{Unissued Reserve}(H)$$

---

## 13. Relationship with Scytale Passbook

Passbook acts as a **human-readable viewer** for the underlying Value Provenance graph:

```text
SCYTALE PASSBOOK - VALUE PROVENANCE INSPECTION
─────────────────────────────────────────────────────────────
Selected Entry : +5.00000000 SCY (500,000,000 quanta)
UTXO OutPoint  : 3b9a7c...f812 : Output 0

Lineage Path:
├── [Block #10,420] Coinbase Subsidy (10.00 SCY)
│         ↓
├── [TxID: 8d2e...4a11] Transfer -> OutPoint 8d2e...:1 (5.00 SCY)
│         ↓
└── [Current Spendable UTXO in Passbook]
```

### Protocol Identity vs. Presentation References:
| Protocol / Ledger Identity | Passbook Presentation Layer |
| :--- | :--- |
| **`TxID`** (32-byte BLAKE3 Digest) | Chronological Entry Number (e.g., Entry #00042) |
| **`OutPoint`** (`TxID : OutputIndex`) | Transaction Description ("Received Payment", "Mining Reward") |
| **`BlockID` / Block Height** | Human-readable Date / Timestamp |

*Passbook entry numbers are local UI conveniences; consensus identity resides strictly in cryptographic hashes and OutPoints.*

---

## 14. Partial Value Spending & Lineage Inheritance

When a UTXO is split across multiple outputs of smaller values, provenance is not degraded or lost:

```text
                    UTXO Parent (1,000,000 quanta)
                                  │
                   ┌──────────────┴──────────────┐
                   ▼                             ▼
         Output 0 (300,000 q)          Output 1 (699,000 q)
    (Inherits Full Parent Lineage) (Inherits Full Parent Lineage)
```

- Each child output inherits the complete ancestral DAG of the parent UTXO.
- Value consolidation (merging multiple UTXOs into one output) similarly joins their ancestral DAG lineages.

---

## 15. No Coin Serialization Requirement

Scytale explicitly rejects per-quantum serialization (such as assigning serial numbers to individual quanta):
- **Lightweight State:** The UTXO set only stores `(OutPoint -> TxOut { value, locking_condition })`.
- **High Performance:** Nodes avoid tracking billions of individual quanta records.
- **Deterministic Derivation:** Historical provenance is computed on-demand by traversing confirmed transaction records.

---

## 16. Double-Spend Prevention & State Exclusivity

Value Provenance enforces that an active UTXO can be spent **exactly once**:
- A confirmed transaction consumes its input UTXOs atomically.
- Any competing transaction attempting to reference an already-consumed OutPoint is rejected immediately as an invalid state transition.

---

## 17. Historical State vs. Active Spendable State

```text
+-------------------------------------------------------------------------+
|                  Historical Ledger History (Read-Only)                  |
|    - Confirmed Blocks, Transactions, Signatures, Inputs, Outputs        |
|    - Provides the complete Value Provenance DAG                         |
+-------------------------------------------------------------------------+
                                    │
                                    ▼
+-------------------------------------------------------------------------+
|                    Active UTXO Set (Spendable State)                    |
|    - Contains currently unspent transaction outputs                     |
|    - Evaluated by consensus for double-spend prevention                 |
+-------------------------------------------------------------------------+
```

---

## 18. Storage Architecture Implications

While specific database schemas are defined in storage specifications:
- The storage layer must maintain indexed links enabling efficient backward traversal from an `OutPoint` to its creating transaction and parent block.
- `Storage Design: Defined in future storage specification.`

---

## 19. Deterministic Provenance Traversal Query

A compliant node resolves provenance through the following deterministic algorithm:

```text
Given Target OutPoint (TxID_0, Index_0):
  1. Retrieve transaction Tx_0 matching TxID_0.
  2. Locate Block B_0 containing Tx_0.
  3. If Tx_0 is Genesis -> Output Provenance: Genesis Allocation (Category resolved). Done.
  4. If Tx_0 is Coinbase -> Output Provenance: Mined Block Subsidy at Height H_0. Done.
  5. For each input in Tx_0.inputs:
       Recursively trace input.previous_output (TxID_prev, Index_prev).
```

---

## 20. Security & Trust Model

- **Ledger Invariant:** Provenance is only as trustworthy as the Proof-of-Work and UTXO consensus validation of the underlying blocks.
- **Zero Client Assumptions:** Client applications, Passbook interfaces, and block explorers cannot fabricate provenance; they merely present verified on-chain history.

---

## 21. Open Questions & Pending Specifications

The following implementation domains remain designated as **TBD**:

| Area | Status | Scope |
| :--- | :--- | :--- |
| **Provenance Query API** | `TBD` | RPC / Node query contract for fetching ancestral DAG paths. |
| **Historical Indexing Optimization** | `TBD` | Secondary indexing strategies in `scytale-storage` for rapid lineage lookups. |
| **Pruning Policy vs. Lineage Availability** | `TBD` | Rules for archival nodes vs. pruned validating nodes regarding full historical provenance traversal. |
| **Genesis Provenance Data Encoding** | `TBD` | Specific metadata encoding within Block 0 outputs. |
| **Passbook Provenance Inspection UI** | `TBD` | Visual graphical tree representation in Passbook client. |

---

## 22. Cross-Specification References

- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: Master UTXO ledger model and quanta denomination.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint lifecycle, state transitions, and validation.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Transaction format, TxID derivation, and validity rules.
- **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)**: Block structure, coinbase position, and state transitions.
- **[`docs/GENESIS-ALLOCATION.md`](GENESIS-ALLOCATION.md)**: 15/5/5/75 supply distribution breakdown.
- **[`docs/GENESIS-SPEC.md`](GENESIS-SPEC.md)**: Genesis block specification and zero-balance onboarding.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: 42,000,000 SCY cap, emission schedule, and integer quanta arithmetic.
- **[`docs/POW-SPEC.md`](POW-SPEC.md)**: Proof-of-Work validation and BLAKE3 target evaluation.
- **[`docs/PASSBOOK-CONCEPT.md`](PASSBOOK-CONCEPT.md)**: User-facing financial presentation layer and provenance viewer.
