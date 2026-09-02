# Scytale Ledger Specification

Scytale implements a pure **Unspent Transaction Output (UTXO)** ledger model. The global ledger state is defined exclusively by the set of all existing, unspent transaction outputs (`UTXO Set`).

---

## 1. Denomination & Accounting Units

Ledger accounting is performed strictly in atomic integer units:

```text
SCY    : Primary display unit (UI / user-facing)
quanta : Smallest ledger accounting unit (10^-8 SCY)

Conversion: 1 SCY = 100,000,000 quanta
```

- **Integer Representation:** All balances, inputs, outputs, fees, and coinbase values in the ledger are represented as unsigned 64-bit integers (`u64`) denominated in **quanta**.
- **No Fractional Ambiguity:** Floating-point values are forbidden within consensus state transitions.

---

## 2. Core Data Structures

### 2.1 OutPoint
A reference to a specific output of a previously confirmed transaction.

```text
OutPoint
├── txid: 32-byte transaction hash (Blake3)
└── index: u32 (0-indexed output position within the transaction)
```

### 2.2 TxIn (Transaction Input)
Consumes an existing unspent output from the active UTXO set.

```text
TxIn
├── previous_output: OutPoint
└── unlocking_data: Vec<u8> (Data satisfying the locking conditions of the referenced output)
```

> [!NOTE]
> Specific digital signature schemes, witness formats, and script execution engines are explicitly decoupled at this baseline stage and will be specified in subsequent phases.

### 2.3 TxOut (Transaction Output)
Creates a new spendable output with an assigned value and spending conditions.

```text
TxOut
├── value: u64 (Amount denominated in integer quanta)
└── locking_data: Vec<u8> (Conditions required to spend this output)
```

**Denomination Examples:**
- $1.00\text{ SCY} \implies \text{value} = 100,000,000\text{ quanta}$
- $3.25\text{ SCY} \implies \text{value} = 325,000,000\text{ quanta}$
- $0.00000001\text{ SCY} \implies \text{value} = 1\text{ quanta}$

### 2.4 Tx (Transaction)
A state transition container that consumes zero or more existing outputs and produces one or more new outputs.

```text
Tx
├── version: u32
├── inputs: Vec<TxIn>
└── outputs: Vec<TxOut>
```

---

## 3. Value Provenance & Traceability

A core architectural property of Scytale is **Strict Value Provenance**: every spendable unit of value on the ledger must have an unbroken, mathematically verifiable ancestry path leading back to a valid coinbase block issuance or genesis allocation.

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
1. **No Arbitrary Minting:** Value cannot be created *ex nihilo* outside the strict consensus constraints of the coinbase subsidy.
2. **No Untraceable Value:** Every input references an exact `OutPoint` whose lineage is fully resolvable through preceding valid transactions.
3. **Deterministic Ancestry:** Every UTXO in the current state set can be deterministically traced backward through a directed acyclic graph (DAG) of transaction IDs to its origin block.
4. **Consensus Property:** Provenance is an intrinsic consensus invariant enforced during block validation, not merely an external block explorer or indexing feature.

---

## 4. State Transition & UTXO Invariants

1. **Existence & Exclusivity:**
   - Every `previous_output` referenced in a `TxIn` must exist in the active UTXO set.
   - A UTXO can be spent exactly once. Once referenced by a confirmed transaction input, it is permanently consumed and pruned from the active set.

2. **Atomic State Transition:**
   - When a transaction is applied, all consumed input UTXOs are deleted from the UTXO set and all newly created outputs (`TxOut`) are inserted into the UTXO set within a single, atomic database transaction.
   - If any input, value balance, or authorization condition fails verification, the entire transaction is rejected and the ledger state remains completely unaltered.

3. **Value Conservation & Transaction Fees:**
   - For standard (non-coinbase) transactions, the sum of input values must be greater than or equal to the sum of output values:
     $$\sum \text{Input Values (quanta)} \ge \sum \text{Output Values (quanta)}$$
   - The implicit difference constitutes the transaction fee awarded to the miner:
     $$\text{Fee (quanta)} = \sum_{i} \text{Input}_i.\text{value} - \sum_{j} \text{Output}_j.\text{value}$$

---

## 5. Coinbase Transaction & Block Rules

1. **Position:**
   - The coinbase transaction must be the exact first transaction (`index 0`) in every block.
   - A block must contain exactly one coinbase transaction.

2. **Coinbase Input:**
   - The coinbase transaction does not consume standard UTXO outputs.
   - Its input carries arbitrary miner data (such as extra nonce or height signals) instead of referencing a valid `previous_output`.

3. **Maximum Coinbase Valuation:**
   - Consensus independently computes the maximum permissible output value for the coinbase transaction:
     $$\text{Maximum Coinbase Value (quanta)} = R_{\text{quanta}}(height) + \sum_{k=1}^{N} \text{Fee}_{\text{quanta}}(Tx_k)$$
     where $N$ is the number of non-coinbase transactions included in the block.
   - **Block Invalidation:** If the sum of output values in the coinbase transaction exceeds $\text{Maximum Coinbase Value}$, the entire block is invalid and must be rejected.
   - A miner is permitted to claim less than the maximum allowable coinbase value (leaving the remainder unminted).
