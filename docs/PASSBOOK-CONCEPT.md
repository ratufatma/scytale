# Scytale Passbook Product Concept

This document defines the product concept, architectural role, and user-facing design principles of **Scytale Passbook**.

---

## 1. Definition & Architectural Role

> **Passbook is Scytale's user-facing presentation and interaction interface that renders asset ownership, balances, and transaction records through the intuitive mental model of a traditional financial passbook.**

```text
+-------------------------------------------------------------------------+
|                        Scytale Passbook Layer                           |
|       - User-Facing Financial Interface                                 |
|       - Human-Readable Balance & Transaction History                    |
|       - Value Provenance Viewer & Transaction Creation Flow             |
+------------------------------------+------------------------------------+
                                     |
                                     v
+-------------------------------------------------------------------------+
|                      Cryptographic Wallet Layer                         |
|       - Key Management, Signing, & Authorization Credentials            |
+------------------------------------+------------------------------------+
                                     |
                                     v
+-------------------------------------------------------------------------+
|                        Scytale Canonical Ledger                         |
|       - Source of Truth (UTXO Set, Consensus Rules, Storage)           |
+-------------------------------------------------------------------------+
```

### What Passbook Is Not:
- **Not an Independent Source of Truth:** Passbook does not maintain or dictate balances independently.
- **Not the Canonical Ledger:** All state resides strictly on the Scytale blockchain.
- **Not a Consensus Engine:** Passbook does not validate network consensus rules or produce blocks.
- **Not a Node Replacement:** It interfaces with the underlying ledger/node to query state and broadcast signed transactions.

---

## 2. Product Metaphor

Passbook adopts the familiar mental model of a physical bank passbook, abstracting raw cryptographic ledger mechanics into clean, chronological entries:

```text
SCYTALE PASSBOOK
─────────────────────────────────────────────
Balance
12.45000000 SCY

Transaction History
Date         Description        Amount (SCY)      Status
─────────────────────────────────────────────
02 Sep       Received            +5.00000000      Confirmed
01 Sep       Payment             -2.50000000      Confirmed
29 Aug       Mining Reward      +10.00000000      Confirmed
27 Aug       Payment             -1.00000000      Confirmed
```

*Note: The figures above are illustrative examples to demonstrate presentation structure.*

---

## 3. Balance Calculation Model

Passbook adheres strictly to the rule that **balance is a dynamically derived property, never a static record**:

```text
Scytale Canonical Ledger
           ↓
    Active UTXO Set
           ↓
Filter by Verifiable Ownership
           ↓
   Relevant Active UTXOs
           ↓
      Σ UTXO Values
           ↓
     Passbook Balance
```

### Denomination & Precision Rules:
- **Internal Accounting:** All balance summations and ledger computations are executed strictly in **integer quanta** ($1\text{ SCY} = 100,000,000\text{ quanta}$).
- **Display Representation:** The user interface may render amounts in human-readable decimal format (e.g., `12.45000000 SCY`), but internal state is stored exclusively as `1,245,000,000 quanta`.
- **Zero Synthetic Balances:** Passbook never displays credit or funds that cannot be substantiated by active, verifiable UTXOs on the ledger.

---

## 4. Transaction History

Passbook presents a clean, chronological ledger of all transactions relevant to the user's keys:

```text
Transaction Record Summary
├── Date / Timestamp
├── Direction / Type (e.g., Received, Sent, Mining Reward, Fee)
├── Net Amount (in SCY / quanta)
├── Transaction Fee
├── Status (Pending, Confirmed, Rejected, Unknown)
└── Transaction Identifier (TxID)
```

---

## 5. Transaction Detail & Auditability

Selecting any entry reveals a comprehensive, transparent inspection view:

```text
Transaction Detail View
├── TxID (32-byte Blake3 Hash)
├── Confirmation Status & Block Reference (Height / Hash)
├── Gross Amount & Fee Breakdown (in quanta & SCY)
├── Inputs Consumed (Originating OutPoints)
├── Outputs Created (Destination OutPoints & Values)
└── Value Provenance Chain
```

---

## 6. Value Provenance Viewer

Passbook serves as an intuitive viewer for Scytale's **Value Provenance** consensus invariant:

```text
Issuance Block
      ↓
Coinbase Transaction
      ↓
   TxID (Hash)
      ↓
  Initial UTXO
      ↓
Transferred via Transaction
      ↓
   New OutPoint
      ↓
Current Spendable UTXO (in Passbook)
```

- **Viewer Role:** Passbook does not generate provenance; it traverses the verifiable DAG history provided by the node and renders the lineage of coins held by the user.
- **Transparency:** Enables users to verify that every quantum held possesses a direct, unbroken lineage back to legitimate block subsidies or valid genesis issuance.

---

## 7. Send Workflow

Passbook guides users through a clear, deterministic value transfer process:

