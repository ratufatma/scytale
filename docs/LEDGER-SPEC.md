# Scytale Ledger Specification

Scytale implements a pure **Unspent Transaction Output (UTXO)** ledger model. The ledger state is defined exclusively by the set of all existing, unspent transaction outputs.

---

## 1. Core Data Structures

### 1.1 OutPoint
A reference to a specific output of a previously confirmed transaction.

```text
OutPoint
├── txid: 32-byte transaction hash (Blake3)
└── index: u32 (0-indexed output position within the transaction)
```

### 1.2 TxIn (Transaction Input)
Consumes an existing unspent output from the UTXO set.

```text
TxIn
├── previous_output: OutPoint
└── unlocking_data: Vec<u8> (Data satisfying the locking conditions of the referenced output)
```

> [!NOTE]
> Detailed cryptographic authorization formats (such as specific digital signature algorithms, witness schemes, or verification scripts) are explicitly decoupled at this baseline stage and will be specified in subsequent phases.

### 1.3 TxOut (Transaction Output)
Creates a new spendable output with an assigned value and spending conditions.

```text
TxOut
├── value: u64 (Atomic token units)
└── locking_data: Vec<u8> (Conditions/commitment required to spend this output)
```

### 1.4 Tx (Transaction)
A state transition container that consumes zero or more existing outputs and produces one or more new outputs.

```text
Tx
├── version: u32
├── inputs: Vec<TxIn>
└── outputs: Vec<TxOut>
```

---

## 2. State Transition & UTXO Invariants

1. **Existence & Exclusivity:**
   - Every `previous_output` referenced in `TxIn` must exist in the active UTXO set.
   - A UTXO can only be spent once. Once referenced by a confirmed transaction input, it is permanently consumed and removed from the active set.

2. **Atomic State Transition:**
   - When a transaction is applied, all consumed input UTXOs are deleted from the UTXO set and all produced outputs (`TxOut`) are added to the UTXO set within a single, atomic database transaction.
   - If any input or condition fails verification, the entire transaction is rejected and the ledger state remains unchanged.

3. **Value Conservation & Transaction Fees:**
   - For standard (non-coinbase) transactions, the total value of all inputs must be strictly greater than or equal to the total value of all outputs:
     $$\sum \text{Input Values} \ge \sum \text{Output Values}$$
   - The implicit difference represents the transaction fee allocated to the miner:
     $$\text{Fee} = \sum_{i} \text{Input}_i.\text{value} - \sum_{j} \text{Output}_j.\text{value}$$

---

## 3. Coinbase Transaction & Block Rules

1. **Position:**
   - The coinbase transaction must be the exact first transaction (`index 0`) in every block.
   - A block must contain exactly one coinbase transaction.

2. **Coinbase Input:**
   - The coinbase transaction does not consume standard UTXO outputs.
   - Its input carries arbitrary miner data (such as extra nonce or height signals) instead of referencing a valid `previous_output`.

3. **Maximum Coinbase Valuation:**
   - Consensus independently computes the maximum permissible output value for the coinbase transaction:
     $$\text{Maximum Coinbase Value} = \text{Block Subsidy}(height) + \sum_{k=1}^{N} \text{Fee}(Tx_k)$$
     where $N$ is the number of non-coinbase transactions included in the block.
   - **Block Invalidation:** If the sum of output values in the coinbase transaction exceeds $\text{Maximum Coinbase Value}$, the entire block is invalid and must be rejected.
   - A miner is permitted to claim less than the maximum allowable coinbase value (leaving the remainder unminted).
