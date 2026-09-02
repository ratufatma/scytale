# Scytale Ledger Specification

Scytale implements a pure **Unspent Transaction Output (UTXO)** ledger model. The global ledger state is defined exclusively by the set of all existing, unspent transaction outputs (`UTXO Set`).

---

## 1. Specification Architecture & Sub-Modules

This document serves as the top-level specification for the Scytale ledger. Specific technical domains are detailed in their respective dedicated specifications:

| Domain | Specification Document | Key Focus Areas |
| :--- | :--- | :--- |
| **Blocks** | **[`docs/BLOCK-SPEC.md`](BLOCK-SPEC.md)** | Block header schema, PoW verification, coinbase position, and state transitions. |
| **Proof-of-Work** | **[`docs/POW-SPEC.md`](POW-SPEC.md)** | BLAKE3 target evaluation, difficulty, and 60-second block cadence. |
| **Transactions** | **[`docs/TRANSACTION-SPEC.md`](TRANSACTION-SPEC.md)** | Structural layout, canonical serialization, TxID, and validity rules. |
| **UTXO Model** | **[`docs/UTXO-SPEC.md`](UTXO-SPEC.md)** | `OutPoint` primary key, lifecycle phases, and storage layout. |
| **Authorization** | **[`docs/AUTHORIZATION-SPEC.md`](AUTHORIZATION-SPEC.md)** | Locking conditions, cryptographic proofs, and stateless verification. |
| **Hashing & Serialization** | **[`docs/HASHING-AND-SERIALIZATION-SPEC.md`](HASHING-AND-SERIALIZATION-SPEC.md)** | BLAKE3 digests, canonical byte encoding, TxID derivation, and determinism. |

---

## 2. Denomination & Accounting Units

Ledger accounting is performed strictly in atomic integer units:

```text
SCY    : Primary display unit (UI / user-facing)
quanta : Smallest ledger accounting unit (10^-8 SCY)

Conversion: 1 SCY = 100,000,000 quanta
```

- **Integer Representation:** All balances, inputs, outputs, fees, and coinbase values in the ledger are represented as unsigned 64-bit integers (`u64`) denominated in **quanta**.
- **Zero Floating-Point Tolerance:** Floating-point values are prohibited within consensus state transitions.

---

## 3. Core Data Structures Overview

```text
Transaction
├── version: u32
├── inputs: Vec<TxIn>
│   └── TxIn
│       ├── previous_output: OutPoint (TxID + OutputIndex)
│       └── authorization: Vec<u8>
└── outputs: Vec<TxOut>
    └── TxOut
        ├── value: u64 (in quanta)
        └── locking_condition: Vec<u8>
```

- **`OutPoint`**: Unique reference pairing a 32-byte transaction hash (`TxID`) with an output position (`u32`).
- **`TxIn`**: Consumes a referenced `OutPoint` and provides the necessary `authorization` payload.
- **`TxOut`**: Encapsulates a spendable `value` in integer quanta and a `locking_condition`.

---

## 4. Value Provenance & Traceability

Scytale enforces **Value Provenance** as a core consensus invariant: every spendable unit of value on the ledger must have an unbroken, mathematically verifiable ancestry path leading back to a valid coinbase block issuance or genesis allocation.

```text
Standard Transaction Lineage:
Issuance
   ↓
Block
   ↓
Transaction
   ↓
TxID
   ↓
OutPoint
   ↓
UTXO
   ↓
Spending Transaction
   ↓
New OutPoint
   ↓
New UTXO

Coinbase Issuance Lineage:
Block
   ↓
Coinbase Transaction
   ↓
Coinbase TxID
   ↓
OutPoint
   ↓
UTXO
```

### Provenance Principles:
1. **No Arbitrary Minting:** Value cannot be created outside the deterministic consensus constraints of the block subsidy.
2. **No Untraceable Value:** Every input references an exact `OutPoint` whose lineage is fully resolvable through preceding valid transactions.
3. **Deterministic Ancestry:** Every UTXO in the current state set can be deterministically traced backward through a directed acyclic graph (DAG) of transaction IDs to its origin block.
4. **Consensus Property:** Provenance is an intrinsic consensus invariant enforced during block connection, not merely an external indexing feature.

---

## 5. State Transition & Value Conservation

1. **Existence & Exclusivity:**
   - Every `previous_output` referenced in a `TxIn` must exist in the active UTXO set.
   - A UTXO can be spent exactly once. Once referenced by a confirmed transaction input, it is permanently consumed and pruned from the active set.

2. **Atomic State Transition:**
   - When a transaction is applied, all consumed input UTXOs are deleted from the UTXO set and all newly created outputs (`TxOut`) are inserted into the UTXO set within a single, atomic database transaction.
   - If any input, value balance, or authorization condition fails verification, the entire transaction is rejected and the ledger state remains completely unaltered.

3. **Value Conservation Invariant:**
   - For standard (non-coinbase) transactions, the sum of input values must equal the sum of output values plus the implicit fee:
     $$\sum \text{Input Values (quanta)} = \sum \text{Output Values (quanta)} + \text{Fee (quanta)}$$
   - A transaction is strictly invalid if $\sum \text{Outputs} > \sum \text{Inputs}$.

---

## 6. Coinbase Transaction & Issuance

1. **Position & Isolation:**
   - The coinbase transaction must be the exact first transaction (`index 0`) in every block.
   - A block must contain exactly one coinbase transaction.
   - The coinbase transaction does not consume standard UTXOs.

2. **Maximum Coinbase Valuation:**
   - Consensus independently computes the maximum permissible output value for the coinbase transaction:
     $$\text{Maximum Coinbase Value (quanta)} = R_{\text{quanta}}(height) + \sum_{k=1}^{N} \text{Fee}_{\text{quanta}}(Tx_k)$$
     where $N$ is the number of non-coinbase transactions included in the block.
   - **Block Invalidation:** If the sum of output values in the coinbase transaction exceeds $\text{Maximum Coinbase Value}$, the entire block is invalid and must be rejected.

---

## 7. Consensus Boundaries vs. Node Policy

Scytale strictly distinguishes universal consensus rules from local node/miner policy:

| Dimension | Consensus Rules (Universal) | Miner / Node Policy (Local Autonomy) |
| :--- | :--- | :--- |
| **Scope** | Enforced identically by every validating node. | Configured locally by miners and node operators. |
| **Examples** | - Valid UTXO existence and no double spends.<br>- Strict value conservation ($\sum \text{Inputs} \ge \sum \text{Outputs}$).<br>- Valid cryptographic authorization.<br>- Coinbase output limit compliance.<br>- Deterministic canonical serialization. | - Transaction fee density prioritization.<br>- Mempool minimum fee rate admission.<br>- Transaction replacement / eviction policies.<br>- Block template transaction selection. |

---

## 8. Determinism Guarantee

All validating nodes must produce byte-for-byte identical state transitions when executing valid transactions against the same ledger state:
- All monetary operations use fixed integer quanta arithmetic.
- Canonical serialization ensures unique transaction hashes (`TxID`).
- Authorization verification is stateless and deterministic.