```text
Available Active UTXOs
           ↓
User Enters Destination Identifier
           ↓
User Enters Amount (SCY / quanta)
           ↓
Fee Estimation & UTXO Coin Selection
           ↓
Canonical Transaction Constructed
           ↓
Cryptographic Authorization (Signed by Wallet)
           ↓
Transaction Submitted to Mempool / Node
           ↓
Network Confirmation
           ↓
Passbook History & Balance Updated
```

---

## 8. Receive Workflow

Passbook provides a simple mechanism for receiving funds:

```text
Receive Request
├── Receiving Identifier (Public Address / Locking Target)
├── Requested Amount (Optional, in SCY / quanta)
└── Real-time Inbound Transaction Monitoring
```

- **Identifier Status:** `Address / Receiving Identifier Format: TBD` (Dependent on the finalized cryptographic authorization specification).

---

## 9. Separation of Concerns: Passbook vs. Wallet

| Layer | Primary Role | Responsibilities |
| :--- | :--- | :--- |
| **Passbook** | User Interface & Financial Presentation | Balance display, history rendering, transaction drafting, provenance visualization, and ledger monitoring. |
| **Wallet** | Cryptographic & Security Component | Key generation, secret seed storage, private key custody, and cryptographic signature generation. |

Passbook coordinates with wallet functionality to request signatures for constructed transactions, but never compromises cryptographic security boundaries.

---

## 10. Passbook & UTXO Interactions

```text
Canonical UTXO Set (Ledger)
            ↓
Ownership & Authorization Verification
            ↓
Passbook Interface Layer
            ↓
Human-Readable Financial Journal
```

- Passbook interacts with UTXOs strictly read-only for balance calculation.
- Any modification to UTXO state occurs exclusively by broadcasting valid, signed transactions that satisfy network consensus rules.

---

## 11. Trust & Integrity Guarantees

Passbook is designed with strict integrity constraints:
- **No Synthetic Credit:** Does not display unverified or fictional balances.
- **Immutable History:** Never alters or rewrites confirmed transaction history.
- **Cache Isolation:** Does not treat local UI caches as canonical ground truth; always re-verifies against the node's UTXO set.
- **Explicit Lifecycle States:**
  - `Pending`: Broadcasted to mempool, awaiting inclusion in a block.
  - `Confirmed`: Included in a valid, connected blockchain block.
  - `Rejected`: Dropped or invalidated by consensus rules.
  - `Unknown / Unverified`: Awaiting node synchronization or network verification.

---

## 12. Design Philosophy

> **"Complex ledger underneath, familiar financial experience above."**

1. **Simplicity First:** Present balances, payments, and receipts with the clarity of modern banking journals.
2. **Accessible Depth:** Keep protocol complexities (OutPoints, byte serialization, Blake3 hashes) accessible under detail views without cluttering daily workflows.
3. **Utility-Driven:** Focus squarely on ownership, payments, and auditable records rather than high-frequency trading or speculative market dashboards.

---

## 13. Non-Goals

Scytale Passbook explicitly avoids the following scopes:
- Not an exchange or token swap interface.
- Not a speculative trading terminal or charting engine.
- Not a custodial banking service or lending platform.
- Not an investment dashboard or DeFi yield tracker.
- Not a standalone full block explorer.

---

## 14. Open Questions & Pending Specifications

The following implementation domains remain designated as **TBD** pending subsequent protocol and interface milestones:

| Area | Status | Scope |
| :--- | :--- | :--- |
| **Address / Receiving Format** | `TBD` | Encoding and checksum scheme for public receiving identifiers. |
| **Authorization Integration** | `TBD` | Interface contract between Passbook UI and underlying signing keystore. |
| **Key Management & Custody** | `TBD` | Seed generation, key derivation paths, and hardware key integration. |
| **Backup & Recovery** | `TBD` | Mnemonic phrase standards and secure encrypted export formats. |
| **Target Platform Selection** | `TBD` | Distribution target (Terminal/CLI, Native Desktop, Mobile, or Lightweight Web). |
| **Confirmation Policy** | `TBD` | Default depth threshold required to mark transactions as permanently settled. |
| **Transaction Labeling & Notes** | `TBD` | Local user annotations and payment categorization metadata. |
| **Fiat Reference Valuation** | `TBD` | Optional third-party price feeds vs. strict self-sovereign offline operation. |

---

## 15. Cross-Specification References

This product concept integrates directly with the technical specifications defined across the Scytale documentation suite:
- **[`docs/LEDGER-SPEC.md`](LEDGER-SPEC.md)**: Top-level UTXO ledger architecture and accounting units.
- **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)**: Structural layout, inputs, outputs, and fees.
- **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)**: OutPoint indexing, state transitions, and Value Provenance.
- **[`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md)**: Locking conditions and validation proofs.
- **[`docs/MONETARY-POLICY.md`](MONETARY-POLICY.md)**: SCY and quanta denomination standards ($10^8\text{ quanta/SCY}$).
- **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md)**: BLAKE3 TxID derivation and canonical encoding.
