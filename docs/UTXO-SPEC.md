# Scytale UTXO Specification

This document defines the formal specification of the Unspent Transaction Output (UTXO) model in Scytale. In Scytale, the global ledger state is represented exclusively as an immutable collection of unspent discrete value units identified by unique output points.

---

## 1. UTXO Definition & Identity

A **UTXO** is an unspent transaction output that has been validated, recorded in the active ledger state, and is eligible to be consumed by a future transaction.

### 1.1 OutPoint (UTXO Primary Key)
Every UTXO is uniquely identified across the entire blockchain lifecycle by an `OutPoint`:

$$\text{OutPoint} = (\text{TxID}, \text{OutputIndex})$$

```text
OutPoint
├── TxID: 32-byte Blake3 hash of the originating transaction
└── OutputIndex: u32 (0-indexed position within originating transaction outputs)
```

### 1.2 UTXO State Record
Within the storage layer (`scytale-storage`), a UTXO record contains the spending requirements and the integer value:

```text
UTXO
├── value: u64 (Atomic integer amount denominated in quanta)
└── locking_condition: Vec<u8> (Encumbrance required to authorize spending)
```

---

## 2. UTXO Lifecycle

A UTXO transitions through distinct, non-reversible lifecycle phases:

```text
       Transaction Output
               ↓
          UTXO Created
  (Added to active UTXO set)
               ↓
      UTXO Remains Unspent
    (Available for spending)
               ↓
Transaction References OutPoint
               ↓
        UTXO Consumed
(Permanently deleted from set)
               ↓
    New Transaction Outputs
               ↓
           New UTXOs
```

### Core Lifecycle Rules:
1. **Pre-existence:** A UTXO must exist in the active UTXO set before it can be referenced by a transaction input.
2. **Double-Spend Prevention:** A UTXO can only be spent once. Referencing an `OutPoint` that has already been spent or does not exist renders the transaction invalid.
3. **Atomic State Deletion:** When a transaction is accepted into a block, all consumed UTXOs are atomically removed from the active state table.
4. **Immediate Availability:** All newly created outputs (`TxOut`) from a confirmed transaction become active UTXOs available for spending in subsequent transactions.
5. **Atomic Execution:** State updates (deleting consumed inputs and inserting new outputs) occur within a single database transaction; partial updates are impossible.
6. **Strict Integer Accounting:** Every UTXO value is denominated in integer **quanta** ($10^{-8}\text{ SCY}$).
7. **Solvency Invariant:** The total value of consumed input UTXOs must satisfy the sum of all created output values plus the implicit transaction fee.

---

## 3. Value Provenance

In Scytale, **Value Provenance** is a foundational consensus property. Every unit of spendable value on the ledger must possess an unbroken, verifiable ancestry path leading back to a valid coinbase block issuance or genesis allocation.

```text
Standard Value Provenance:
Origin Block / Preceding State
              ↓
    Spending Transaction
              ↓
         TxID (Hash)
              ↓
      OutPoint (TxID:Index)
              ↓
            UTXO
              ↓
   Subsequent Spending Tx
              ↓
         New OutPoint
              ↓
          New UTXO
```

```text
Coinbase Value Provenance:
          Block
            ↓
   Coinbase Transaction
            ↓
       Coinbase TxID
            ↓
   OutPoint (TxID:Index)
            ↓
       Initial UTXO
```

### Principles of Value Provenance:
1. **Deterministic Ancestry:** Every UTXO in the current state set can be traced backward through a directed acyclic graph (DAG) of transaction identifiers to its originating block.
2. **No Arbitrary Value Creation:** Value cannot enter the ledger state through synthetic or unauthorized channels; new supply is minted exclusively through protocol coinbase rules.
3. **No Untraceable Value:** There are no anonymous balance injections or non-deterministic state adjustments. Every spendable quantum is tied to a specific `OutPoint`.
4. **Consensus Invariant:** Value provenance is not an optional indexing feature or block explorer utility; it is a structural consensus constraint enforced on every block connection.

---

## 4. State Storage Model

The active UTXO set is maintained within the embedded database (`scytale-storage` via `redb`) as a high-performance key-value mapping:

$$\text{Key: } \text{OutPoint} \implies \text{Value: } \text{UTXO}(\text{value}, \text{locking\_condition})$$

- **Read Operations:** Fast key lookup to verify input existence during transaction verification and mempool admission.
- **Write Operations:** Batch deletion of spent `OutPoints` and insertion of new `OutPoints` during block application.
- **Pruning Capability:** Because spent UTXOs are deleted from the active set, the working set size is bounded by the number of unspent outputs rather than total historical transaction volume.
